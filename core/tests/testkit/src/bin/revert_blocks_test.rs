use web3::transports::Http;

use zksync_core::state_keeper::ZkSyncStateInitParams;
use zksync_types::{block::Block, AccountId, AccountMap, AccountTree};

use zksync_testkit::zksync_account::ZkSyncETHAccountData;
use zksync_testkit::*;
use zksync_testkit::{
    data_restore::verify_restore,
    scenarios::{perform_basic_operations, BlockProcessing},
};
use zksync_types::{BlockNumber, Nonce, TokenId};

use crate::{
    eth_account::{parse_ether, EthereumAccount},
    external_commands::{deploy_contracts, get_test_accounts, Contracts},
    zksync_account::ZkSyncAccount,
};

fn create_test_setup_state(
    testkit_config: &TestkitConfig,
    contracts: &Contracts,
    fee_account: &ZkSyncAccount,
) -> (EthereumAccount, AccountSet) {
    let transport = Http::new(&testkit_config.web3_url).expect("http transport start");
    let (test_accounts_info, commit_account_info) = get_test_accounts();
    let commit_account = EthereumAccount::new(
        commit_account_info.private_key,
        commit_account_info.address,
        transport.clone(),
        contracts.contract,
        testkit_config.chain_id,
        testkit_config.gas_price_factor,
    );
    let eth_accounts = test_accounts_info
        .into_iter()
        .map(|test_eth_account| {
            EthereumAccount::new(
                test_eth_account.private_key,
                test_eth_account.address,
                transport.clone(),
                contracts.contract,
                testkit_config.chain_id,
                testkit_config.gas_price_factor,
            )
        })
        .collect::<Vec<_>>();

    let zksync_accounts = {
        let mut zksync_accounts = vec![fee_account.clone()];
        zksync_accounts.extend(eth_accounts.iter().map(|eth_account| {
            let rng_zksync_key = ZkSyncAccount::rand().private_key;
            ZkSyncAccount::new(
                rng_zksync_key,
                Nonce(0),
                eth_account.address,
                ZkSyncETHAccountData::EOA {
                    eth_private_key: eth_account.private_key,
                },
            )
        }));
        zksync_accounts
    };

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts,
        fee_account_id: ZKSyncAccountId(0),
    };

    (commit_account, accounts)
}

async fn execute_blocks(
    test_setup: &mut TestSetup,
    start_block_number: BlockNumber,
    number_of_verified_iteration_blocks: u16, // Each operation generate 4 blocks
    number_of_committed_iteration_blocks: u16,
    number_of_reverted_iterations_blocks: u16,
) -> (ZkSyncStateInitParams, AccountSet, Block) {
    let deposit_amount = parse_ether("1.0").unwrap();

    let mut executed_blocks = Vec::new();
    let token = 0;
    let mut states = Vec::new();

    for _ in 0..number_of_verified_iteration_blocks {
        let blocks = perform_basic_operations(
            TokenId(token),
            test_setup,
            deposit_amount.clone(),
            BlockProcessing::CommitAndVerify,
        )
        .await;
        executed_blocks.extend(blocks.into_iter());
        states.push((
            test_setup.get_current_state().await,
            test_setup.accounts.clone(),
        ));
    }
    test_setup
        .get_eth_balance(ETHAccountId(0), TokenId(0))
        .await;
    for _ in 0..number_of_committed_iteration_blocks - number_of_verified_iteration_blocks {
        let blocks = perform_basic_operations(
            TokenId(token),
            test_setup,
            deposit_amount.clone(),
            BlockProcessing::NoVerify,
        )
        .await;
        executed_blocks.extend(blocks.into_iter());
        states.push((
            test_setup.get_current_state().await,
            test_setup.accounts.clone(),
        ));
    }
    test_setup
        .get_eth_balance(ETHAccountId(0), TokenId(0))
        .await;

    let executed_blocks_reverse_order = executed_blocks
        .clone()
        .into_iter()
        .rev()
        .take((number_of_reverted_iterations_blocks * 4) as usize)
        .collect::<Vec<_>>();

    let reverted_state_idx = std::cmp::max(
        number_of_verified_iteration_blocks,
        number_of_committed_iteration_blocks - number_of_reverted_iterations_blocks,
    ) - 1;
    let (reverted_state, test_setup_accounts) = states[reverted_state_idx as usize].clone();

    let executed_block = executed_blocks
        [(*reverted_state.last_block_number - *start_block_number - 1) as usize]
        .clone();

    test_setup
        .revert_blocks(&executed_blocks_reverse_order)
        .await
        .expect("revert_blocks call fails");

    (reverted_state, test_setup_accounts, executed_block)
}

fn balance_tree_to_account_map(balance_tree: &AccountTree) -> AccountMap {
    let mut account_map = AccountMap::default();
    for (id, account) in balance_tree.items.iter() {
        account_map.insert(AccountId(*id as u32), account.clone());
    }
    account_map
}

async fn revert_blocks_test() {
    let fee_account = ZkSyncAccount::rand();
    let test_config = TestkitConfig::from_env();

    let state = genesis_state(&fee_account.address);

    println!("deploying contracts");
    let contracts = deploy_contracts(false, state.tree.root_hash());
    println!("contracts deployed");

    let (commit_account, account_set) =
        create_test_setup_state(&test_config, &contracts, &fee_account);

    let hash = state.tree.root_hash();
    let (handler, sender, channels) = spawn_state_keeper(&fee_account.address, state);
    let mut test_setup = TestSetup::new(
        channels,
        account_set.clone(),
        &contracts,
        commit_account.clone(),
        hash,
        None,
    );

    // Verify 1
    // Commit 3
    // Revert 2
    // Revert all uncommitted transactions
    let (state, account_set, last_block) =
        execute_blocks(&mut test_setup, BlockNumber(0), 1, 3, 2).await;

    sender.send(()).expect("sk stop send");
    handler.join().expect("sk thread join");
    let hash = state.tree.root_hash();
    let start_block_number = state.last_block_number;

    let (handler, sender, channels) = spawn_state_keeper(&fee_account.address, state);

    let mut test_setup = TestSetup::new(
        channels,
        account_set.clone(),
        &contracts,
        commit_account.clone(),
        hash,
        Some(last_block),
    );

    // Verify 2
    // Commit 3
    // Revert 2
    // Try to revert some unverified blocks

    let (state, account_set, last_block) =
        execute_blocks(&mut test_setup, start_block_number, 2, 3, 2).await;
    sender.send(()).expect("sk stop send");
    handler.join().expect("sk thread join");

    let hash = state.tree.root_hash();
    let start_block_number = state.last_block_number;

    let (handler, sender, channels) = spawn_state_keeper(&fee_account.address, state);

    let mut test_setup = TestSetup::new(
        channels,
        account_set.clone(),
        &contracts,
        commit_account.clone(),
        hash,
        Some(last_block),
    );
    // Verify 1
    // Commit 1
    // Revert 0
    // Do not revert blocks for verifying restore

    let (state, _, _) = execute_blocks(&mut test_setup, start_block_number, 1, 1, 0).await;
    sender.send(()).expect("sk stop send");
    handler.join().expect("sk thread join");

    verify_restore(
        &test_config,
        &contracts,
        fee_account.address,
        balance_tree_to_account_map(&state.tree),
        vec![TokenId(0)],
        test_setup.current_state_root.unwrap(),
    )
    .await;
    println!("some blocks are committed and verified \n\n");
}

#[tokio::main]
async fn main() {
    revert_blocks_test().await;
}
