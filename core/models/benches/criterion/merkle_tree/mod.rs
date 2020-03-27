use criterion::criterion_group;

use self::parallel_smt::bench_merkle_tree as parallel_smt_bench;
use self::sequential_smt::bench_merkle_tree as sequential_smt_bench;

mod parallel_smt;
mod pedersen_hasher;
mod sequential_smt;

criterion_group!(
    merkle_tree_benches,
    sequential_smt_bench,
    parallel_smt_bench
);
