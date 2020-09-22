use criterion::criterion_main;

use merkle_tree::merkle_tree_benches;
use primitives::primitives_benches;
use signatures::signature_benches;

mod merkle_tree;
mod primitives;
mod signatures;

criterion_main!(merkle_tree_benches, primitives_benches, signature_benches);
