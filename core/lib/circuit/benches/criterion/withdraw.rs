use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::Witness;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::{Address, WithdrawOp};

use zksync_circuit::witness::withdraw::WithdrawWitness;

type WithdrawWitnessBn256 = WithdrawWitness<Bn256>;

/// Measures the time of withdraw witness
fn withdraw_witness(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account = &accounts[0];
    let withdraw_op = WithdrawOp {
        tx: account
            .zksync_account
            .sign_withdraw(
                0,
                "",
                BigUint::from(100u64),
                BigUint::from(1u64),
                &Address::zero(),
                None,
                true,
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

pub fn bench_withdraw(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Withdraw witness", 1usize),
        &1usize,
        withdraw_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Withdraw witness", 10usize),
        &10usize,
        withdraw_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Withdraw witness", 100usize),
        &100usize,
        withdraw_witness,
    );
}

criterion_group!(withdraw_benches, bench_withdraw);
