// External uses
use criterion::{black_box, criterion_group, BatchSize, Bencher, Criterion};
// Workspace uses
use crypto_exports::rand::{thread_rng, Rng};
use models::node::{
    account::{Account, PubKeyHash},
    operations::{
        ChangePubKeyOp, CloseOp, DepositOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp,
    },
    priority_ops::{Deposit, FullExit},
    priv_key_from_fs,
    tx::{ChangePubKey, Close, Transfer, TxSignature, Withdraw},
    AccountId, AccountMap, Address, BlockNumber, TokenId,
};
// Local uses
use plasma::state::PlasmaState;

const ETH_TOKEN_ID: TokenId = 0x00;
const ACCOUNTS_AMOUNT: AccountId = 10;
const CURRENT_BLOCK: BlockNumber = 1_000;

fn generate_account() -> Account {
    let default_balance = 1_000_000.into();

    let rng = &mut thread_rng();
    let sk = priv_key_from_fs(rng.gen());

    let mut account = Account::default();
    account.pub_key_hash = PubKeyHash::from_privkey(&sk);
    account.address = Address::random();
    account.set_balance(ETH_TOKEN_ID, default_balance);

    account
}

/// Creates a `PlasmaState` object and fills it with accounts.
fn generate_state() -> PlasmaState {
    let mut accounts = AccountMap::default();

    for account_id in 0..ACCOUNTS_AMOUNT {
        let new_account = generate_account();

        accounts.insert(account_id, new_account);
    }

    PlasmaState::new(accounts, CURRENT_BLOCK)
}

fn apply_transfer_to_new_op(b: &mut Bencher<'_>) {
    let state = generate_state();

    let from_account = state.get_account(0).expect("Can't get the account");

    let transfer = Transfer {
        from: from_account.address,
        to: Address::random(),
        token: ETH_TOKEN_ID,
        amount: 10.into(),
        fee: 1.into(),
        nonce: 0,
        signature: TxSignature::default(),
    };

    let transfer_op = TransferToNewOp {
        tx: transfer,
        from: 0,
        to: ACCOUNTS_AMOUNT,
    };

    let setup = || (state.clone(), transfer_op.clone());

    b.iter_batched(
        setup,
        |(mut state, transfer_op)| {
            state
                .apply_transfer_to_new_op(&black_box(transfer_op))
                .expect("Failed transfer operation");
        },
        BatchSize::SmallInput,
    );
}

fn apply_transfer_op(b: &mut Bencher<'_>) {
    let state = generate_state();

    let from_account = state.get_account(0).expect("Can't get the account");
    let to_account = state.get_account(1).expect("Can't get the account");

    let transfer = Transfer {
        from: from_account.address,
        to: to_account.address,
        token: ETH_TOKEN_ID,
        amount: 10.into(),
        fee: 1.into(),
        nonce: 0,
        signature: TxSignature::default(),
    };

    let transfer_op = TransferOp {
        tx: transfer,
        from: 0,
        to: 1,
    };

    let setup = || (state.clone(), transfer_op.clone());

    b.iter_batched(
        setup,
        |(mut state, transfer_op)| {
            state
                .apply_transfer_op(&black_box(transfer_op))
                .expect("Failed transfer operation");
        },
        BatchSize::SmallInput,
    );
}

fn apply_full_exit_op(b: &mut Bencher<'_>) {
    let state = generate_state();

    let to_account = state.get_account(0).expect("Can't get the account");

    let full_exit = FullExit {
        account_id: 0,
        eth_address: Address::random(),
        token: ETH_TOKEN_ID,
    };

    let full_exit_op = FullExitOp {
        priority_op: full_exit,
        withdraw_amount: Some(to_account.get_balance(ETH_TOKEN_ID)),
    };

    let setup = || (state.clone(), full_exit_op.clone());

    b.iter_batched(
        setup,
        |(mut state, full_exit_op)| {
            let _ = state.apply_full_exit_op(&black_box(full_exit_op));
        },
        BatchSize::SmallInput,
    );
}

