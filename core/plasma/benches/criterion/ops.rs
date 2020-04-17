//! Benchmarks for the `PlasmaState` operations execution time.

// Built-in deps
use std::collections::HashMap;
// External uses
use criterion::{black_box, criterion_group, BatchSize, Bencher, Criterion, Throughput};
use web3::types::H256;
// Workspace uses
use crypto_exports::rand::{thread_rng, Rng};
use models::node::{
    account::{Account, PubKeyHash},
    priority_ops::{Deposit, FullExit},
    priv_key_from_fs,
    tx::{ChangePubKey, PackedEthSignature, Transfer, Withdraw},
    AccountId, AccountMap, Address, BlockNumber, FranklinPriorityOp, FranklinTx, PrivateKey,
    TokenId,
};
// Local uses
use plasma::state::PlasmaState;

const ETH_TOKEN_ID: TokenId = 0x00;
// The amount is not important, since we always work with 1 account.
// We use some small non-zero value, so the overhead for cloning will not be big.
const ACCOUNTS_AMOUNT: AccountId = 10;
const CURRENT_BLOCK: BlockNumber = 1_000;

/// Creates a random ZKSync account.
fn generate_account() -> (H256, PrivateKey, Account) {
    let default_balance = 1_000_000.into();

    let rng = &mut thread_rng();
    let sk = priv_key_from_fs(rng.gen());

    let eth_sk = H256::random();
    let address = PackedEthSignature::address_from_private_key(&eth_sk)
        .expect("Can't get address from the ETH secret key");

    let mut account = Account::default();
    account.pub_key_hash = PubKeyHash::from_privkey(&sk);
    account.address = address;
    account.set_balance(ETH_TOKEN_ID, default_balance);

    (eth_sk, sk, account)
}

/// Creates a `PlasmaState` object and fills it with accounts.
fn generate_state() -> (HashMap<AccountId, (PrivateKey, H256)>, PlasmaState) {
    let mut accounts = AccountMap::default();
    let mut keys = HashMap::new();

    for account_id in 0..ACCOUNTS_AMOUNT {
        let (eth_sk, sk, new_account) = generate_account();

        accounts.insert(account_id, new_account);
        keys.insert(account_id, (sk, eth_sk));
    }

    let state = PlasmaState::new(accounts, CURRENT_BLOCK);

    (keys, state)
}

/// Bench for `PlasmaState::apply_transfer_to_new_op`.
fn apply_transfer_to_new_op(b: &mut Bencher<'_>) {
    let (keys, state) = generate_state();
    let (private_key, _) = keys.get(&0).expect("Can't key the private key");

    let from_account = state.get_account(0).expect("Can't get the account");

    let transfer = Transfer::new_signed(
        from_account.address,
        Address::random(),
        ETH_TOKEN_ID,
        10.into(),
        1.into(),
        0,
        private_key,
    )
    .expect("failed to sign transfer");
    let transfer_tx = FranklinTx::Transfer(Box::new(transfer));

    let setup = || (state.clone(), transfer_tx.clone());

    b.iter_batched(
        setup,
        |(mut state, transfer_tx)| {
            state
                .execute_tx(black_box(transfer_tx))
                .expect("Failed to execute tx");
        },
        BatchSize::SmallInput,
    );
}

/// Bench for `PlasmaState::apply_transfer_op`.
fn apply_transfer_op(b: &mut Bencher<'_>) {
    let (keys, state) = generate_state();
    let (private_key, _) = keys.get(&0).expect("Can't key the private key");

    let from_account = state.get_account(0).expect("Can't get the account");
    let to_account = state.get_account(1).expect("Can't get the account");

    let transfer = Transfer::new_signed(
        from_account.address,
        to_account.address,
        ETH_TOKEN_ID,
        10.into(),
        1.into(),
        0,
        private_key,
    )
    .expect("failed to sign transfer");

    let transfer_tx = FranklinTx::Transfer(Box::new(transfer));

    let setup = || (state.clone(), transfer_tx.clone());

    b.iter_batched(
        setup,
        |(mut state, transfer_tx)| {
            state
                .execute_tx(black_box(transfer_tx))
                .expect("Failed to execute tx");
        },
        BatchSize::SmallInput,
    );
}

/// Bench for `PlasmaState::apply_full_exit_op`.
fn apply_full_exit_op(b: &mut Bencher<'_>) {
    let (_, state) = generate_state();

    let from_account = state.get_account(0).expect("Can't get the account");

    let full_exit = FullExit {
        account_id: 0,
        eth_address: from_account.address,
        token: ETH_TOKEN_ID,
    };

    let full_exit_op = FranklinPriorityOp::FullExit(full_exit);

    let setup = || (state.clone(), full_exit_op.clone());

    b.iter_batched(
        setup,
        |(mut state, full_exit_op)| {
            let _ = state.execute_priority_op(black_box(full_exit_op));
        },
        BatchSize::SmallInput,
    );
}

