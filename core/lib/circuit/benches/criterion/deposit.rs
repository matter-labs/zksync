use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::Witness;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::{Deposit, DepositOp};

use zksync_circuit::witness::deposit::DepositWitness;

type DepositWitnessBn256 = DepositWitness<Bn256>;

/// Measures the time of creating deposit witness
fn create_deposit_witness(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account = &accounts[0];
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: account.account.address,
            token: 0,
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

pub fn bench_deposit_witness(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Create deposit witness", 1usize),
        &1usize,
        create_deposit_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Create deposit witness", 10usize),
        &10usize,
        create_deposit_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Create deposit witness", 100usize),
        &100usize,
        create_deposit_witness,
    );
}

criterion_group!(deposit_witness_benches, bench_deposit_witness);
