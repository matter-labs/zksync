use criterion::criterion_main;

use merkle_tree::merkle_tree_benches;

mod merkle_tree;

criterion_main!(merkle_tree_benches);
