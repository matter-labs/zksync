use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{black_box, criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::{utils::SigDataInput, Witness};
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::{Address, TokenId, WithdrawOp};

use zksync_circuit::witness::withdraw::WithdrawWitness;

type WithdrawWitnessBn256 = WithdrawWitness<Bn256>;

/// Measures the time of withdraw apply tx
fn withdraw_apply_tx(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account = &accounts[0];
    let withdraw_op = WithdrawOp {
        tx: account
            .zksync_account
            .sign_withdraw(
                TokenId(0),
                "",
                BigUint::from(100u64),
                BigUint::from(1u64),
                &Address::zero(),
                None,
                true,
                Default::default(),
            )
            .0,
        account_id: account.id,
    };
    let (_, circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let setup = || (circuit_account_tree.clone());
    b.iter_with_setup(setup, |mut circuit_account_tree| {
        WithdrawWitnessBn256::apply_tx(&mut circuit_account_tree, &withdraw_op);
    });
}

/// Measures the time of withdraw get pubdata
fn withdraw_get_pubdata(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account = &accounts[0];
    let withdraw_op = WithdrawOp {
        tx: account
            .zksync_account
            .sign_withdraw(
                TokenId(0),
                "",
                BigUint::from(100u64),
                BigUint::from(1u64),
                &Address::zero(),
                None,
                true,
                Default::default(),
            )
            .0,
        account_id: account.id,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = WithdrawWitnessBn256::apply_tx(&mut circuit_account_tree, &withdraw_op);
    b.iter(|| {
        let _pubdata = black_box(witness.get_pubdata());
    });
}

/// Measures the time of withdraw calculate operations
fn withdraw_calculate_operations(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account = &accounts[0];
    let withdraw_op = WithdrawOp {
        tx: account
            .zksync_account
            .sign_withdraw(
                TokenId(0),
                "",
                BigUint::from(100u64),
                BigUint::from(1u64),
                &Address::zero(),
                None,
                true,
                Default::default(),
            )
            .0,
        account_id: account.id,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = WithdrawWitnessBn256::apply_tx(&mut circuit_account_tree, &withdraw_op);
    let input = SigDataInput::from_withdraw_op(&withdraw_op).expect("SigDataInput creation failed");
    let setup = || (input.clone());
    b.iter_with_setup(setup, |input| {
        let _ops = black_box(witness.calculate_operations(input));
    });
}

pub fn bench_withdraw(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Withdraw apply tx", 1usize),
        &1usize,
        withdraw_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Withdraw apply tx", 10usize),
        &10usize,
        withdraw_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Withdraw apply tx", 100usize),
        &100usize,
        withdraw_apply_tx,
    );
    c.bench_function("Withdraw get pubdata", withdraw_get_pubdata);
    c.bench_function(
        "Withdraw calculate operations",
        withdraw_calculate_operations,
    );
}

criterion_group!(withdraw_benches, bench_withdraw);
