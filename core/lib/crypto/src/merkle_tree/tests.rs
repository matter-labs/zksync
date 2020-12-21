use super::hasher::Hasher;
use crate::merkle_tree::{parallel_smt, sequential_smt, RescueHasher};
use crate::primitives::GetBits;
use crate::rand::{Rng, SeedableRng, XorShiftRng};
use crate::{Engine, Fr};
use serde::{Deserialize, Serialize};

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

/// Checks if root and path from middle leaf of merkle tree with pre-defined elements is correct.
#[test]
fn small_input_and_middle_leaf() {
    use crate::ff::from_hex;
    let depth = 3;
    let elements = vec![1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, 8u64];
    let root_hashes: Vec<Fr> = vec![
        from_hex("0x18ddd3700477ce74623ee03932d61dba8e754946e8a67b61a02058f271e37599").unwrap(),
        from_hex("0x0761a365c4a300023c342e2f5b1182378f8110fa4d9eb8ab8d07e032ec7fadd6").unwrap(),
        from_hex("0x06e82852a69c830b7107680684f3fd0bc6eb055f4ee959119735baf569ea0a67").unwrap(),
        from_hex("0x2a7d3529a1fb32fac22dbbe7d73e4b12f5455adde1ae284471001133f0d9cdac").unwrap(),
        from_hex("0x25b59668cd551bf48c8b71af61fd5b3c47859e52f21338ad50dd33ab293b6ffc").unwrap(),
        from_hex("0x156682ffffccde9b2f4826b1065fb13a318a9ff63030d0688fa2937fc12cdf42").unwrap(),
        from_hex("0x1979d0a3a964a32f19baa9e59a13383a8da1c7ef775b22d1e96ecf858eda5b44").unwrap(),
        from_hex("0x1ed2242a760f91b8c3215f7fcce49f77362021a054c94d421b3859691e19e0af").unwrap(),
    ];
    let index_to_find_path = 3u32;
    let path: Vec<(Fr, bool)> = vec![
        (
            from_hex("0x096dcaf26d7018c81028431321c744817fbd30825051ebf6d6612d7ac9179c77").unwrap(),
            true,
        ),
        (
            from_hex("0x062262455eee2e6e7c14081ae6140ebea14afa0e3c30c5571c518334bc43e227").unwrap(),
            true,
        ),
        (
            from_hex("0x1a25a862ab27ac4048b76818378ea9acd53971efc91616ad874e42a237eee103").unwrap(),
            false,
        ),
    ];

    let mut par_tree = parallel_smt::SparseMerkleTree::<u64, Fr, RescueHasher<Engine>>::new(depth);
    let mut seq_tree =
        sequential_smt::SparseMerkleTree::<u64, Fr, RescueHasher<Engine>>::new(depth);

    for (idx, item) in elements.iter().enumerate() {
        par_tree.insert(idx as u32, *item);
        seq_tree.insert(idx, *item);
        assert_eq!(root_hashes[idx], par_tree.root_hash());
        assert_eq!(root_hashes[idx], seq_tree.root_hash());
    }
    assert_eq!(path, par_tree.merkle_path(index_to_find_path));
}

/// Checks if root and path from leftmost leaf of merkle tree with pre-defined elements is correct.
#[test]
fn small_input_and_leftmost_leaf() {
    use crate::ff::from_hex;
    let depth = 3;
    let elements = vec![
        52987011536924u64,
        30064785464u64,
        108088250798717285u64,
        90073581693712538u64,
        75502028985968359u64,
        57838933031571551u64,
        6990032406494164777u64,
        3948052069168408798u64,
    ];
    let root_hashes: Vec<Fr> = vec![
        from_hex("0x1839cadd698dcea410ecb73d700b27198798b6b5c8e7d01f92d9e2cc37d4160f").unwrap(),
        from_hex("0x1efe48f5d8432d5b9557503feb95ba4fea7e30a9a30c9c411315937927c2b15f").unwrap(),
        from_hex("0x0975cf0b6e47a02f3d7a723d8acebeaa1423cdf6357352d4510f28e8dd81f1c8").unwrap(),
        from_hex("0x22769902120e7179f0c8dc921f71fc8aec2cf1d4c8f29d9b67ea3f8bebf970bc").unwrap(),
        from_hex("0x1679767b0e70ac79b440723e02c0ba13c6b740dd678f411cc21ef9edc9cce978").unwrap(),
        from_hex("0x03becf46b5610ce51329c26f7ef4644a7b03fd7838245007423ffdd949f0fade").unwrap(),
        from_hex("0x24bd245c140b29705c093484ed7b96867cab1dfa5f66a350e12923a5b3eb72db").unwrap(),
        from_hex("0x19836d1f4c10a9261f5a5bcf9f234d3f24e9d9fb3753953abbc8513b16a24f34").unwrap(),
    ];
    let index_to_find_path = 0u32;
    let path: Vec<(Fr, bool)> = vec![
        (
            from_hex("0x257fe7723ab34030ed0c9303601163e50046c6cf46d8b7fa30da763c3a8c2e9d").unwrap(),
            false,
        ),
        (
            from_hex("0x2a41acfa8a0fcc9d0f7feacc3c534a43d5b1494e7d3cb88e712c5da4ac159cd5").unwrap(),
            false,
        ),
        (
            from_hex("0x1c633d9705fb0a4303fce66f5f17987865d7a37e5ce3e559031b99fb629c607d").unwrap(),
            false,
        ),
    ];

    let mut par_tree = parallel_smt::SparseMerkleTree::<u64, Fr, RescueHasher<Engine>>::new(depth);
    let mut seq_tree =
        sequential_smt::SparseMerkleTree::<u64, Fr, RescueHasher<Engine>>::new(depth);

    for (idx, item) in elements.iter().enumerate() {
        par_tree.insert(idx as u32, *item);
        seq_tree.insert(idx, *item);
        assert_eq!(root_hashes[idx], par_tree.root_hash());
        assert_eq!(root_hashes[idx], seq_tree.root_hash());
    }
    assert_eq!(path, par_tree.merkle_path(index_to_find_path));
}

#[derive(Serialize, Deserialize)]
struct InputData {
    depth: usize,
    elements: Vec<u64>,
    root_hash: String,
}

/// Checks if root of merkle tree with height 11 and pre-defined elements is correct.
#[test]
fn big_test() {
    let input_str = include_str!("big_test.json");
    let input: InputData = serde_json::from_str(&input_str).unwrap();
    let mut par_tree =
        parallel_smt::SparseMerkleTree::<u64, Fr, RescueHasher<Engine>>::new(input.depth);

    for (idx, item) in input.elements.iter().enumerate() {
        par_tree.insert(idx as u32, *item);
    }
    let root_hash: Fr = crate::ff::from_hex(&input.root_hash).unwrap();
    assert_eq!(root_hash, par_tree.root_hash());
}