/// Bench for `PlasmaState::apply_deposit_op`.
fn apply_deposit_op(b: &mut Bencher<'_>) {
    let (_, state) = generate_state();

    let to_account = state.get_account(0).expect("Can't get the account");

    let deposit = Deposit {
        from: Address::random(),
        to: to_account.address,
        token: ETH_TOKEN_ID,
        amount: 10.into(),
    };

    let deposit_op = FranklinPriorityOp::Deposit(deposit);

    let setup = || (state.clone(), deposit_op.clone());

    b.iter_batched(
        setup,
        |(mut state, deposit_op)| {
            let _ = state.execute_priority_op(black_box(deposit_op));
        },
        BatchSize::SmallInput,
    );
}

/// Bench for `PlasmaState::apply_withdraw_op`.
fn apply_withdraw_op(b: &mut Bencher<'_>) {
    let (keys, state) = generate_state();

    let from_account = state.get_account(0).expect("Can't get the account");
    let (private_key, _) = keys.get(&0).expect("Can't key the private key");

    let withdraw = Withdraw::new_signed(
        from_account.address,
        Address::random(),
        ETH_TOKEN_ID,
        10.into(),
        1.into(),
        0,
        private_key,
    )
    .expect("failed to sign withdraw");

    let withdraw_tx = FranklinTx::Withdraw(Box::new(withdraw));

    let setup = || (state.clone(), withdraw_tx.clone());

    b.iter_batched(
        setup,
        |(mut state, withdraw_tx)| {
            let _ = state.execute_tx(black_box(withdraw_tx));
        },
        BatchSize::SmallInput,
    );
}

// There is no bench for `PlasmaState::apply_close_op`, since closing accounts is currently disabled.

/// Bench for `PlasmaState::apply_change_pubkey_op`.
fn apply_change_pubkey_op(b: &mut Bencher<'_>) {
    let (keys, state) = generate_state();

    let to_change = state.get_account(0).expect("Can't get the account");
    let (_, eth_private_key) = keys.get(&0).expect("Can't key the private key");

    let rng = &mut thread_rng();
    let new_sk = priv_key_from_fs(rng.gen());

    let nonce = 0;

    let eth_signature = {
        let sign_bytes = ChangePubKey::get_eth_signed_data(nonce, &to_change.pub_key_hash)
            .expect("Failed to construct ChangePubKey signed message.");
        let eth_signature =
            PackedEthSignature::sign(eth_private_key, &sign_bytes).expect("Signing failed");
        Some(eth_signature)
    };

    let change_pubkey = ChangePubKey {
        account: to_change.address,
        new_pk_hash: PubKeyHash::from_privkey(&new_sk),
        nonce,
        eth_signature,
    };

    let change_pubkey_tx = FranklinTx::ChangePubKey(Box::new(change_pubkey));

    let setup = || (state.clone(), change_pubkey_tx.clone());

    b.iter_batched(
        setup,
        |(mut state, change_pubkey_tx)| {
            let _ = state.execute_tx(black_box(change_pubkey_tx));
        },
        BatchSize::SmallInput,
    );
}

/// Bench for `PlasmaState::insert_account`.
///
/// While this method is not directly performing an operation, it is used in every operation,
/// and it seems to be the most expensive part of all the methods above.
fn insert_account(b: &mut Bencher<'_>) {
    let (_, state) = generate_state();

    let (_, _, to_insert) = generate_account();
    let setup = || (state.clone(), to_insert.clone());

    b.iter_batched(
        setup,
        |(mut state, to_insert)| {
            state.insert_account(black_box(ACCOUNTS_AMOUNT), to_insert);
        },
        BatchSize::SmallInput,
    );
}

pub fn bench_ops(c: &mut Criterion) {
    const INPUT_SIZE: Throughput = Throughput::Elements(1);

    let mut group = c.benchmark_group("PlasmaState operations");

    // Setup the input size so the throughput will be reported.
    group.throughput(INPUT_SIZE);

    group.bench_function(
        "PlasmaState::apply_transfer_to_new_op bench",
        apply_transfer_to_new_op,
    );
    group.bench_function("PlasmaState::apply_transfer_op bench", apply_transfer_op);
    group.bench_function("PlasmaState::apply_withdraw_op bench", apply_withdraw_op);
    group.bench_function(
        "PlasmaState::apply_change_pubkey_op bench",
        apply_change_pubkey_op,
    );
    group.bench_function("PlasmaState::apply_deposit_op bench", apply_deposit_op);
    group.bench_function("PlasmaState::apply_full_exit_op bench", apply_full_exit_op);
    group.bench_function("PlasmaState::insert_account bench", insert_account);

    group.finish();
}

criterion_group!(ops_benches, bench_ops);
