use criterion::criterion_group;

use self::parallel_smt::bench_merkle_tree as bench_parallel_smt;
use self::pedersen_hasher::bench_pedersen_hasher;
use self::sequential_smt::bench_merkle_tree as bench_sequential_smt;

mod parallel_smt;
mod pedersen_hasher;
mod sequential_smt;

criterion_group!(
    merkle_tree_benches,
    bench_parallel_smt,
    bench_sequential_smt,
    bench_pedersen_hasher,
);
