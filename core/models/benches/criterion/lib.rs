use criterion::{criterion_group, criterion_main};

use crate::merkle_tree::bench_merkle_tree;

mod merkle_tree;

criterion_group!(benches, bench_merkle_tree);
criterion_main!(benches);
