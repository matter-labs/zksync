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
use crate::external_commands::{deploy_test_contracts, get_test_accounts};
use crate::zksync_account::ZksyncAccount;
use bigdecimal::BigDecimal;
use log::*;
use models::config_options::ConfigurationOptions;
use models::node::AccountMap;
use models::EncodedProof;
use std::time::{Duration, Instant};
use testkit::*;
use web3::transports::Http;

const PRIORITY_EXPIRATION: u64 = 16;

/// Using deposits from `deposit_accounts` creates initial state where each of the `zksync_account` have `deposit_amount`
/// of the `tokens` tokens.
fn create_verified_initial_state(
    test_setup: &mut TestSetup,
    deposit_account: ETHAccountId,
    deposit_amount: &BigDecimal,
    tokens: &[Token],
    zksync_accounts: &[ZKSyncAccountId],
) {
    info!("Creating initial state");
    for token in tokens {
        test_setup.start_block();
        for account in zksync_accounts {
            test_setup.deposit(deposit_account, *account, *token, deposit_amount.clone());
        }
        test_setup
            .execute_commit_and_verify_block()
            .expect("Commit and verify initial block");
    }
    info!("Done creating initial state");
}

// Commits deposit that has to fail, returns block close to the block where deposit was committed.
fn commit_deposit_to_expire(
    test_setup: &mut TestSetup,
    from: ETHAccountId,
    to: ZKSyncAccountId,
    token: Token,
    deposit_amount: &BigDecimal,
) -> u64 {
    info!("Commit deposit to expire");
    test_setup.start_block();
    test_setup.deposit(from, to, token, deposit_amount.clone());
    let reason = test_setup
        .execute_commit_block()
        .expect("commit expired deposit fail");
    assert_eq!(
        reason,
        String::from("tx success"),
        "Expired deposit commit should succeed."
    );

    info!("Done commit deposit to expire");
    test_setup.eth_block_number()
}

// Trigger exodus mode using `eth_account`, it is preferred to use not operator account for this
fn trigger_exodus(
    test_setup: &TestSetup,
    eth_account: ETHAccountId,
    expire_count_start_block: u64,
) {
    info!("Triggering exodus");
    let is_exodus = test_setup.is_exodus();
    assert!(!is_exodus, "Exodus should be triggered later");

    while test_setup.eth_block_number() - expire_count_start_block < PRIORITY_EXPIRATION {
        std::thread::sleep(Duration::from_millis(500));
    }

    test_setup.trigger_exodus_if_needed(eth_account);

    let is_exodus = test_setup.is_exodus();
    assert!(is_exodus, "Exodus should be triggered after expiration");
    info!("Done triggering exodus");
}

fn cancel_outstanding_deposits(
    test_setup: &TestSetup,
    deposit_account: ETHAccountId,
    deposit_token: Token,
    deposit_amount: &BigDecimal,
    call_cancel_account: ETHAccountId,
) {
    info!("Canceling outstangind deposits");
    let balance_to_withdraw_before =
        test_setup.get_balance_to_withdraw(deposit_account, deposit_token);

    test_setup.cancel_outstanding_deposits(call_cancel_account);

    let balance_to_withdraw_after =
        test_setup.get_balance_to_withdraw(deposit_account, deposit_token);

    assert_eq!(
        balance_to_withdraw_before + deposit_amount,
        balance_to_withdraw_after,
        "Balances after deposit cancel is not correct"
    );
    info!("Done canceling outstangind deposits");
}

fn check_exit_garbage_proof(
    test_setup: &mut TestSetup,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigDecimal,
) {
    info!(
        "Checking exit with garbage proof token: {}, amount: {}",
        token.0, amount
    );
    let proof = EncodedProof::default();
    test_setup
        .exit(send_account, fund_owner, token, amount, proof)
        .expect_revert("vvy14");
    info!("Done cheching exit with garbage proof");
}

fn check_exit_correct_proof(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigDecimal,
) {
    info!("Checking exit with correct proof");
    let balance_to_withdraw_before = test_setup.get_balance_to_withdraw(send_account, token);

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    test_setup
        .exit(send_account, fund_owner, token, &exit_amount, proof)
        .expect_success();

    let balance_to_withdraw_after = test_setup.get_balance_to_withdraw(send_account, token);

    assert_eq!(
        balance_to_withdraw_before + exit_amount,
        balance_to_withdraw_after,
        "Balance to withdraw is not incremented"
    );
    info!("Done checking exit with correct proof");
}

fn check_exit_correct_proof_second_time(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigDecimal,
) {
    info!("Checking exit with correct proof twice");
    let balance_to_withdraw_before = test_setup.get_balance_to_withdraw(send_account, token);

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    test_setup
        .exit(send_account, fund_owner, token, &exit_amount, proof)
        .expect_revert("fet12");

    let balance_to_withdraw_after = test_setup.get_balance_to_withdraw(send_account, token);

    assert_eq!(
        balance_to_withdraw_before, balance_to_withdraw_after,
        "Balance to withdraw is incremented"
    );
    info!("Done checking exit with correct proof twice");
}

