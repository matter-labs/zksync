use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{black_box, criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::Witness;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::{Deposit, DepositOp, TokenId};

use zksync_circuit::witness::deposit::DepositWitness;

type DepositWitnessBn256 = DepositWitness<Bn256>;

/// Measures the time of create deposit apply tx
fn create_deposit_apply_tx(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account = &accounts[0];
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: account.account.address,
            token: TokenId(0),
            amount: BigUint::from(1u32),
            to: account.account.address,
        },
        account_id: account.id,
    };
    let (_, circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let setup = || (circuit_account_tree.clone());
    b.iter_with_setup(setup, |mut circuit_account_tree| {
        DepositWitnessBn256::apply_tx(&mut circuit_account_tree, &deposit_op);
    });
}

/// Measures the time of create deposit get pubdata
fn create_deposit_get_pubdata(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account = &accounts[0];
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: account.account.address,
            token: TokenId(0),
            amount: BigUint::from(1u32),
            to: account.account.address,
        },
        account_id: account.id,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = DepositWitnessBn256::apply_tx(&mut circuit_account_tree, &deposit_op);
    b.iter(|| {
        let _pubdata = black_box(witness.get_pubdata());
    });
}

/// Measures the time of create deposit calculate operations
fn create_deposit_calculate_operations(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account = &accounts[0];
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: account.account.address,
            token: TokenId(0),
            amount: BigUint::from(1u32),
            to: account.account.address,
        },
        account_id: account.id,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = DepositWitnessBn256::apply_tx(&mut circuit_account_tree, &deposit_op);
    b.iter(|| {
        let _ops = black_box(witness.calculate_operations(()));
    });
}

pub fn bench_deposit_witness(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Create deposit apply tx", 1usize),
        &1usize,
        create_deposit_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Create deposit apply tx", 10usize),
        &10usize,
        create_deposit_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Create deposit apply tx", 100usize),
        &100usize,
        create_deposit_apply_tx,
    );
    c.bench_function("Create deposit get pubdata", create_deposit_get_pubdata);
    c.bench_function(
        "Create deposit calculate operations",
        create_deposit_calculate_operations,
    );
}

criterion_group!(deposit_witness_benches, bench_deposit_witness);
