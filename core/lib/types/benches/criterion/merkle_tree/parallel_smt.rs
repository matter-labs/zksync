//! Benchmarks for the Parallel Sparse Merkle Tree.

use criterion::{black_box, BatchSize, Bencher, Criterion};

use zksync_crypto::circuit::account::CircuitAccount;
use zksync_crypto::ff::PrimeField;
use zksync_crypto::merkle_tree::{parallel_smt::SparseMerkleTree, RescueHasher};
use zksync_crypto::{Engine, Fr};

// This value should be not to high, since the bench will be run for thousands
// of iterations. Despite the tree cloning time won't affect the bench results
// (cloning is performed within `setup` closure), the bench will take forever to
// be completed if the value is too big.
const N_ACCOUNTS: u32 = 100;

/// Type alias equivalent to the actually used SMT (but parallel tree is used instead of sequential).
type RealSMT = SparseMerkleTree<CircuitAccount<Engine>, Fr, RescueHasher<Engine>>;

fn gen_account(id: u32) -> CircuitAccount<Engine> {
    CircuitAccount::<Engine> {
        address: Fr::from_str(&id.to_string()).unwrap(),
        ..Default::default()
    }
}

/// `account.clone()` is used inside several benchmarks, since tree has a big `Drop` cost,
/// but at the same time `account` is consumed by the tree.
/// So if benchmark uses `account.clone()`, you can subtract its cost from the overall bench result.
fn account_clone(b: &mut Bencher<'_>) {
    let account = gen_account(0);

    b.iter(|| {
        let _ = black_box(account.clone());
    });
}

/// Measures the time of `RealSMT` creation time.
fn smt_create(b: &mut Bencher<'_>) {
    let depth = zksync_crypto::params::account_tree_depth();

    b.iter_with_large_drop(|| {
        RealSMT::new(black_box(depth));
    });
}

/// Measures the time of insertion into an empty SMT.
fn smt_insert_empty(b: &mut Bencher<'_>) {
    let depth = zksync_crypto::params::account_tree_depth();

    // Create an empty SMT and one account in setup.
    let tree = RealSMT::new(depth);
    let account = gen_account(0);

    let setup = || tree.clone();

    b.iter_batched_ref(
        setup,
        |tree| {
            let id = 0;
            tree.insert(black_box(id), account.clone());
        },
        BatchSize::SmallInput,
    );
}

/// Measures the time of insertion into a non-empty SMT as the last element.
fn smt_insert_filled_end(b: &mut Bencher<'_>) {
    let depth = zksync_crypto::params::account_tree_depth();

    // Create a tree and fill it with some accounts.
    let mut tree = RealSMT::new(depth);
    for (id, account) in (0..N_ACCOUNTS).map(gen_account).enumerate() {
        let id = id as u32;
        tree.insert(id, account.clone())
    }
    let latest_account = gen_account(N_ACCOUNTS);

    let setup = || tree.clone();

    b.iter_batched_ref(
        setup,
        |tree| {
            let id = N_ACCOUNTS;
            tree.insert(black_box(id), latest_account.clone());
        },
        BatchSize::SmallInput,
    );
}

/// Measures the time of insertion into a non-empty SMT in between several elements.
fn smt_insert_filled_middle(b: &mut Bencher<'_>) {
    let depth = zksync_crypto::params::account_tree_depth();

    let target_id = N_ACCOUNTS / 2;

    // Create a tree and fill it with some accounts.
    let mut tree = RealSMT::new(depth);
    for (id, account) in (0..N_ACCOUNTS).map(gen_account).enumerate() {
        let id = id as u32;
        if id == target_id {
            continue;
        }

        tree.insert(id, account.clone())
    }
    let latest_account = gen_account(N_ACCOUNTS);

    let setup = || tree.clone();

    b.iter_batched_ref(
        setup,
        |tree| {
            tree.insert(black_box(target_id), latest_account.clone());
        },
        BatchSize::SmallInput,
    );
}

/// Measures the time of obtaining a SMT root hash.
fn smt_root_hash(b: &mut Bencher<'_>) {
    let depth = zksync_crypto::params::account_tree_depth();

    // Create a tree and fill it with some accounts.
    let mut tree = RealSMT::new(depth);
    for (id, account) in (0..N_ACCOUNTS).map(gen_account).enumerate() {
        let id = id as u32;
        tree.insert(id, account.clone());
    }

    let setup = || (tree.clone());

    b.iter_batched_ref(
        setup,
        |tree| {
            let _hash = black_box(tree.root_hash());
        },
        BatchSize::SmallInput,
    );
}

/// Measures the time of obtaining a SMT root hash with `root_hash` invoked
/// when 50% of accounts are inserted.
///
/// This bench is expected to get better results than `smt_root_hash` due
/// to some hashes being cached.
fn smt_root_hash_cached(b: &mut Bencher<'_>) {
    let depth = zksync_crypto::params::account_tree_depth();

    // Create a tree and fill it with some accounts.
    let mut tree = RealSMT::new(depth);
    for (id, account) in (0..N_ACCOUNTS).map(gen_account).enumerate() {
        let id = id as u32;
        tree.insert(id, account.clone());

        if id == N_ACCOUNTS / 2 {
            // Calculate the root hash to create cache.
            let _ = tree.root_hash();
        }
    }

    let setup = || (tree.clone());

    b.iter_batched_ref(
        setup,
        |tree| {
            let _hash = black_box(tree.root_hash());
        },
        BatchSize::SmallInput,
    );
}

pub fn bench_merkle_tree(c: &mut Criterion) {
    c.bench_function("account.clone()", account_clone);
    c.bench_function("Parallel SMT create", smt_create);

    // Insert benchmarks.
    c.bench_function("Parallel SMT insert (empty)", smt_insert_empty);
    c.bench_function(
        "Parallel SMT insert (filled, at end)",
        smt_insert_filled_end,
    );
    c.bench_function(
        "Parallel SMT insert (filled, at middle)",
        smt_insert_filled_middle,
    );

    // Root hash benchmarks.
    c.bench_function("Parallel SMT root hash", smt_root_hash);
    c.bench_function("Parallel SMT root hash (half-cached)", smt_root_hash_cached);
}
