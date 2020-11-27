//! Common scenarios used by testkit derivatives.

use num::BigUint;
use std::time::Instant;
use web3::transports::Http;

use crate::{
    data_restore::verify_restore,
    eth_account::{parse_ether, EthereumAccount},
    external_commands::{deploy_contracts, get_test_accounts},
    state_keeper_utils::spawn_state_keeper,
    zksync_account::ZkSyncAccount,
};

use super::*;

/// Performs a fixed set of operations which covers most of the main server's functionality.
/// Aim is to cover operations processed by state keeper, while manually simulating everything else around it.
pub async fn perform_basic_tests() {
    // This test is actually nowhere near "basic", and deserves a careful refactoring, but
    // ain't nobody got time for that ¯\_(ツ)_/¯

    let testkit_config = TestkitConfig::from_env();

    let fee_account = ZkSyncAccount::rand();
    let fee_account_address = fee_account.address;
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account_address);

    let deploy_timer = Instant::now();
    println!("deploying contracts");
    let contracts = deploy_contracts(false, Default::default());
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
        let mut zksync_accounts = Vec::new();
        zksync_accounts.push(fee_account);
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

    let mut test_setup = TestSetup::new(sk_channels, accounts, &contracts, commit_account);

    let deposit_amount = parse_ether("1.0").unwrap();

    let mut tokens = vec![];
    for token in 0..=1 {
        perform_basic_operations(
            token,
            &mut test_setup,
            deposit_amount.clone(),
            BlockProcessing::CommitAndVerify,
        )
        .await;
        tokens.push(token);
    }

    verify_restore(
        &testkit_config.web3_url,
        testkit_config.available_block_chunk_sizes.clone(),
        &contracts,
        fee_account_address,
        test_setup.get_accounts_state().await,
        tokens,
        test_setup.current_state_root.expect("Should exist"),
    )
    .await;

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum BlockProcessing {
    CommitAndVerify,
    NoVerify,
}

pub async fn perform_basic_operations(
    token: u16,
    test_setup: &mut TestSetup,
    deposit_amount: BigUint,
    blocks_processing: BlockProcessing,
) {
    // test deposit to other account
    test_setup.start_block();
    test_setup
        .deposit(
            ETHAccountId(0),
            ZKSyncAccountId(2),
            Token(token),
            deposit_amount.clone(),
        )
        .await;
    if blocks_processing == BlockProcessing::CommitAndVerify {
        test_setup
            .execute_commit_and_verify_block()
            .await
            .expect("Block execution failed");
        println!("Deposit to other account test success, token_id: {}", token);
    } else {
        test_setup.execute_commit_block().await.0.expect_success();
    }

    // test two deposits
    test_setup.start_block();
    test_setup
        .deposit(
            ETHAccountId(0),
            ZKSyncAccountId(1),
            Token(token),
            deposit_amount.clone(),
        )
        .await;
    test_setup
        .deposit(
            ETHAccountId(0),
            ZKSyncAccountId(1),
            Token(token),
            deposit_amount.clone(),
        )
        .await;
    if blocks_processing == BlockProcessing::CommitAndVerify {
        test_setup
            .execute_commit_and_verify_block()
            .await
            .expect("Block execution failed");
        println!("Deposit test success, token_id: {}", token);
    } else {
        test_setup.execute_commit_block().await.0.expect_success();
    }

    // test transfers
    test_setup.start_block();

    if blocks_processing == BlockProcessing::CommitAndVerify {
        test_setup
            .change_pubkey_with_onchain_auth(
                ETHAccountId(0),
                ZKSyncAccountId(1),
                Token(token),
                0u32.into(),
            )
            .await;
    } else {
        test_setup
            .change_pubkey_with_tx(ZKSyncAccountId(1), Token(token), 0u32.into())
            .await;
    }

    //transfer to self should work
    test_setup
        .transfer(
            ZKSyncAccountId(1),
            ZKSyncAccountId(1),
            Token(token),
            &deposit_amount / BigUint::from(8u32),
            &deposit_amount / BigUint::from(8u32),
        )
        .await;

    //should be executed as a transfer
    test_setup
        .transfer(
            ZKSyncAccountId(1),
            ZKSyncAccountId(2),
            Token(token),
            &deposit_amount / BigUint::from(8u32),
            &deposit_amount / BigUint::from(8u32),
        )
        .await;

    let nonce = test_setup.accounts.zksync_accounts[1].nonce();
    let incorrect_nonce_transfer = test_setup.accounts.transfer(
        ZKSyncAccountId(1),
        ZKSyncAccountId(0),
        Token(token),
        deposit_amount.clone(),
        BigUint::from(0u32),
        Some(nonce + 1),
        false,
    );
    test_setup
        .execute_incorrect_tx(incorrect_nonce_transfer)
        .await;

    //should be executed as a transfer to new
    test_setup
        .transfer(
            ZKSyncAccountId(1),
            ZKSyncAccountId(2),
            Token(token),
            &deposit_amount / BigUint::from(4u32),
            &deposit_amount / BigUint::from(4u32),
        )
        .await;

    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(2), Token(token), 0u32.into())
        .await;

    test_setup
        .withdraw(
            ZKSyncAccountId(2),
            ETHAccountId(0),
            Token(token),
            &deposit_amount / BigUint::from(4u32),
            &deposit_amount / BigUint::from(4u32),
        )
        .await;
    if blocks_processing == BlockProcessing::CommitAndVerify {
        test_setup
            .execute_commit_and_verify_block()
            .await
            .expect("Block execution failed");
        println!("Transfer test success, token_id: {}", token);
    } else {
        test_setup.execute_commit_block().await.0.expect_success();
    }

    test_setup.start_block();
    test_setup
        .full_exit(ETHAccountId(0), ZKSyncAccountId(1), Token(token))
        .await;
    if blocks_processing == BlockProcessing::CommitAndVerify {
        test_setup
            .execute_commit_and_verify_block()
            .await
            .expect("Block execution failed");
    } else {
        test_setup.execute_commit_block().await.0.expect_success();
    }
}
