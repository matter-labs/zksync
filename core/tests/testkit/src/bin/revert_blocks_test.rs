use crate::eth_account::{parse_ether, EthereumAccount};
use crate::external_commands::{deploy_contracts, get_test_accounts, Contracts};
use crate::zksync_account::ZkSyncAccount;
use itertools::Itertools;
use std::thread::JoinHandle;
use web3::transports::Http;
use zksync_core::state_keeper::ZkSyncStateInitParams;
use zksync_testkit::data_restore::verify_restore;
use zksync_testkit::scenarios::{perform_basic_operations, BlockProcessing};
use zksync_testkit::*;
use zksync_types::block::Block;
use zksync_types::{AccountMap, AccountTree, PriorityOp};

fn create_test_setup_state(
    testkit_config: &TestkitConfig,
    contracts: &Contracts,
    fee_account: &ZkSyncAccount,
) -> (EthereumAccount<Http>, AccountSet<Http>) {
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
        let mut zksync_accounts = Vec::new();
        zksync_accounts.push(fee_account.clone());
        zksync_accounts.extend(eth_accounts.iter().map(|eth_account| {
            let rng_zksync_key = ZkSyncAccount::rand().private_key;
            ZkSyncAccount::new(
                rng_zksync_key,
                0,
                eth_account.address,
                eth_account.private_key,
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
    contracts: &Contracts,
    fee_account: &ZkSyncAccount,
    test_setup: &mut TestSetup,
    testkit_config: &TestkitConfig,
    number_of_verified_iteration_blocks: u16, // Each operation generate 4 blocks
    number_of_committed_iteration_blocks: u16,
    number_of_reverted_iterations_blocks: u16,
) -> (ZkSyncStateInitParams, Block, Vec<PriorityOp>) {
    let deposit_amount = parse_ether("1.0").unwrap();

    let mut executed_blocks = Vec::new();
    let token = 0;
    let mut priority_ops_states = Vec::new();
    let mut states = Vec::new();

    for _ in 0..number_of_verified_iteration_blocks {
        let (blocks, priority_ops) = perform_basic_operations(
            token,
            test_setup,
            deposit_amount.clone(),
            BlockProcessing::CommitAndVerify,
        )
        .await;
        executed_blocks.extend(blocks.into_iter());
        states.push(test_setup.get_current_state().await);
        priority_ops_states.push(priority_ops);
    }
    for _ in 0..number_of_committed_iteration_blocks - number_of_verified_iteration_blocks {
        let (blocks, priority_ops) = perform_basic_operations(
            token,
            test_setup,
            deposit_amount.clone(),
            BlockProcessing::NoVerify,
        )
        .await;
        executed_blocks.extend(blocks.into_iter());
        states.push(test_setup.get_current_state().await);
        priority_ops_states.push(priority_ops);
    }

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
    let reverted_state = states[reverted_state_idx as usize].clone();

    let executed_block = executed_blocks[(reverted_state.last_block_number - 1) as usize].clone();
    let priority_ops = priority_ops_states
        .into_iter()
        .rev()
        .take((number_of_reverted_iterations_blocks - 1) as usize)
        .rev()
        .concat();

    test_setup
        .revert_blocks(&executed_blocks_reverse_order)
        .await
        .expect("revert_blocks call fails");

    verify_restore(
        &testkit_config.web3_url,
        testkit_config.available_block_chunk_sizes.clone(),
        contracts,
        fee_account.address,
        balance_tree_to_account_map(&reverted_state.tree),
        vec![token],
        test_setup.current_state_root.unwrap(), // executed_blocks.last().unwrap().new_root_hash,
    )
    .await;

    (reverted_state, executed_block, priority_ops)
}

fn balance_tree_to_account_map(balance_tree: &AccountTree) -> AccountMap {
    let mut account_map = AccountMap::default();
    for (id, account) in balance_tree.items.iter() {
        account_map.insert(*id as u32, account.clone());
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

    let (state, block, priority_ops) = execute_blocks(
        &contracts,
        &fee_account,
        &mut test_setup,
        &test_config,
        1,
        2,
        1,
    )
    .await;
    sender.send(()).expect("sk stop send");
    handler.join().expect("sk thread join");

    let hash = state.tree.root_hash();
    let (handler, sender, channels) = spawn_state_keeper(&fee_account.address, state);
    let account_set = test_setup.accounts;
    let mut test_setup = TestSetup::new(
        channels,
        account_set,
        &contracts,
        commit_account,
        hash,
        Some(block),
    );
    for op in priority_ops.into_iter() {
        test_setup.execute_priority_op(op).await
    }
    let state = execute_blocks(
        &contracts,
        &fee_account,
        &mut test_setup,
        &test_config,
        1,
        2,
        0,
    )
    .await;
    sender.send(()).expect("sk stop send");
    handler.join().expect("sk thread join");

    println!("some blocks are committed and verified \n\n");
}

#[tokio::main]
async fn main() {
    revert_blocks_test().await;
}
