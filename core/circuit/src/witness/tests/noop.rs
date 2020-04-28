// External deps
use crypto_exports::franklin_crypto::{
    alt_babyjubjub::AltJubjubBn256,
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::{Field, PrimeField},
    },
    eddsa::{PrivateKey, PublicKey},
    group_hash::BlakeHasher,
    jubjub::FixedGenerators,
    rescue::bn256::Bn256RescueParams,
};
use crypto_exports::rand::{Rng, SeedableRng, XorShiftRng};
// Workspace deps
use models::{
    circuit::{
        account::{Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree},
        utils::pub_key_hash_fe,
    },
    merkle_tree::RescueHasher,
    params,
};

// Local deps
use crate::{
    circuit::FranklinCircuit,
    witness::{
        noop::noop_operation,
        tests::test_utils::{check_circuit, check_circuit_non_panicking},
        utils::{apply_fee, get_audits, public_data_commitment},
    },
};

fn generate_keys(
    rng: &mut XorShiftRng,
    p_g: FixedGenerators,
    jubjub_params: &AltJubjubBn256,
    phasher: &RescueHasher<Bn256>,
) -> Fr {
    let sk = PrivateKey::<Bn256>(rng.gen());
    let pk = PublicKey::from_private(&sk, p_g, jubjub_params);
    let pub_key_hash = pub_key_hash_fe(&pk, phasher);

    pub_key_hash
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
    let validator_pub_key_hash = generate_keys(rng, p_g, &jubjub_params, &phasher);

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
    let sender_address: u32 = rng.gen::<u32>() % tree.capacity() as u32;
    let sender_balance_token_id: u32 = 2;
    let sender_balance_value: u128 = 2000;
    let sender_balance = Fr::from_str(&sender_balance_value.to_string()).unwrap();
    let sender_pub_key_hash = generate_keys(rng, p_g, &jubjub_params, &phasher);

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
    // Cryptographic utilities initialization.
    let jubjub_params = &AltJubjubBn256::new();
    let rescue_params = &Bn256RescueParams::new_2_into_1::<BlakeHasher>();
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

    let public_data_commitment = public_data_commitment::<Bn256>(
        &[false; 64],
        Some(tree.root_hash()),
        Some(tree.root_hash()),
        Some(validator_address),
        Some(block_number),
    );

    // Parametrize the circuit instance.
    let circuit_instance = FranklinCircuit {
        operation_batch_size: 1,
        rescue_params,
        jubjub_params,
        old_root: Some(tree.root_hash()),
        operations: vec![operation],
        pub_data_commitment: Some(public_data_commitment),
        block_number: Some(block_number),
        validator_account: validator_account_witness,
        validator_address: Some(validator_address),
        validator_balances,
        validator_audit_path,
    };

    // Check that there are no unsatisfied constraints.
    check_circuit(circuit_instance);
}

/// Test for the root hash being set to the incorrect value.
///
/// The following checks are performed:
/// - Incorrect old root hash in `pub_data_commitment`,
/// - Incorrect new root hash in `pub_data_commitment`,
/// - Incorrect old root hash in `FranklinCircuit`,
/// - Incorrect old root hash in both `pub_data_commitment` and `FranklinCircuit` (same value),
#[test]
#[ignore]
fn incorrect_root() {
    // Cryptographic utilities initialization.
    let jubjub_params = &AltJubjubBn256::new();
    let rescue_params = &Bn256RescueParams::new_2_into_1::<BlakeHasher>();
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
            "root state before applying operation is valid",
        ),
        (
            incorrect_hash,
            correct_hash,
            incorrect_hash,
            "root state before applying operation is valid",
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

        // Parametrize the circuit instance.
        let circuit_instance = FranklinCircuit {
            operation_batch_size: 1,
            rescue_params,
            jubjub_params,
            old_root: Some(circuit_old_hash),
            operations: vec![operation.clone()],
            pub_data_commitment: Some(public_data_commitment),
            block_number: Some(block_number),
            validator_account: validator_account_witness.clone(),
            validator_address: Some(validator_address),
            validator_balances: validator_balances.clone(),
            validator_audit_path: validator_audit_path.clone(),
        };

        let error = check_circuit_non_panicking(circuit_instance)
            .expect_err("Incorrect hash values should lead to error");

        assert!(
            error.contains(expected_msg),
            "Got error message '{}', but expected '{}'",
            error,
            expected_msg
        );
    }
}
