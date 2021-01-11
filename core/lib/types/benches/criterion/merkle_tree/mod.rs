use criterion::criterion_group;

use self::parallel_smt::bench_merkle_tree as bench_parallel_smt;
use self::rescue_hasher::bench_rescue_hasher;

mod parallel_smt;
mod rescue_hasher;

criterion_group!(merkle_tree_benches, bench_parallel_smt, bench_rescue_hasher);
