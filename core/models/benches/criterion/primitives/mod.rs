use criterion::{criterion_group, Criterion};

pub fn bench_primitives(_c: &mut Criterion) {}

criterion_group!(primitives_benches, bench_primitives);
