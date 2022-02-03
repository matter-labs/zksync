//! Target code to analyze tree performance.

use zksync_crypto::{
    merkle_tree::{RescueHasher, SparseMerkleTree},
    params::ACCOUNT_TREE_DEPTH,
    Engine, Fr,
};

type Value = u64;
type Tree = SparseMerkleTree<u64, Fr, RescueHasher<Engine>>;

// This binary is a playground, so feel free to change the params to achieve behavior you want.
const INITIALIZED_VALUES: usize = 100_000;
const N_CYCLES: usize = 1_000;
const VALUES_PER_CYCLE: usize = 1_000;

/// An entry point for analysis.
pub(crate) fn analyze_tree() {
    let mut tree = Tree::new(ACCOUNT_TREE_DEPTH);

    prepare(&mut tree);
    stress_get(&mut tree);
    stress_affected(&mut tree);
    stress_new(&mut tree);
    drop(tree);
}

fn prepare(tree: &mut Tree) {
    for val in 0..INITIALIZED_VALUES {
        tree.insert(val as u32, val as Value);
    }
    tree.root_hash();
}

fn stress_get(tree: &mut Tree) {
    const START_FROM: usize = INITIALIZED_VALUES / 2;
    for _ in 0..N_CYCLES {
        for val in START_FROM..(START_FROM + VALUES_PER_CYCLE) {
            // Insert some new value.
            let got = tree.get(val as u32).copied();
            // Deny optimizing this stuff out.
            assert_eq!(got, Some(val as Value));
        }
    }
}

fn stress_affected(tree: &mut Tree) {
    // Range contains affected accounts and rewrites them multiple time.
    const START_FROM: usize = INITIALIZED_VALUES / 2;
    for cycle in 0..N_CYCLES {
        for val in START_FROM..(START_FROM + VALUES_PER_CYCLE) {
            // Insert some new value.
            tree.insert(val as u32, (val + cycle) as Value);
        }
        // Recalculate the root hash.
        tree.root_hash();
    }
}

fn stress_new(tree: &mut Tree) {
    // Range is outside of where we have been putting out elements.
    // Each account is updated one time.
    const START_FROM: usize = INITIALIZED_VALUES * 100;
    for val in START_FROM..(START_FROM + INITIALIZED_VALUES) {
        // Each ID is multiplied by 2, so that they are not neighbors and the access
        // is somewhat sparse.
        tree.insert((val * 2) as u32, val as Value);
    }
}