fn apply_deposit_op(b: &mut Bencher<'_>) {
    let state = generate_state();

    let to_account = state.get_account(0).expect("Can't get the account");

    let deposit = Deposit {
        from: Address::random(),
        to: to_account.address,
        token: ETH_TOKEN_ID,
        amount: 10.into(),
    };

    let deposit_op = DepositOp {
        priority_op: deposit,
        account_id: 0,
    };

    let setup = || (state.clone(), deposit_op.clone());

    b.iter_batched(
        setup,
        |(mut state, deposit_op)| {
            let _ = state.apply_deposit_op(&black_box(deposit_op));
        },
        BatchSize::SmallInput,
    );
}

fn apply_withdraw_op(b: &mut Bencher<'_>) {
    let state = generate_state();

    let from_account = state.get_account(0).expect("Can't get the account");

    let withdraw = Withdraw {
        from: from_account.address,
        to: Address::random(),
        token: ETH_TOKEN_ID,
        amount: 10.into(),
        fee: 1.into(),
        nonce: 0,
        signature: TxSignature::default(),
    };

    let withdraw_op = WithdrawOp {
        tx: withdraw,
        account_id: 0,
    };

    let setup = || (state.clone(), withdraw_op.clone());

    b.iter_batched(
        setup,
        |(mut state, withdraw_op)| {
            let _ = state.apply_withdraw_op(&black_box(withdraw_op));
        },
        BatchSize::SmallInput,
    );
}

fn apply_close_op(b: &mut Bencher<'_>) {
    let mut state = generate_state();

    let mut to_remove = state.get_account(0).expect("Can't get the account");

    // Remove balance from the account to close.
    to_remove.set_balance(ETH_TOKEN_ID, 0.into());
    state.insert_account(0, to_remove.clone());

    let close = Close {
        account: to_remove.address,
        nonce: 0,
        signature: TxSignature::default(),
    };

    let close_op = CloseOp {
        tx: close,
        account_id: 0,
    };

    let setup = || (state.clone(), close_op.clone());

    b.iter_batched(
        setup,
        |(mut state, close_op)| {
            let _ = state.apply_close_op(&black_box(close_op));
        },
        BatchSize::SmallInput,
    );
}

fn apply_change_pubkey_op(b: &mut Bencher<'_>) {
    let state = generate_state();

    let to_change = state.get_account(0).expect("Can't get the account");

    let rng = &mut thread_rng();
    let new_sk = priv_key_from_fs(rng.gen());

    let change_pubkey = ChangePubKey {
        account: to_change.address,
        new_pk_hash: PubKeyHash::from_privkey(&new_sk),
        nonce: 0,
        eth_signature: None,
    };

    let change_pubkey_op = ChangePubKeyOp {
        tx: change_pubkey,
        account_id: 0,
    };

    let setup = || (state.clone(), change_pubkey_op.clone());

    b.iter_batched(
        setup,
        |(mut state, change_pubkey_op)| {
            let _ = state.apply_change_pubkey_op(&black_box(change_pubkey_op));
        },
        BatchSize::SmallInput,
    );
}

fn insert_account(b: &mut Bencher<'_>) {
    let state = generate_state();

    let to_insert = generate_account();
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
    c.bench_function(
        "PlasmaState::apply_transfer_to_new_op bench",
        apply_transfer_to_new_op,
    );
    c.bench_function("PlasmaState::apply_transfer_op bench", apply_transfer_op);
    c.bench_function("PlasmaState::apply_withdraw_op bench", apply_withdraw_op);
    c.bench_function("PlasmaState::apply_apply_close_op bench", apply_close_op);
    c.bench_function(
        "PlasmaState::apply_change_pubkey_op bench",
        apply_change_pubkey_op,
    );
    c.bench_function("PlasmaState::apply_deposit_op bench", apply_deposit_op);
    c.bench_function("PlasmaState::apply_full_exit_op bench", apply_full_exit_op);
    c.bench_function("PlasmaState::insert_account bench", insert_account);
}

criterion_group!(ops_benches, bench_ops);
