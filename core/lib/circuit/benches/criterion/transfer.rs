use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::Witness;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::TransferOp;

use zksync_circuit::witness::transfer::TransferWitness;

type TransferWitnessBn256 = TransferWitness<Bn256>;

/// Measures the time of trasfer witness
fn transfer_witness(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account_from = &accounts[0];
    let account_to = &accounts[0];
    let transfer_op = TransferOp {
        tx: account_from
            .zksync_account
            .sign_transfer(
                0,
                "",
                BigUint::from(100u64),
                BigUint::from(1u64),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        from: account_from.id,
        to: account_to.id,
    };
    let (_, circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let setup = || (circuit_account_tree.clone());
    b.iter_with_setup(setup, |mut circuit_account_tree| {
        TransferWitnessBn256::apply_tx(&mut circuit_account_tree, &transfer_op);
    });
}

pub fn bench_transfer(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Transfer witness", 1usize),
        &1usize,
        transfer_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Transfer witness", 10usize),
        &10usize,
        transfer_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Transfer witness", 100usize),
        &100usize,
        transfer_witness,
    );
}

criterion_group!(transfer_benches, bench_transfer);
