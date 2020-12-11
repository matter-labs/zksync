use crate::generate_accounts;
use crate::utils::WitnessTestAccount;
use crate::utils::ZkSyncStateGenerator;
use criterion::{criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::Witness;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::TransferToNewOp;

use zksync_circuit::witness::transfer_to_new::TransferToNewWitness;

type TransferToNewWitnessBn256 = TransferToNewWitness<Bn256>;

/// Measures the time of trasfer to new witness
fn transfer_to_new_witness(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account_from = &accounts[0];
    let account_to = WitnessTestAccount::new(1000u32, 200u64);
    let transfer_op = TransferToNewOp {
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
        TransferToNewWitnessBn256::apply_tx(&mut circuit_account_tree, &transfer_op);
    });
}

pub fn bench_transfer_to_new(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Transfer to new witness", 1usize),
        &1usize,
        transfer_to_new_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Transfer to new witness", 10usize),
        &10usize,
        transfer_to_new_witness,
    );
    c.bench_with_input(
        BenchmarkId::new("Transfer to new witness", 100usize),
        &100usize,
        transfer_to_new_witness,
    );
}

criterion_group!(transfer_to_new_benches, bench_transfer_to_new);
