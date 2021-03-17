use crate::generate_accounts;
use crate::utils::ZkSyncStateGenerator;
use criterion::{black_box, criterion_group, Bencher, BenchmarkId, Criterion};
use zksync_circuit::witness::{utils::SigDataInput, Witness};
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_types::{tx::ChangePubKeyType, ChangePubKeyOp, TokenId};

use zksync_circuit::witness::change_pubkey_offchain::ChangePubkeyOffChainWitness;

type ChangePubkeyOffChainWitnessBn256 = ChangePubkeyOffChainWitness<Bn256>;

/// Measures the time of change pubkey offchain apply tx
fn change_pubkey_offchain_apply_tx(b: &mut Bencher<'_>, number_of_accounts: &usize) {
    let accounts = generate_accounts(*number_of_accounts);
    let account = &accounts[0];
    let change_pkhash_op = ChangePubKeyOp {
        tx: account.zksync_account.sign_change_pubkey_tx(
            None,
            true,
            TokenId(0),
            Default::default(),
            ChangePubKeyType::ECDSA,
            Default::default(),
        ),
        account_id: account.id,
    };
    let (_, circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let setup = || (circuit_account_tree.clone());

    b.iter_with_setup(setup, |mut circuit_account_tree| {
        ChangePubkeyOffChainWitnessBn256::apply_tx(&mut circuit_account_tree, &change_pkhash_op);
    });
}

/// Measures the time of change pubkey offchain get pubdata
fn change_pubkey_offchain_get_pubdata(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account = &accounts[0];
    let change_pkhash_op = ChangePubKeyOp {
        tx: account.zksync_account.sign_change_pubkey_tx(
            None,
            true,
            TokenId(0),
            Default::default(),
            ChangePubKeyType::ECDSA,
            Default::default(),
        ),
        account_id: account.id,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness =
        ChangePubkeyOffChainWitnessBn256::apply_tx(&mut circuit_account_tree, &change_pkhash_op);
    b.iter(|| {
        let _pubdata = black_box(witness.get_pubdata());
    });
}

/// Measures the time of change pubkey offchain calculate operations
fn change_pubkey_offchain_calculate_operations(b: &mut Bencher<'_>) {
    let accounts = generate_accounts(10);
    let account = &accounts[0];
    let change_pkhash_op = ChangePubKeyOp {
        tx: account.zksync_account.sign_change_pubkey_tx(
            None,
            true,
            TokenId(0),
            Default::default(),
            ChangePubKeyType::ECDSA,
            Default::default(),
        ),
        account_id: account.id,
    };
    let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);

    let witness =
        ChangePubkeyOffChainWitnessBn256::apply_tx(&mut circuit_account_tree, &change_pkhash_op);
    let input = SigDataInput::from_change_pubkey_op(&change_pkhash_op)
        .expect("SigDataInput creation failed");
    let setup = || (input.clone());
    b.iter_with_setup(setup, |input| {
        let _ops = black_box(witness.calculate_operations(input));
    });
}

pub fn bench_change_pubkey_offchain_witness(c: &mut Criterion) {
    c.bench_with_input(
        BenchmarkId::new("Change pubkey offchain apply tx", 1usize),
        &1usize,
        change_pubkey_offchain_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Change pubkey offchain apply tx", 10usize),
        &10usize,
        change_pubkey_offchain_apply_tx,
    );
    c.bench_with_input(
        BenchmarkId::new("Change pubkey offchain apply tx", 100usize),
        &100usize,
        change_pubkey_offchain_apply_tx,
    );
    c.bench_function(
        "Change pubkey offchain get pubdata",
        change_pubkey_offchain_get_pubdata,
    );
    c.bench_function(
        "Change pubkey offchain calculate operations",
        change_pubkey_offchain_calculate_operations,
    );
}

criterion_group!(
    change_pubkey_offchain_witness_benches,
    bench_change_pubkey_offchain_witness
);
