use super::hasher::Hasher;
use crate::merkle_tree::{parallel_smt, sequential_smt, RescueHasher};
use crate::primitives::GetBits;
use crate::rand::{Rng, SeedableRng, XorShiftRng};
use crate::{Engine, Fr};

/// Applies the proof for the element and compares it against the expected
/// root hash.
fn verify_proof<T>(
    element_index: u64,
    element: T,
    hasher: RescueHasher<Engine>,
    merkle_proof: Vec<(Fr, bool)>,
    expected_root: Fr,
) where
    T: GetBits,
{
    // To check the proof, we fold it starting from the hash of the value
    // and updating with the hashes from the proof.
    // We should obtain the root hash at the end if the proof is correct.
    let mut proof_index = 0;
    let mut aggregated_hash = hasher.hash_bits(element.get_bits_le());
    for (level, (hash, dir)) in merkle_proof.into_iter().enumerate() {
        let (lhs, rhs) = if dir {
            proof_index |= 1 << level;
            (hash, aggregated_hash)
        } else {
            (aggregated_hash, hash)
        };

        aggregated_hash = hasher.compress(&lhs, &rhs, level);
    }

    assert_eq!(
        proof_index, element_index,
        "Got unexpected element index while verifying the proof"
    );
    assert_eq!(
        aggregated_hash, expected_root,
        "Got unexpected root hash while verifying the proof"
    );
}

/// Verifies that for a randomly-chosen sequence of elements
/// the merkle paths provided by both sequential and parallel trees
/// are equal.
#[test]
fn cross_trees_merkle_path_comparison() {
    let depth = 8;

    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);
    let elements = rng.gen_iter::<u64>().take(1 << depth);

    let mut par_tree = parallel_smt::SparseMerkleTree::<u64, Fr, RescueHasher<Engine>>::new(depth);
    let mut seq_tree =
        sequential_smt::SparseMerkleTree::<u64, Fr, RescueHasher<Engine>>::new(depth);

    for (idx, item) in elements.enumerate() {
        // Insert the same element in both trees and verify that the root hash is the same.
        let idx = idx as u32;
        par_tree.insert(idx, item);
        seq_tree.insert(idx as usize, item);
        assert_eq!(
            par_tree.root_hash(),
            seq_tree.root_hash(),
            "Root hashes for seq/par trees diverged, element idx: {}, item: {}",
            idx,
            item
        );

        let par_merkle_path = par_tree.merkle_path(idx);
        let seq_merkle_path = seq_tree.merkle_path(idx as usize);

        // Check that proofs are equal.
        assert_eq!(
            par_merkle_path, seq_merkle_path,
            "Merkle paths for seq/par trees diverged, element idx: {}",
            idx
        );

        // Check that verifying proofs provides expected results.
        verify_proof(
            idx as u64,
            item,
            seq_tree.hasher.clone(),
            seq_merkle_path,
            seq_tree.root_hash(),
        );

        verify_proof(
            idx as u64,
            item,
            par_tree.hasher.clone(),
            par_merkle_path,
            par_tree.root_hash(),
        );
    }
}

/// Simulates a transfer operation, then obtains the
/// proof for the element absent in the tree and compares
/// the proofs between sequential and parallel trees.
#[test]
fn simulate_transfer_to_new_par_tree_seq_tree() {
    let depth = 3;

    let mut par_tree = parallel_smt::SparseMerkleTree::<u64, Fr, RescueHasher<Engine>>::new(depth);
    let mut seq_tree =
        sequential_smt::SparseMerkleTree::<u64, Fr, RescueHasher<Engine>>::new(depth);

    let from_account_id = 1;
    let from_account_before_bal = 5;

    let to_account_id = 2;

    // First, we insert the element to the both trees, and then
    // we get the proof for the element which is absent in the tree.

    let (par_root_before, par_audit_to_before) = {
        let tree = &mut par_tree;
        tree.insert(from_account_id, from_account_before_bal);
        let root_before = tree.root_hash();
        let audit_to_before = tree.merkle_path(to_account_id);
        (root_before, audit_to_before)
    };

    let (seq_root_before, seq_audit_to_before) = {
        let tree = &mut seq_tree;
        tree.insert(from_account_id as usize, from_account_before_bal);
        let root_before = tree.root_hash();
        let audit_to_before = tree.merkle_path(to_account_id as usize);
        (root_before, audit_to_before)
    };

    assert_eq!(par_root_before, seq_root_before);
    assert_eq!(par_audit_to_before, seq_audit_to_before);

    // Check the sequential tree proof.
    let element_idx = to_account_id as u64;
    let element: u64 = 0;
    verify_proof(
        element_idx,
        element,
        seq_tree.hasher.clone(),
        seq_audit_to_before,
        seq_tree.root_hash(),
    );

    // Check the parallel tree proof.
    verify_proof(
        element_idx,
        element,
        par_tree.hasher.clone(),
        par_audit_to_before,
        par_tree.root_hash(),
    );
}
