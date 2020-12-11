use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{criterion_group, Bencher, BenchmarkId, Criterion};
use zksync_circuit::witness::Witness;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::CloseOp;

use zksync_circuit::witness::close_account::CloseAccountWitness;

type CloseAccountWitnessBn256 = CloseAccountWitness<Bn256>;

/// Measures the time of closing account witness
fn close_account_witness(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account = &accounts[0];
    let close_account_op = CloseOp {
        tx: account.zksync_account.sign_close(None, true),
        account_id: account.id,
    };
    let (_, circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let setup = || (circuit_account_tree.clone());
    b.iter_with_setup(setup, |mut circuit_account_tree| {
        CloseAccountWitnessBn256::apply_tx(&mut circuit_account_tree, &close_account_op);
    });
}

pub fn bench_close_account_witness(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Close account witness", 1usize),
        &1usize,
        close_account_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Close account witness", 10usize),
        &10usize,
        close_account_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Close account witness", 100usize),
        &100usize,
        close_account_witness,
    );
}

criterion_group!(close_account_witness_benches, bench_close_account_witness);
