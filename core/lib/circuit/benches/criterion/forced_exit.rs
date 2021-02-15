use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{black_box, criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::{utils::SigDataInput, Witness};
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::{ForcedExitOp, TokenId};

use zksync_circuit::witness::forced_exit::ForcedExitWitness;

type ForcedExitWitnessBn256 = ForcedExitWitness<Bn256>;

/// Measures the time of forced exit apply tx
fn forced_exit_apply_tx(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account_from = &accounts[0];
    let account_to = &accounts[0];
    let forced_exit_op = ForcedExitOp {
        tx: account_from.zksync_account.sign_forced_exit(
            TokenId(0),
            BigUint::from(1u64),
            &account_to.account.address,
            None,
            true,
            Default::default(),
        ),
        target_account_id: account_to.id,
        withdraw_amount: Some(BigUint::from(100u64).into()),
    };
    let (_, circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let setup = || (circuit_account_tree.clone());
    b.iter_with_setup(setup, |mut circuit_account_tree| {
        ForcedExitWitnessBn256::apply_tx(&mut circuit_account_tree, &forced_exit_op);
    });
}

/// Measures the time of forced exit get pubdata
fn forced_exit_get_pubdata(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account_from = &accounts[0];
    let account_to = &accounts[0];
    let forced_exit_op = ForcedExitOp {
        tx: account_from.zksync_account.sign_forced_exit(
            TokenId(0),
            BigUint::from(1u64),
            &account_to.account.address,
            None,
            true,
            Default::default(),
        ),
        target_account_id: account_to.id,
        withdraw_amount: Some(BigUint::from(100u64).into()),
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = ForcedExitWitnessBn256::apply_tx(&mut circuit_account_tree, &forced_exit_op);
    b.iter(|| {
        let _pubdata = black_box(witness.get_pubdata());
    });
}

/// Measures the time of forced exit calculate operations
fn forced_exit_calculate_operations(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account_from = &accounts[0];
    let account_to = &accounts[0];
    let forced_exit_op = ForcedExitOp {
        tx: account_from.zksync_account.sign_forced_exit(
            TokenId(0),
            BigUint::from(1u64),
            &account_to.account.address,
            None,
            true,
            Default::default(),
        ),
        target_account_id: account_to.id,
        withdraw_amount: Some(BigUint::from(100u64).into()),
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = ForcedExitWitnessBn256::apply_tx(&mut circuit_account_tree, &forced_exit_op);
    let input =
        SigDataInput::from_forced_exit_op(&forced_exit_op).expect("SigDataInput creation failed");
    let setup = || (input.clone());
    b.iter_with_setup(setup, |input| {
        let _ops = black_box(witness.calculate_operations(input));
    });
}

pub fn bench_forced_exit(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Forced exit apply tx", 1usize),
        &1usize,
        forced_exit_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Forced exit apply tx", 10usize),
        &10usize,
        forced_exit_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Forced exit apply tx", 100usize),
        &100usize,
        forced_exit_apply_tx,
    );
    c.bench_function("Forced exit get pubdata", forced_exit_get_pubdata);
    c.bench_function(
        "Forced exit calculate operations",
        forced_exit_calculate_operations,
    );
}

criterion_group!(forced_exit_benches, bench_forced_exit);
