use crate::merkle_tree::{parallel_smt, sequential_smt, PedersenHasher};
use crate::node::{Engine, Fr};
use crypto_exports::rand::{Rng, SeedableRng, XorShiftRng};

#[test]
fn random_compare_par_tree_seq_tree() {
    let depth = 8;

    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);
    let elements = rng.gen_iter::<u64>().take(1 << depth).collect::<Vec<_>>();

    let mut par_tree =
        parallel_smt::SparseMerkleTree::<u64, Fr, PedersenHasher<Engine>>::new(depth);
    let mut seq_tree =
        sequential_smt::SparseMerkleTree::<u64, Fr, PedersenHasher<Engine>>::new(depth as u32);

    for (idx, item) in elements.into_iter().enumerate() {
        let idx = idx as u32;
        par_tree.insert(idx, item);
        seq_tree.insert(idx, item);
        assert_eq!(
            par_tree.root_hash(),
            seq_tree.root_hash(),
            "root_hahs() idx: {}, item: {}",
            idx,
            item
        );

        let par_merkle_path = par_tree.merkle_path(idx);
        let seq_merkle_path = seq_tree.merkle_path(idx);

        assert_eq!(
            par_merkle_path, seq_merkle_path,
            "merkle_path() idx: {}",
            idx
        );
    }
}

#[test]
fn simulate_transfer_to_new_par_tree_seq_tree() {
    let depth = 3;

    let mut par_tree =
        parallel_smt::SparseMerkleTree::<u64, Fr, PedersenHasher<Engine>>::new(depth);
    let mut seq_tree =
        sequential_smt::SparseMerkleTree::<u64, Fr, PedersenHasher<Engine>>::new(depth as u32);

    let from_account_id = 1;
    let from_account_before_bal = 5;

    let to_account_id = 2;

    let (par_root_before, par_audit_to_before) = {
        let tree = &mut par_tree;
        tree.insert(from_account_id, from_account_before_bal);
        let root_before = tree.root_hash();
        let audit_to_before = tree.merkle_path(to_account_id);
        (root_before, audit_to_before)
    };

    let (seq_root_before, seq_audit_to_before) = {
        let tree = &mut seq_tree;
        tree.insert(from_account_id, from_account_before_bal);
        let root_before = tree.root_hash();
        let audit_to_before = tree.merkle_path(to_account_id);
        (root_before, audit_to_before)
    };

    assert_eq!(par_root_before, seq_root_before);
    assert_eq!(par_audit_to_before, seq_audit_to_before);
}
