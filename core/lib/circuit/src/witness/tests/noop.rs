// External deps
use zksync_crypto::franklin_crypto::{
    alt_babyjubjub::AltJubjubBn256,
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::{Field, PrimeField},
    },
    eddsa::{PrivateKey, PublicKey},
    jubjub::FixedGenerators,
    rescue::bn256::Bn256RescueParams,
};
use zksync_crypto::rand::{Rng, SeedableRng, XorShiftRng};
// Workspace deps
use zksync_crypto::{
    circuit::{
        account::{Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree},
        utils::pub_key_hash_fe,
    },
    merkle_tree::RescueHasher,
    params::{self, account_tree_depth, used_account_subtree_depth},
};

// Local deps
use crate::{
    circuit::ZkSyncCircuit,
    witness::{
        noop::noop_operation,
        tests::test_utils::{check_circuit, check_circuit_non_panicking},
        utils::{apply_fee, get_audits, get_used_subtree_root_hash, public_data_commitment},
        WitnessBuilder,
    },
};

/// Creates a random private key and returns a public key hash for it.
fn generate_pubkey_hash(
    rng: &mut XorShiftRng,
    p_g: FixedGenerators,
    jubjub_params: &AltJubjubBn256,
    phasher: &RescueHasher<Bn256>,
) -> Fr {
    let sk = PrivateKey::<Bn256>(rng.gen());
    let pk = PublicKey::from_private(&sk, p_g, jubjub_params);
    pub_key_hash_fe(&pk, phasher)
}

fn insert_validator(
    tree: &mut CircuitAccountTree,
    rng: &mut XorShiftRng,
    p_g: FixedGenerators,
    jubjub_params: &AltJubjubBn256,
    phasher: &RescueHasher<Bn256>,
) -> (u32, Fr, Vec<Option<Fr>>) {
    // Validator account credentials
    let validator_address_number = 7;
    let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
    let validator_pub_key_hash = generate_pubkey_hash(rng, p_g, &jubjub_params, &phasher);

    // Create a validator account as an account tree leaf.
    let validator_leaf = CircuitAccount::<Bn256> {
        subtree: CircuitBalanceTree::new(params::balance_tree_depth()),
        nonce: Fr::zero(),
        pub_key_hash: validator_pub_key_hash,
        address: Fr::zero(),
    };

    // Initialize all the validator balances as 0.
    let empty_balance = Some(Fr::zero());
    let validator_balances = vec![empty_balance; params::total_tokens()];

    // Insert account into tree.
    tree.insert(validator_address_number, validator_leaf);

    (
        validator_address_number,
        validator_address,
        validator_balances,
    )
}

fn insert_sender(
    tree: &mut CircuitAccountTree,
    rng: &mut XorShiftRng,
    p_g: FixedGenerators,
    jubjub_params: &AltJubjubBn256,
    phasher: &RescueHasher<Bn256>,
) {
    let sender_address: u32 = rng.gen::<u32>() % 2u32.pow(used_account_subtree_depth() as u32);
    let sender_balance_token_id: u32 = 2;
    let sender_balance_value: u128 = 2000;
    let sender_balance = Fr::from_str(&sender_balance_value.to_string()).unwrap();
    let sender_pub_key_hash = generate_pubkey_hash(rng, p_g, &jubjub_params, &phasher);

    // Create a sender account as an account tree leaf.
    // Balance tree of this account will only contain one token with non-zero amount of funds.
    let mut sender_balance_tree = CircuitBalanceTree::new(params::balance_tree_depth());
    sender_balance_tree.insert(
        sender_balance_token_id,
        Balance {
            value: sender_balance,
        },
    );

    let sender_leaf_initial = CircuitAccount::<Bn256> {
        subtree: sender_balance_tree,
        nonce: Fr::zero(),
        pub_key_hash: sender_pub_key_hash,
        address: Fr::zero(),
    };

    // Insert both accounts into a tree.
    tree.insert(sender_address, sender_leaf_initial);
}

/// Low-level test for circuit based on a non-operation execution.
/// In this test we manually create an account tree, add a validator account,
/// an account with a non-zero balance in tokens, and apply
/// a `noop` operation to the validator account.
/// After that, we check that circuit doesn't contain any unsatisfied constraints.
#[test]
#[ignore]
fn test_noop() {
    let mut circuit_account_tree = CircuitAccountTree::new(account_tree_depth());
    circuit_account_tree.insert(0, CircuitAccount::default());

    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, 0, 1);
    witness_accum.extend_pubdata_with_noops(1);
    witness_accum.collect_fees(&[]);
    witness_accum.calculate_pubdata_commitment();

    let circuit_instance = witness_accum.into_circuit_instance();
    // Check that there are no unsatisfied constraints.
    check_circuit(circuit_instance);
}

