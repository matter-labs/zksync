use criterion::criterion_main;

use merkle_tree::merkle_tree_benches;
use primitives::primitives_benches;

mod merkle_tree;
mod primitives;

criterion_main!(merkle_tree_benches, primitives_benches);