fn check_exit_correct_proof_other_account(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigDecimal,
    false_owner: ZKSyncAccountId,
) {
    info!("Checking exit with correct proof for other account");
    let balance_to_withdraw_before = test_setup.get_balance_to_withdraw(send_account, token);

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    test_setup
        .exit(send_account, false_owner, token, &exit_amount, proof)
        .expect_revert("fet13");

    let balance_to_withdraw_after = test_setup.get_balance_to_withdraw(send_account, token);

    assert_eq!(
        balance_to_withdraw_before, balance_to_withdraw_after,
        "Balance to withdraw is incremented"
    );
    info!("Done checking exit with correct proof for other account");
}

fn check_exit_correct_proof_other_token(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigDecimal,
    false_token: Token,
) {
    info!("Checking exit with correct proof other token");
    let balance_to_withdraw_before = test_setup.get_balance_to_withdraw(send_account, token);

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    test_setup
        .exit(send_account, fund_owner, false_token, &exit_amount, proof)
        .expect_revert("fet13");

    let balance_to_withdraw_after = test_setup.get_balance_to_withdraw(send_account, token);

    assert_eq!(
        balance_to_withdraw_before, balance_to_withdraw_after,
        "Balance to withdraw is incremented"
    );
    info!("Done checking exit with correct proof other token");
}

fn check_exit_correct_proof_other_amount(
    test_setup: &mut TestSetup,
    accounts: AccountMap,
    send_account: ETHAccountId,
    fund_owner: ZKSyncAccountId,
    token: Token,
    amount: &BigDecimal,
    false_amount: &BigDecimal,
) {
    info!("Checking exit with correct proof other amount");
    let balance_to_withdraw_before = test_setup.get_balance_to_withdraw(send_account, token);

    let (proof, exit_amount) = test_setup.gen_exit_proof(accounts, fund_owner, token);
    assert_eq!(
        &exit_amount, amount,
        "Exit proof generated with unexpected amount"
    );
    test_setup
        .exit(send_account, fund_owner, token, false_amount, proof)
        .expect_revert("fet13");

    let balance_to_withdraw_after = test_setup.get_balance_to_withdraw(send_account, token);

    assert_eq!(
        balance_to_withdraw_before, balance_to_withdraw_after,
        "Balance to withdraw is incremented"
    );
    info!("Done checking exit with correct proof other amount");
}

fn exit_test() {
    env_logger::init();

    let config = ConfigurationOptions::from_env();

    let fee_account = ZksyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address);

    let deploy_timer = Instant::now();
    info!("deploying contracts");
    let contracts = deploy_test_contracts();
    info!(
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

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts: vec![fee_account, ZksyncAccount::rand()],
        fee_account_id: ZKSyncAccountId(0),
    };

    let mut test_setup = TestSetup::new(sk_channels, accounts, &contracts, commit_account);

    let deposit_amount = parse_ether("0.1").unwrap();

    let test_accounts = vec![ZKSyncAccountId(0), ZKSyncAccountId(1)];
    let tokens = test_setup.get_tokens();

    create_verified_initial_state(
        &mut test_setup,
        ETHAccountId(0),
        &deposit_amount,
        &tokens,
        &test_accounts,
    );
    let verified_accounts_state = test_setup.get_accounts_state();
    println!("{:#?}", verified_accounts_state);

    let expire_count_start_block = commit_deposit_to_expire(
        &mut test_setup,
        ETHAccountId(0),
        ZKSyncAccountId(0),
        Token(0),
        &deposit_amount,
    );
    trigger_exodus(&test_setup, ETHAccountId(1), expire_count_start_block);
    cancel_outstanding_deposits(
        &test_setup,
        ETHAccountId(0),
        Token(0),
        &deposit_amount,
        ETHAccountId(1),
    );

    check_exit_correct_proof_other_account(
        &mut test_setup,
        verified_accounts_state.clone(),
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
        ZKSyncAccountId(0),
    );
    check_exit_correct_proof_other_token(
        &mut test_setup,
        verified_accounts_state.clone(),
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
        Token(1),
    );
    let incorrect_amount = BigDecimal::from(2) * deposit_amount.clone();
    check_exit_correct_proof_other_amount(
        &mut test_setup,
        verified_accounts_state.clone(),
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
        &incorrect_amount,
    );
    check_exit_garbage_proof(
        &mut test_setup,
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
    );

    check_exit_correct_proof(
        &mut test_setup,
        verified_accounts_state.clone(),
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
    );

    check_exit_correct_proof_second_time(
        &mut test_setup,
        verified_accounts_state,
        ETHAccountId(1),
        ZKSyncAccountId(1),
        Token(0),
        &deposit_amount,
    );

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

fn main() {
    exit_test();
}
