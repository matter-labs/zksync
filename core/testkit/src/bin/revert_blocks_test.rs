use crate::eth_account::{parse_ether, EthereumAccount};
use crate::external_commands::{
    deploy_test_contracts, get_test_accounts, revert_all_not_verified_blocks, Contracts,
};
use crate::zksync_account::ZksyncAccount;
use std::time::Instant;
use testkit::*;
use web3::transports::Http;

fn run_some_operations(contracts: Contracts, verify_blocks: bool) {
    let testkit_config = get_testkit_config_from_env();

    let fee_account = ZksyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address);

    let (_el, transport) = Http::new(&testkit_config.web3_url).expect("http transport start");
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
        if verify_blocks {
            perform_basic_operations(token, &mut test_setup, deposit_amount.clone());
        } else {
            perform_some_operations_no_verify(token, &mut test_setup, deposit_amount.clone());
        }
    }

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

fn revert_blocks_test() {
    let deploy_timer = Instant::now();
    println!("deploying contracts");
    let contracts = deploy_test_contracts();
    println!(
        "contracts deployed {:#?}, {} secs",
        contracts,
        deploy_timer.elapsed().as_secs()
    );

    run_some_operations(contracts.clone(), false);
    println!("some blocks are committed");

    revert_all_not_verified_blocks(contracts.contract);
    println!("blocks reverted");

    run_some_operations(contracts, true);
}

fn main() {
    revert_blocks_test();
}
