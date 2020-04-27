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
        tests::test_utils::check_circuit,
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

/// Low-level test for circuit based on a non-operation execution.
/// In this test we manually create an account tree, add a validator account with non-zero balance
/// in tokens, and apply a `noop` operation to this account.
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
    let validator_address_number = 7;
    let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
    let validator_pub_key_hash = generate_keys(rng, p_g, &jubjub_params, &phasher);

    // Sender account credentials.
    let sender_address: u32 = rng.gen::<u32>() % tree.capacity() as u32;
    let sender_balance_token_id: u32 = 2;
    let sender_balance_value: u128 = 2000;
    let sender_balance = Fr::from_str(&sender_balance_value.to_string()).unwrap();
    let sender_pub_key_hash = generate_keys(rng, p_g, &jubjub_params, &phasher);

    // Give some funds to sender and make zero balance for recipient (validator account)

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
    tree.insert(validator_address_number, validator_leaf);
    tree.insert(sender_address, sender_leaf_initial);

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
        new_root: Some(tree.root_hash()),
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
