//! Benchmarks for the Parallel Sparse Merkle Tree.

use criterion::{black_box, BatchSize, Bencher, Criterion, Throughput};
use models::franklin_crypto::bellman::pairing::bn256::Bn256;
use models::merkle_tree::{hasher::Hasher, PedersenHasher};

const SMALL_INPUT_SIZE: usize = 16; // 16 bits / 2 bytes
const BIG_INPUT_SIZE: usize = models::params::MAX_CIRCUIT_PEDERSEN_HASH_BITS; // Biggest supported size.

/// Creates a boolean vector for `PedersonHasher` input.
fn generate_input(size: usize) -> Vec<bool> {
    (0..size).map(|i| i % 2 == 0).collect()
}

/// Measures the hashing time for a small input.
fn pedersen_small(b: &mut Bencher<'_>) {
    const INPUT_SIZE: usize = SMALL_INPUT_SIZE;

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
    const INPUT_SIZE: usize = BIG_INPUT_SIZE;

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
    let mut small_input_group = c.benchmark_group("Small input");
    small_input_group.throughput(Throughput::Bytes((SMALL_INPUT_SIZE / 8) as u64));
    small_input_group.bench_function("Pedersen Hasher", pedersen_small);
    small_input_group.finish();

    let mut big_input_group = c.benchmark_group("Big input");
    big_input_group.throughput(Throughput::Bytes((BIG_INPUT_SIZE / 8) as u64));
    big_input_group.bench_function("Pedersen Hasher", pedersen_big);
    big_input_group.finish();
}
