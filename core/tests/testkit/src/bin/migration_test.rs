use crate::eth_account::{parse_ether, EthereumAccount};
use crate::external_commands::{deploy_contracts, get_test_accounts, run_upgrade_franklin};
use crate::zksync_account::ZkSyncAccount;
use std::time::Instant;
use web3::transports::Http;
use zksync_testkit::scenarios::{perform_basic_operations, BlockProcessing};
use zksync_testkit::zksync_account::ZkSyncETHAccountData;
use zksync_testkit::*;
use zksync_types::{Nonce, TokenId};

async fn migration_test() {
    let testkit_config = TestkitConfig::from_env();

    let fee_account = ZkSyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address, genesis_state(&fee_account.address));
    let genesis_root = genesis_state(&fee_account.address).tree.root_hash();

    let deploy_timer = Instant::now();
    println!("deploying contracts");
    let contracts = deploy_contracts(false, genesis_root);
    println!(
        "contracts deployed {:#?}, {} secs",
        contracts,
        deploy_timer.elapsed().as_secs()
    );

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
        let mut zksync_accounts = vec![fee_account];
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

    let mut test_setup = TestSetup::new(
        sk_channels,
        accounts,
        &contracts,
        commit_account,
        genesis_root,
        None,
    );

    let deposit_amount = parse_ether("1.0").unwrap();

    let token = TokenId(0);
    perform_basic_operations(
        token,
        &mut test_setup,
        deposit_amount.clone(),
        BlockProcessing::CommitAndVerify,
    )
    .await;

    let start_upgrade = Instant::now();
    run_upgrade_franklin(contracts.contract, contracts.upgrade_gatekeeper);
    println!("Upgrade done in {:?}", start_upgrade.elapsed());

    let token = TokenId(1);
    perform_basic_operations(
        token,
        &mut test_setup,
        deposit_amount.clone(),
        BlockProcessing::CommitAndVerify,
    )
    .await;

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

#[tokio::main]
async fn main() {
    migration_test().await;
}
