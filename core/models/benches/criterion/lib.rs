use criterion::{criterion_group, criterion_main};

use crate::parallel_smt::bench_merkle_tree as parallel_smt_bench;
use crate::sequential_smt::bench_merkle_tree as sequential_smt_bench;

mod parallel_smt;
mod sequential_smt;

criterion_group!(benches, sequential_smt_bench, parallel_smt_bench);
criterion_main!(benches);
