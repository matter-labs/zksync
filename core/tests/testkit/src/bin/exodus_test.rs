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
use crate::external_commands::{deploy_contracts, get_test_accounts};
use crate::zksync_account::ZkSyncAccount;
use log::*;
use num::BigUint;
use std::time::Instant;
use web3::transports::Http;
use zksync_crypto::proof::EncodedProofPlonk;
use zksync_testkit::*;
use zksync_types::{AccountId, AccountMap};

const PRIORITY_EXPIRATION: u64 = 101;

/// Using deposits from `deposit_accounts` creates initial state where each of the `zksync_account` have `deposit_amount`
/// of the `tokens` tokens.
async fn create_verified_initial_state(
    test_setup: &mut TestSetup,
    deposit_account: ETHAccountId,
    deposit_amount: &BigUint,
    tokens: &[Token],
    zksync_accounts: &[ZKSyncAccountId],
) {
    info!("Creating initial state");
    test_setup.start_block();
    for token in tokens {
        for account in zksync_accounts {
            test_setup
                .deposit(deposit_account, *account, *token, deposit_amount.clone())
                .await;
        }
    }
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Commit and verify initial block");
    info!("Done creating initial state");
}

// Commits deposit that has to fail, returns block close to the block where deposit was committed.
async fn commit_deposit_to_expire(
    test_setup: &mut TestSetup,
    from: ETHAccountId,
    to: ZKSyncAccountId,
    token: Token,
    deposit_amount: &BigUint,
) -> u64 {
    info!("Commit deposit to expire");
    test_setup.start_block();
    test_setup
        .deposit(from, to, token, deposit_amount.clone())
        .await;
    test_setup.execute_commit_block().await.0.expect_success();

    info!("Done commit deposit to expire");
    test_setup.eth_block_number().await
}

// Trigger exodus mode using `eth_account`, it is preferred to use not operator account for this
async fn trigger_exodus(
    test_setup: &TestSetup,
    eth_account: ETHAccountId,
    expire_count_start_block: u64,
) {
    info!("Triggering exodus");
    let is_exodus = test_setup.is_exodus().await;
    assert!(!is_exodus, "Exodus should be triggered later");

    while test_setup.eth_block_number().await - expire_count_start_block < PRIORITY_EXPIRATION {
        test_setup.trigger_exodus_if_needed(eth_account).await;
    }

    test_setup.trigger_exodus_if_needed(eth_account).await;

    let is_exodus = test_setup.is_exodus().await;
    assert!(is_exodus, "Exodus should be triggered after expiration");
    info!("Done triggering exodus");
}

async fn cancel_outstanding_deposits(
    test_setup: &TestSetup,
    deposit_receiver_account: ETHAccountId,
    deposit_token: Token,
    deposit_amount: &BigUint,
    call_cancel_account: ETHAccountId,
) {
    info!("Canceling outstangind deposits");
    let balance_to_withdraw_before = test_setup
        .get_balance_to_withdraw(deposit_receiver_account, deposit_token)
        .await;

    test_setup
        .cancel_outstanding_deposits(call_cancel_account)
        .await;

    let balance_to_withdraw_after = test_setup
        .get_balance_to_withdraw(deposit_receiver_account, deposit_token)
        .await;

    assert_eq!(
        balance_to_withdraw_before + deposit_amount,
        balance_to_withdraw_after,
        "Balances after deposit cancel is not correct"
    );
    info!("Done canceling outstanging deposits");
}

async fn check_exit_garbage_proof(
    test_setup: &mut TestSetup,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigUint,
) {
    info!(
        "Checking exit with garbage proof token: {}, amount: {}",
        token.0, amount
    );
    let proof = EncodedProofPlonk::default();
    test_setup
        .exit(
            send_account,
            fund_owner.0 as AccountId,
            token,
            amount,
            proof,
        )
        .await
        .expect_revert("fet13");
    info!("Done cheching exit with garbage proof");
}

async fn check_exit_correct_proof(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigUint,
) {
    info!("Checking exit with correct proof");
    let balance_to_withdraw_before = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    assert_eq!(
        test_setup.accounts.zksync_accounts[fund_owner.0].address,
        test_setup.accounts.eth_accounts[send_account.0].address,
        "Sender should have same address",
    );
    let account_id = test_setup
        .get_zksync_account_committed_state(fund_owner)
        .await
        .expect("Account should exits")
        .0;
    test_setup
        .exit(send_account, account_id, token, &exit_amount, proof)
        .await
        .expect_success();

    let balance_to_withdraw_after = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    assert_eq!(
        balance_to_withdraw_before + exit_amount,
        balance_to_withdraw_after,
        "Balance to withdraw is not incremented"
    );
    info!("Done checking exit with correct proof");
}

async fn check_exit_correct_proof_second_time(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigUint,
) {
    info!("Checking exit with correct proof twice");
    let balance_to_withdraw_before = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    let account_id = test_setup
        .get_zksync_account_committed_state(fund_owner)
        .await
        .expect("Account should exits")
        .0;
    test_setup
        .exit(send_account, account_id, token, &exit_amount, proof)
        .await
        .expect_revert("fet12");

    let balance_to_withdraw_after = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    assert_eq!(
        balance_to_withdraw_before, balance_to_withdraw_after,
        "Balance to withdraw is incremented"
    );
    info!("Done checking exit with correct proof twice");
}

