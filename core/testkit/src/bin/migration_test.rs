//! Exodus mode test steps:
//! + Create verified state with balances on the accounts.
//! + Commit some deposits and wait for priority expiration.
//! + Check exodus mode triggered.
//! + Check canceling of the outstanding deposits.
//! + Check exit with correct proof.
//! + Check double exit with the correct proof.
//! + Check exit with garbage proof.
//! + Check exit with correct proof for other account, correct proof for this account but other token, correct proof but wrong amount.

use crate::eth_account::{parse_ether, EthereumAccount};
use crate::external_commands::{deploy_test_contracts, get_test_accounts, run_upgrade_franklin};
use crate::zksync_account::ZksyncAccount;
use models::config_options::ConfigurationOptions;
use std::time::Instant;
use testkit::*;
use web3::transports::Http;

fn migration_test() {
    let config = ConfigurationOptions::from_env();

    let fee_account = ZksyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address);

    let deploy_timer = Instant::now();
    println!("deploying contracts");
    let contracts = deploy_test_contracts();
    println!(
        "contracts deployed {:#?}, {} secs",
        contracts,
        deploy_timer.elapsed().as_secs()
    );

    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let commit_account = EthereumAccount::new(
        config.operator_private_key,
        config.operator_eth_addr,
        transport.clone(),
        contracts.contract,
        &config,
    );

    let eth_accounts = get_test_accounts()
        .into_iter()
        .map(|test_eth_account| {
            EthereumAccount::new(
                test_eth_account.private_key,
                test_eth_account.address,
                transport.clone(),
                contracts.contract,
                &config,
            )
        })
        .collect::<Vec<_>>();

    let zksync_accounts = {
        let mut zksync_accounts = Vec::new();
        zksync_accounts.push(fee_account);
        zksync_accounts.extend(eth_accounts.iter().map(|eth_account| {
            let rng_zksync_key = ZksyncAccount::rand().private_key;
            ZksyncAccount::new(
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

    let mut test_setup = TestSetup::new(sk_channels, accounts, &contracts, commit_account);

    let deposit_amount = parse_ether("1.0").unwrap();

    for token in 0..=1 {
        perform_basic_operations(token, &mut test_setup, deposit_amount.clone());
    }

    let start_upgrade = Instant::now();
    run_upgrade_franklin(contracts.contract, contracts.upgrade_gatekeeper);
    println!("Upgrade done in {:?}", start_upgrade.elapsed());

    for token in 0..=1 {
        perform_basic_operations(token, &mut test_setup, deposit_amount.clone());
    }

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

fn main() {
    migration_test();
}
