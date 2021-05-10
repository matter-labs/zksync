use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{black_box, criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::Witness;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::{FullExit, FullExitOp, TokenId};

use zksync_circuit::witness::full_exit::FullExitWitness;

type FullExitWitnessBn256 = FullExitWitness<Bn256>;

/// Measures the time of full exit apply tx
fn full_exit_apply_tx(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account = &accounts[0];
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: account.id,
            eth_address: account.account.address,
            token: TokenId(0),
        },
        withdraw_amount: Some(BigUint::from(10u32).into()),
        creator_account_id: None,
        creator_address: None,
        serial_id: None,
        content_hash: None,
    };
    let (_, circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let setup = || (circuit_account_tree.clone());
    b.iter_with_setup(setup, |mut circuit_account_tree| {
        FullExitWitnessBn256::apply_tx(&mut circuit_account_tree, &(full_exit_op.clone(), true));
    });
}

/// Measures the time of full exit get pubdata
fn full_exit_get_pubdata(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account = &accounts[0];
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: account.id,
            eth_address: account.account.address,
            token: TokenId(0),
        },
        withdraw_amount: Some(BigUint::from(10u32).into()),
        creator_account_id: None,
        creator_address: None,
        serial_id: None,
        content_hash: None,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = FullExitWitnessBn256::apply_tx(&mut circuit_account_tree, &(full_exit_op, true));
    b.iter(|| {
        let _pubdata = black_box(witness.get_pubdata());
    });
}

/// Measures the time of full exit calculate operations
fn full_exit_calculate_operations(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account = &accounts[0];
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: account.id,
            eth_address: account.account.address,
            token: TokenId(0),
        },
        withdraw_amount: Some(BigUint::from(10u32).into()),
        creator_account_id: None,
        creator_address: None,
        serial_id: None,
        content_hash: None,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = FullExitWitnessBn256::apply_tx(&mut circuit_account_tree, &(full_exit_op, true));
    b.iter(|| {
        let _ops = black_box(witness.calculate_operations(()));
    });
}

pub fn bench_full_exit(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Full exit apply tx", 1usize),
        &1usize,
        full_exit_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Full exit apply tx", 10usize),
        &10usize,
        full_exit_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Full exit apply tx", 100usize),
        &100usize,
        full_exit_apply_tx,
    );
    c.bench_function("Full exit get pubdata", full_exit_get_pubdata);
    c.bench_function(
        "Full exit calculate operations",
        full_exit_calculate_operations,
    );
}

criterion_group!(full_exit_benches, bench_full_exit);
