//! Benchmarks for the Account Tree (used in the state module).
//! The difference is that it uses `Account` as a leaf element instead of `CircuitAccount`.

use criterion::{black_box, BatchSize, Bencher, Criterion};

use zksync_basic_types::{Address, TokenId};
use zksync_types::{Account, AccountTree};

// This value should be not to high, since the bench will be run for thousands
// of iterations. Despite the tree cloning time won't affect the bench results
// (cloning is performed within `setup` closure), the bench will take forever to
// be completed if the value is too big.
const N_ACCOUNTS: u32 = 100;

// Token to change balance within the account (so that only 1 item per balance tree is changed).
const TOUCHED_TOKEN: TokenId = TokenId(42);

/// Type alias equivalent to the actually used SMT (but parallel tree is used instead of sequential).
type RealSMT = AccountTree;

fn gen_account(id: u32) -> Account {
    let mut acc = Account::default_with_address(&Address::from_low_u64_le(id as u64));

    acc.add_balance(TOUCHED_TOKEN, &100u64.into());
    for i in 1..=20 {
        acc.add_balance(TokenId(i * 100), &i.into());
    }

    acc
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
fn smt_root_hash(b: &mut Bencher<'_>, size: u32) {
    let depth = zksync_crypto::params::account_tree_depth();

    // Create a tree and fill it with some accounts.
    let mut tree = RealSMT::new(depth);
    for (id, account) in (0..size).map(gen_account).enumerate() {
        let id = id as u32;
        tree.insert(id, account.clone());
    }

    let setup = || tree.clone();

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
fn smt_root_hash_cached(b: &mut Bencher<'_>, size: u32) {
    let depth = zksync_crypto::params::account_tree_depth();

    // Create a tree and fill it with some accounts.
    let mut tree = RealSMT::new(depth);
    for (id, account) in (0..size).map(gen_account).enumerate() {
        let id = id as u32;
        tree.insert(id, account);
    }

    // Calculate the root hash to create cache.
    let _ = tree.root_hash();

    for id in 0..(size / 2) {
        let mut acc = tree.get(id).unwrap().clone();
        // Increase the balance of a single token to invalidate cache.
        acc.add_balance(TOUCHED_TOKEN, &100u64.into());
        tree.insert(id, acc);
    }

    let setup = || tree.clone();

    b.iter_batched_ref(
        setup,
        |tree| {
            let _hash = black_box(tree.root_hash());
        },
        BatchSize::SmallInput,
    );
}

/// Measures the time to `drop` a tree with calculated cache.
fn smt_drop(b: &mut Bencher<'_>, size: u32) {
    let depth = zksync_crypto::params::account_tree_depth();

    // Create a tree and fill it with some accounts.
    let mut tree = RealSMT::new(depth);
    for (id, account) in (0..size).map(gen_account).enumerate() {
        let id = id as u32;
        tree.insert(id, account.clone());
    }
    tree.root_hash();

    let setup = || tree.clone();

    b.iter_batched(
        setup,
        |tree| {
            drop(tree);
        },
        BatchSize::SmallInput,
    );
}

pub fn bench_merkle_tree(c: &mut Criterion) {
    c.bench_function("AccountTree account.clone()", account_clone);
    c.bench_function("AccountTree create", smt_create);

    // Insert benchmarks.
    c.bench_function("AccountTree insert (empty)", smt_insert_empty);
    c.bench_function("AccountTree insert (filled, at end)", smt_insert_filled_end);
    c.bench_function(
        "AccountTree insert (filled, at middle)",
        smt_insert_filled_middle,
    );

    // Root hash benchmarks.
    for tree_size in &[10, 100] {
        let bench_name = format!("AccountTree root hash / size {}", tree_size);
        c.bench_function(&bench_name, |b| smt_root_hash(b, *tree_size));
    }
    for tree_size in &[10, 100] {
        let bench_name = format!("AccountTree root hash (half-cached) / size {}", tree_size);
        c.bench_function(&bench_name, |b| smt_root_hash_cached(b, *tree_size));
    }

    // Drop benchmarks.
    for tree_size in &[10, 100, 1000] {
        let bench_name = format!("AccountTree drop / size {}", tree_size);
        c.bench_function(&bench_name, |b| smt_drop(b, *tree_size));
    }
}
