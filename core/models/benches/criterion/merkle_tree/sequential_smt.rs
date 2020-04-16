//! Benchmarks for the Sequential Sparse Merkle Tree.

use criterion::{black_box, BatchSize, Bencher, Criterion};

use models::circuit::account::CircuitAccount;
use models::franklin_crypto::bellman::pairing::bn256::{Bn256, Fr};
use models::merkle_tree::{pedersen_hasher::PedersenHasher, sequential_smt::SparseMerkleTree};
use std::convert::TryInto;

// This value should be not to high, since the bench will be run for thousands
// of iterations. Despite the tree cloning time won't affect the bench results
// (cloning is performed within `setup` closure), the bench will take forever to
// be completed if the value is too big.
const N_ACCOUNTS: u32 = 100;

/// Type alias equivalent to the actually used SMT.
type RealSMT = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;

fn gen_account(id: u32) -> CircuitAccount<Bn256> {
    let mut account = CircuitAccount::<Bn256>::default();

    let id_hex = format!("{:064x}", id);
    account.address = Fr::from_hex(id_hex.as_ref()).unwrap();

    account
}

/// Measures the time of `RealSMT` creation time.
fn smt_create(b: &mut Bencher<'_>) {
    let depth = models::params::account_tree_depth();

    b.iter(|| {
        RealSMT::new(black_box(depth));
    });
}

/// Measures the time of insertion into an empty SMT.
fn smt_insert_empty(b: &mut Bencher<'_>) {
    let depth = models::params::account_tree_depth();

    // Create an empty SMT and one account in setup.
    let tree = RealSMT::new(depth);
    let account = gen_account(0);

    let setup = || (tree.clone(), account.clone());

    b.iter_batched(
        setup,
        |(mut tree, account)| {
            let id = 0;
            tree.insert(black_box(id), account);
        },
        BatchSize::SmallInput,
    );
}

/// Measures the time of insertion into a non-empty SMT.
fn smt_insert_filled(b: &mut Bencher<'_>) {
    let depth = models::params::account_tree_depth();
    let accounts: Vec<_> = (0..N_ACCOUNTS).map(gen_account).collect();

    // Create a tree and fill it with some accounts.
    let mut tree = RealSMT::new(depth);
    for (id, account) in accounts.into_iter().enumerate() {
        tree.insert(id as u32, account.clone())
    }
    let latest_account = gen_account(N_ACCOUNTS);

    let setup = || (tree.clone(), latest_account.clone());

    b.iter_batched(
        setup,
        |(mut tree, account)| {
            let id = N_ACCOUNTS;
            tree.insert(black_box(id), account);
        },
        BatchSize::SmallInput,
    );
}

/// Measures the time of obtaining a SMT root hash.
fn smt_root_hash(b: &mut Bencher<'_>) {
    let depth = models::params::account_tree_depth();
    let accounts: Vec<_> = (0..N_ACCOUNTS).map(gen_account).collect();

    // Create a tree and fill it with some accounts.
    let mut tree = RealSMT::new(depth);
    for (id, account) in accounts.into_iter().enumerate() {
        tree.insert(id as u32, account.clone())
    }

    let setup = || (tree.clone());

    b.iter_batched(
        setup,
        |tree| {
            let _hash = black_box(tree.root_hash());
        },
        BatchSize::SmallInput,
    );
}

pub fn bench_merkle_tree(c: &mut Criterion) {
    c.bench_function("Sequential SMT create", smt_create);
    c.bench_function("Sequential SMT insert (empty)", smt_insert_empty);
    c.bench_function("Sequential SMT insert (filled)", smt_insert_filled);
    c.bench_function("Sequential SMT root hash", smt_root_hash);
}
