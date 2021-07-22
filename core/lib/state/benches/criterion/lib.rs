use criterion::criterion_main;

use ops::ops_benches;

mod ops;

criterion_main!(ops_benches);
