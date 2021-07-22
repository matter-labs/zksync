use crate::generate_accounts;
use crate::utils::WitnessTestAccount;
use crate::utils::ZkSyncStateGenerator;
use criterion::{black_box, criterion_group, Bencher, BenchmarkId, Criterion};
use num::BigUint;
use zksync_circuit::witness::{utils::SigDataInput, Witness};
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::{AccountId, TokenId, TransferToNewOp};

use zksync_circuit::witness::transfer_to_new::TransferToNewWitness;

type TransferToNewWitnessBn256 = TransferToNewWitness<Bn256>;

/// Measures the time of trasfer to new apply tx
fn transfer_to_new_apply_tx(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account_from = &accounts[0];
    let account_to = WitnessTestAccount::new(AccountId(1000), 200u64);
    let transfer_op = TransferToNewOp {
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
        TransferToNewWitnessBn256::apply_tx(&mut circuit_account_tree, &transfer_op);
    });
}

/// Measures the time of trasfer to new get pubdata
fn transfer_to_new_get_pubdata(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account_from = &accounts[0];
    let account_to = WitnessTestAccount::new(AccountId(1000), 200u64);
    let transfer_op = TransferToNewOp {
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

    let witness = TransferToNewWitnessBn256::apply_tx(&mut circuit_account_tree, &transfer_op);
    b.iter(|| {
        let _pubdata = black_box(witness.get_pubdata());
    });
}

/// Measures the time of trasfer to new calculate operations
fn transfer_to_new_calculate_operations(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account_from = &accounts[0];
    let account_to = WitnessTestAccount::new(AccountId(1000), 200u64);
    let transfer_op = TransferToNewOp {
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

    let witness = TransferToNewWitnessBn256::apply_tx(&mut circuit_account_tree, &transfer_op);
    let input =
        SigDataInput::from_transfer_to_new_op(&transfer_op).expect("SigDataInput creation failed");
    let setup = || (input.clone());
    b.iter_with_setup(setup, |input| {
        let _ops = black_box(witness.calculate_operations(input));
    });
}

pub fn bench_transfer_to_new(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Transfer to new apply tx", 1usize),
        &1usize,
        transfer_to_new_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Transfer to new apply tx", 10usize),
        &10usize,
        transfer_to_new_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Transfer to new apply tx", 100usize),
        &100usize,
        transfer_to_new_apply_tx,
    );
    c.bench_function("Transfer to new get pubdata", transfer_to_new_get_pubdata);
    c.bench_function(
        "Transfer to new calculate operations",
        transfer_to_new_calculate_operations,
    );
}

criterion_group!(transfer_to_new_benches, bench_transfer_to_new);