/// Test for the incorrect values being fed to the circuit via the provided
/// pubdata.
///
/// The following checks are performed:
/// - Incorrect old root hash in `pub_data_commitment`,
/// - Incorrect new root hash in `pub_data_commitment`,
/// - Incorrect old root hash in `ZkSyncCircuit`,
/// - Incorrect old root hash in both `pub_data_commitment` and `ZkSyncCircuit` (same value),
/// - Incorrect validator address in pubdata,
/// - Incorrect block number in pubdata.
///
/// All these checks are implemented within one test to reduce the overhead of the
/// circuit initialization.
#[test]
#[ignore]
fn incorrect_circuit_pubdata() {
    // ----------
    // Test setup
    // ----------

    // Cryptographic utilities initialization.
    let jubjub_params = &AltJubjubBn256::new();
    let rescue_params = &Bn256RescueParams::new_checked_2_into_1();
    let p_g = FixedGenerators::SpendingKeyGenerator;
    let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
    let phasher = RescueHasher::<Bn256>::default();

    // Account tree, which we'll manually fill
    let mut tree: CircuitAccountTree = CircuitAccountTree::new(params::account_tree_depth());

    // We'll create one block with number 1
    let block_number = Fr::from_str("1").unwrap();

    // Validator account credentials
    let (validator_address_number, validator_address, validator_balances) =
        insert_validator(&mut tree, rng, p_g, &jubjub_params, &phasher);

    // Insert sender into a tree.
    insert_sender(&mut tree, rng, p_g, &jubjub_params, &phasher);

    // Perform the `noop` operation and collect the data required for circuit instance creation.
    let operation = noop_operation(&tree, validator_address_number);
    let (_, validator_account_witness) = apply_fee(&mut tree, validator_address_number, 0, 0);
    let (validator_audit_path, _) = get_audits(&tree, validator_address_number, 0);

    let correct_hash = tree.root_hash();
    let incorrect_hash = Default::default();

    // ---------------------
    // Incorrect hash values
    // ---------------------

    // Test vector of the following values:
    // (pub data old root hash), (pub data new root hash),
    // (circuit old root hash), (expected error message)
    let test_vector = vec![
        (
            incorrect_hash,
            correct_hash,
            correct_hash,
            "external data hash equality",
        ),
        (
            correct_hash,
            incorrect_hash,
            correct_hash,
            "external data hash equality",
        ),
        (
            correct_hash,
            correct_hash,
            incorrect_hash,
            "old_root contains initial_used_subtree_root",
        ),
        (
            incorrect_hash,
            correct_hash,
            incorrect_hash,
            "old_root contains initial_used_subtree_root",
        ),
    ];

    for (pubdata_old_hash, pubdata_new_hash, circuit_old_hash, expected_msg) in test_vector {
        let public_data_commitment = public_data_commitment::<Bn256>(
            &[false; 64],
            Some(pubdata_old_hash),
            Some(pubdata_new_hash),
            Some(validator_address),
            Some(block_number),
        );

        let circuit_instance = ZkSyncCircuit {
            rescue_params,
            jubjub_params,
            old_root: Some(circuit_old_hash),
            initial_used_subtree_root: Some(get_used_subtree_root_hash(&tree)),
            operations: vec![operation.clone()],
            pub_data_commitment: Some(public_data_commitment),
            block_number: Some(block_number),
            validator_account: validator_account_witness.clone(),
            validator_address: Some(validator_address),
            validator_balances: validator_balances.clone(),
            validator_audit_path: validator_audit_path.clone(),
        };

        let error = check_circuit_non_panicking(circuit_instance)
            .expect_err("Hash check: Incorrect pubdata values should lead to an error");

        assert!(
            error.contains(expected_msg),
            "Hash check: Got error message '{}', but expected '{}'",
            error,
            expected_msg
        );
    }

    // ---------------------------
    // Incorrect validator address
    // ---------------------------

    let pub_data_commitment = public_data_commitment::<Bn256>(
        &[false; 64],
        Some(tree.root_hash()),
        Some(tree.root_hash()),
        Some(Default::default()),
        Some(block_number),
    );

    let circuit_instance = ZkSyncCircuit {
        rescue_params,
        jubjub_params,
        old_root: Some(tree.root_hash()),
        initial_used_subtree_root: Some(get_used_subtree_root_hash(&tree)),
        operations: vec![operation.clone()],
        pub_data_commitment: Some(pub_data_commitment),
        block_number: Some(block_number),
        validator_account: validator_account_witness.clone(),
        validator_address: Some(validator_address),
        validator_balances: validator_balances.clone(),
        validator_audit_path: validator_audit_path.clone(),
    };

    // Validator address is a part of pubdata, which is used to calculate the new root hash,
    // so the hash value will not match expected one.
    // For details see `circuit.rs`.
    let expected_msg = "enforce external data hash equality";

    let error = check_circuit_non_panicking(circuit_instance)
        .expect_err("Validator address: Incorrect pubdata values should lead to an error");

    assert!(
        error.contains(expected_msg),
        "Validator address: Got error message '{}', but expected '{}'",
        error,
        expected_msg
    );

    // ----------------------
    // Incorrect block number
    // ----------------------

    let incorrect_block_number = Fr::from_str("2").unwrap();
    let pub_data_commitment = public_data_commitment::<Bn256>(
        &[false; 64],
        Some(tree.root_hash()),
        Some(tree.root_hash()),
        Some(validator_address),
        Some(incorrect_block_number),
    );

    let circuit_instance = ZkSyncCircuit {
        rescue_params,
        jubjub_params,
        old_root: Some(tree.root_hash()),
        initial_used_subtree_root: Some(get_used_subtree_root_hash(&tree)),
        operations: vec![operation],
        pub_data_commitment: Some(pub_data_commitment),
        block_number: Some(block_number),
        validator_account: validator_account_witness,
        validator_address: Some(validator_address),
        validator_balances,
        validator_audit_path,
    };

    // Block number is a part of pubdata, which is used to calculate the new root hash,
    // so the hash value will not match expected one.
    // For details see `circuit.rs`.
    let expected_msg = "enforce external data hash equality";

    let error = check_circuit_non_panicking(circuit_instance)
        .expect_err("Validator address: Incorrect pubdata values should lead to an error");

    assert!(
        error.contains(expected_msg),
        "Validator address: Got error message '{}', but expected '{}'",
        error,
        expected_msg
    );
}
