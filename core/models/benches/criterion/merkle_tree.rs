use criterion::{black_box, BatchSize, Bencher, Criterion};

use models::circuit::account::CircuitAccount;
use models::franklin_crypto::bellman::pairing::bn256::{Bn256, Fr};
use models::merkle_tree::{PedersenHasher, SparseMerkleTree};

const N_ACCOUNTS: u64 = 10;

type RealSMT = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;

fn gen_account(id: u64) -> CircuitAccount<Bn256> {
    let mut account = CircuitAccount::<Bn256>::default();

    let id_hex = format!("{:064x}", id);
    account.address = Fr::from_hex(id_hex.as_ref()).unwrap();

    account
}

fn bench_tree_create(b: &mut Bencher<'_>) {
    let depth = models::params::account_tree_depth() as u32;

    b.iter(|| {
        RealSMT::new(black_box(depth));
    });
}

fn bench_tree_insert(b: &mut Bencher<'_>) {
    let depth = models::params::account_tree_depth() as u32;

    let setup = || (0..N_ACCOUNTS).map(gen_account).collect::<Vec<_>>();

    b.iter_batched(
        setup,
        |accounts| {
            let mut tree = RealSMT::new(depth);

            for (id, account) in accounts.into_iter().enumerate() {
                tree.insert(id as u32, account);
            }
        },
        BatchSize::SmallInput,
    );
}

pub fn bench_merkle_tree(c: &mut Criterion) {
    c.bench_function("Merkle tree create", bench_tree_create);
    c.bench_function("Merkle tree insert", bench_tree_insert);
}
