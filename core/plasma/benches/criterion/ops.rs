// External uses
use criterion::{black_box, criterion_group, BatchSize, Bencher, Criterion};
// Workspace uses
use crypto_exports::rand::{thread_rng, Rng};
use models::node::{
    account::{Account, PubKeyHash},
    operations::{TransferOp, TransferToNewOp},
    priv_key_from_fs,
    tx::{Transfer, TxSignature},
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

pub fn bench_ops(c: &mut Criterion) {
    c.bench_function("apply_transfer_to_new_op bench", apply_transfer_to_new_op);
    c.bench_function("apply_transfer_op bench", apply_transfer_op);
}

criterion_group!(ops_benches, bench_ops);