async fn check_exit_correct_proof_other_token(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigUint,
    false_token: Token,
) {
    info!("Checking exit with correct proof other token");
    let balance_to_withdraw_before = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    let account_id = test_setup
        .get_zksync_account_committed_state(fund_owner)
        .await
        .expect("Account should exits")
        .0;
    test_setup
        .exit(send_account, account_id, false_token, &exit_amount, proof)
        .await
        .expect_revert("fet13");

    let balance_to_withdraw_after = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    assert_eq!(
        balance_to_withdraw_before, balance_to_withdraw_after,
        "Balance to withdraw is incremented"
    );
    info!("Done checking exit with correct proof other token");
}

async fn check_exit_correct_proof_other_amount(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigUint,
    false_amount: &BigUint,
) {
    info!("Checking exit with correct proof other amount");
    let balance_to_withdraw_before = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    let account_id = test_setup
        .get_zksync_account_committed_state(fund_owner)
        .await
        .expect("Account should exits")
        .0;
    test_setup
        .exit(send_account, account_id, token, false_amount, proof)
        .await
        .expect_revert("fet13");

    let balance_to_withdraw_after = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    assert_eq!(
        balance_to_withdraw_before, balance_to_withdraw_after,
        "Balance to withdraw is incremented"
    );
    info!("Done checking exit with correct proof other amount");
}

async fn check_exit_correct_proof_incorrect_sender(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigUint,
) {
    info!("Checking exit with correct proof and incorrect sender");
    let balance_to_withdraw_before = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    let account_id = test_setup
        .get_zksync_account_committed_state(fund_owner)
        .await
        .expect("Account should exits")
        .0;
    test_setup
        .exit(send_account, account_id, token, &exit_amount, proof)
        .await
        .expect_revert("fet13");

    let balance_to_withdraw_after = test_setup
        .get_balance_to_withdraw(send_account, token)
        .await;

    assert_eq!(
        balance_to_withdraw_before, balance_to_withdraw_after,
        "Balance to withdraw is incremented"
    );
    info!("Done checking exit with correct proof and incorrect sender");
}

async fn exit_test() {
    env_logger::init();
    let testkit_config = get_testkit_config_from_env();

    let fee_account = ZkSyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address);

    let deploy_timer = Instant::now();
    info!("deploying contracts");
    let contracts = deploy_contracts(false, Default::default());
    info!(
        "contracts deployed {:#?}, {} secs",
        contracts,
        deploy_timer.elapsed().as_secs()
    );

    let transport = Http::new(&testkit_config.web3_url).expect("http transport start");

    let (test_accounts_info, commit_account_info) = get_test_accounts();
    let test_accounts_info = test_accounts_info[0..2].to_vec();
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

    let (zksync_accounts, fee_account_id) = {
        let mut zksync_accounts = Vec::new();
        zksync_accounts.extend(eth_accounts.iter().map(|eth_account| {
            let rng_zksync_key = ZkSyncAccount::rand().private_key;
            ZkSyncAccount::new(
                rng_zksync_key,
                0,
                eth_account.address,
                eth_account.private_key,
            )
        }));
        zksync_accounts.push(fee_account);
        let fee_account_id = zksync_accounts.len() - 1;
        (zksync_accounts, fee_account_id)
    };

    let test_accounts = (0..zksync_accounts.len())
        .map(ZKSyncAccountId)
        .collect::<Vec<_>>();

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts,
        fee_account_id: ZKSyncAccountId(fee_account_id),
    };

    let mut test_setup = TestSetup::new(sk_channels, accounts, &contracts, commit_account);

    let deposit_amount = parse_ether("0.1").unwrap();
    let tokens = test_setup.get_tokens();

    create_verified_initial_state(
        &mut test_setup,
        ETHAccountId(0),
        &deposit_amount,
        &tokens,
        &test_accounts,
    )
    .await;
    let verified_accounts_state = test_setup.get_accounts_state().await;

    let expired_deposit_amount = parse_ether("0.3").unwrap();
    let expire_count_start_block = commit_deposit_to_expire(
        &mut test_setup,
        ETHAccountId(0),
        ZKSyncAccountId(1),
        Token(0),
        &expired_deposit_amount,
    )
    .await;
    trigger_exodus(&test_setup, ETHAccountId(1), expire_count_start_block).await;
    cancel_outstanding_deposits(
        &test_setup,
        ETHAccountId(1),
        Token(0),
        &expired_deposit_amount,
        ETHAccountId(1),
    )
    .await;

    check_exit_correct_proof_other_token(
        &mut test_setup,
        verified_accounts_state.clone(),
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
        Token(1),
    )
    .await;
    let incorrect_amount = BigUint::from(2u32) * deposit_amount.clone();
    check_exit_correct_proof_other_amount(
        &mut test_setup,
        verified_accounts_state.clone(),
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
        &incorrect_amount,
    )
    .await;

    check_exit_garbage_proof(
        &mut test_setup,
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
    )
    .await;

    check_exit_correct_proof_incorrect_sender(
        &mut test_setup,
        verified_accounts_state.clone(),
        ETHAccountId(0),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
    )
    .await;

    check_exit_correct_proof(
        &mut test_setup,
        verified_accounts_state.clone(),
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
    )
    .await;

    check_exit_correct_proof_second_time(
        &mut test_setup,
        verified_accounts_state,
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
    )
    .await;

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

#[tokio::main]
async fn main() {
    exit_test().await;
}
