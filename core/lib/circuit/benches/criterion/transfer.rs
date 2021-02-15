use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{black_box, criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::{utils::SigDataInput, Witness};
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::{TokenId, TransferOp};

use zksync_circuit::witness::transfer::TransferWitness;

type TransferWitnessBn256 = TransferWitness<Bn256>;

/// Measures the time of trasfer apply tx
fn transfer_apply_tx(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account_from = &accounts[0];
    let account_to = &accounts[0];
    let transfer_op = TransferOp {
        tx: account_from
            .zksync_account
            .sign_transfer(
                TokenId(0),
                "",
                BigUint::from(100u64),
                BigUint::from(1u64),
                &account_to.account.address,
                None,
                true,
                Default::default(),
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

/// Measures the time of trasfer get pubdata
fn transfer_get_pubdata(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account_from = &accounts[0];
    let account_to = &accounts[0];
    let transfer_op = TransferOp {
        tx: account_from
            .zksync_account
            .sign_transfer(
                TokenId(0),
                "",
                BigUint::from(100u64),
                BigUint::from(1u64),
                &account_to.account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        from: account_from.id,
        to: account_to.id,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = TransferWitnessBn256::apply_tx(&mut circuit_account_tree, &transfer_op);
    b.iter(|| {
        let _pubdata = black_box(witness.get_pubdata());
    });
}

/// Measures the time of trasfer calculate operations
fn transfer_calculate_operations(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account_from = &accounts[0];
    let account_to = &accounts[0];
    let transfer_op = TransferOp {
        tx: account_from
            .zksync_account
            .sign_transfer(
                TokenId(0),
                "",
                BigUint::from(100u64),
                BigUint::from(1u64),
                &account_to.account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        from: account_from.id,
        to: account_to.id,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness = TransferWitnessBn256::apply_tx(&mut circuit_account_tree, &transfer_op);
    let input = SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");
    let setup = || (input.clone());
    b.iter_with_setup(setup, |input| {
        let _ops = black_box(witness.calculate_operations(input));
    });
}

pub fn bench_transfer(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Transfer apply tx", 1usize),
        &1usize,
        transfer_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Transfer apply tx", 10usize),
        &10usize,
        transfer_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Transfer apply tx", 100usize),
        &100usize,
        transfer_apply_tx,
    );
    c.bench_function("Transfer get pubdata", transfer_get_pubdata);
    c.bench_function(
        "Transfer calculate operations",
        transfer_calculate_operations,
    );
}

criterion_group!(transfer_benches, bench_transfer);
