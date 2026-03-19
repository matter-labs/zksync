//! Target code to analyze tree performance.

use std::time::Instant;

use zksync_crypto::params::ACCOUNT_TREE_DEPTH;
use zksync_types::{Account, AccountTree, Address, TokenId};

type Tree = AccountTree;

// This binary is a playground, so feel free to change the params to achieve behavior you want.
const INITIALIZED_VALUES: usize = 100;
const N_CYCLES: usize = 50;
const VALUES_PER_CYCLE: usize = 50;

macro_rules! measured {
    ($descr: expr, $tree: expr, $call: tt) => {{
        let start = Instant::now();
        $call(&mut $tree);
        println!("{}: took {:?}", $descr, start.elapsed());
    }};
}

/// An entry point for analysis.
pub(crate) fn analyze_tree() {
    let mut tree = Tree::new(ACCOUNT_TREE_DEPTH);

    measured!("prepare", tree, prepare);
    measured!("stress_get", tree, stress_get);
    measured!("stress_affected", tree, stress_affected);
    measured!("stress_new", tree, stress_new);
    measured!("stress_get", tree, stress_get);
    let root_hash = tree.root_hash();
    println!("Root hash: {:?}", root_hash);
    drop(tree);
}

fn prepare(tree: &mut Tree) {
    for val in 0..INITIALIZED_VALUES {
        let mut account = Account::default_with_address(&Address::from_low_u64_le(val as u64));
        // Fill some additional balances to the account tree.
        for additional_balance in 0..INITIALIZED_VALUES {
            account.add_balance(
                TokenId(val.wrapping_mul(additional_balance) as u32),
                &(val as u64).into(),
            );
        }
        account.add_balance(TokenId(val as u32), &(val as u64).into());
        tree.insert(val as u32, account);
    }
    tree.root_hash();
}

fn stress_get(tree: &mut Tree) {
    const START_FROM: usize = INITIALIZED_VALUES / 2;
    for _ in 0..N_CYCLES {
        for val in START_FROM..(START_FROM + VALUES_PER_CYCLE) {
            // Insert some new value.
            let got = tree.get(val as u32);
            // Deny optimizing this stuff out.
            assert!(got.is_some());
        }
    }
}

fn stress_affected(tree: &mut Tree) {
    // Range contains affected accounts and rewrites them multiple time.
    const START_FROM: usize = INITIALIZED_VALUES / 2;
    for cycle in 0..N_CYCLES {
        for val in START_FROM..(START_FROM + VALUES_PER_CYCLE) {
            // Insert some new value.
            let mut got = tree.get(val as u32).unwrap().clone();
            got.add_balance(TokenId(val as u32), &(val as u64 + cycle as u64).into());
            tree.insert(val as u32, got);
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
        let mut account = Account::default_with_address(&Address::from_low_u64_le(val as u64));
        account.add_balance(TokenId(val as u32), &(val as u64).into());
        tree.insert(val as u32, account);
    }
}
