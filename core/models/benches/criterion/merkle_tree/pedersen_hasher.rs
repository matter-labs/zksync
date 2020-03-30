//! Benchmarks for the Parallel Sparse Merkle Tree.

use criterion::{black_box, BatchSize, Bencher, Criterion};
use models::franklin_crypto::bellman::pairing::bn256::Bn256;
use models::merkle_tree::{hasher::Hasher, PedersenHasher};

/// Creates a boolean vector for `PedersonHasher` input.
fn generate_input(size: usize) -> Vec<bool> {
    (0..size).map(|i| i % 2 == 0).collect()
}

/// Measures the hashing time for a small input.
fn pedersen_small(b: &mut Bencher<'_>) {
    const INPUT_SIZE: usize = 8; // 1 byte.

    let hasher = PedersenHasher::<Bn256>::default();
    let input: Vec<bool> = generate_input(INPUT_SIZE);

    let setup = || (hasher.clone(), input.clone());

    b.iter_batched(
        setup,
        |(hasher, input)| {
            let _ = hasher.hash_bits(black_box(input));
        },
        BatchSize::SmallInput,
    );
}

/// Measures the hashing time for a (relatively) big input.
fn pedersen_big(b: &mut Bencher<'_>) {
    const INPUT_SIZE: usize = models::params::MAX_CIRCUIT_PEDERSEN_HASH_BITS; // Biggest supported size.

    let hasher = PedersenHasher::<Bn256>::default();
    let input: Vec<bool> = generate_input(INPUT_SIZE);

    let setup = || (hasher.clone(), input.clone());

    b.iter_batched(
        setup,
        |(hasher, input)| {
            let _ = hasher.hash_bits(black_box(input));
        },
        BatchSize::SmallInput,
    );
}

pub fn bench_pedersen_hasher(c: &mut Criterion) {
    c.bench_function("Pedersen Hasher small input", pedersen_small);
    c.bench_function("Pedersen Hasher big input", pedersen_big);
}
