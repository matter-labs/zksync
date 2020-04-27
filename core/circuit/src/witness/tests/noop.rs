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
use crate::franklin_crypto::{
    alt_babyjubjub::AltJubjubBn256,
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::{Field, PrimeField},
    },
    bellman::Circuit,
    circuit::test::TestConstraintSystem,
    eddsa::{PrivateKey, PublicKey},
    group_hash::BlakeHasher,
    jubjub::FixedGenerators,
    rescue::bn256::Bn256RescueParams,
};
use crate::{
    circuit::FranklinCircuit,
    rand::{Rng, SeedableRng, XorShiftRng},
    witness::{
        noop::noop_operation,
        utils::{apply_fee, get_audits, public_data_commitment},
    },
};

#[test]
#[ignore]
fn test_noop() {
    let jubjub_params = &AltJubjubBn256::new();
    let rescue_params = &Bn256RescueParams::new_2_into_1::<BlakeHasher>();
    let p_g = FixedGenerators::SpendingKeyGenerator;
    let validator_address_number = 7;
    let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
    let block_number = Fr::from_str("1").unwrap();
    let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
    let phasher = RescueHasher::<Bn256>::default();

    let mut tree: CircuitAccountTree = CircuitAccountTree::new(params::account_tree_depth());

    let sender_sk = PrivateKey::<Bn256>(rng.gen());
    let sender_pk = PublicKey::from_private(&sender_sk, p_g, jubjub_params);
    let sender_pub_key_hash = pub_key_hash_fe(&sender_pk, &phasher);
    let (sender_x, sender_y) = sender_pk.0.into_xy();
    println!("x = {}, y = {}", sender_x, sender_y);

    // give some funds to sender and make zero balance for recipient
    let validator_sk = PrivateKey::<Bn256>(rng.gen());
    let validator_pk = PublicKey::from_private(&validator_sk, p_g, jubjub_params);
    let validator_pub_key_hash = pub_key_hash_fe(&validator_pk, &phasher);
    let (validator_x, validator_y) = validator_pk.0.into_xy();
    println!("x = {}, y = {}", validator_x, validator_y);
    let validator_leaf = CircuitAccount::<Bn256> {
        subtree: CircuitBalanceTree::new(params::balance_tree_depth()),
        nonce: Fr::zero(),
        pub_key_hash: validator_pub_key_hash,
        address: Fr::zero(),
    };

    let mut validator_balances = vec![];
    for _ in 0..params::total_tokens() {
        validator_balances.push(Some(Fr::zero()));
    }
    tree.insert(validator_address_number, validator_leaf);

    let mut account_address: u32 = rng.gen();
    account_address %= tree.capacity() as u32;
    let token: u32 = 2;

    let sender_balance_before: u128 = 2000;

    let sender_balance_before_as_field_element =
        Fr::from_str(&sender_balance_before.to_string()).unwrap();

    let mut sender_balance_tree = CircuitBalanceTree::new(params::balance_tree_depth());
    sender_balance_tree.insert(
        token,
        Balance {
            value: sender_balance_before_as_field_element,
        },
    );

    let sender_leaf_initial = CircuitAccount::<Bn256> {
        subtree: sender_balance_tree,
        nonce: Fr::zero(),
        pub_key_hash: sender_pub_key_hash,
        address: Fr::zero(),
    };

    tree.insert(account_address, sender_leaf_initial);

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
    {
        let mut cs = TestConstraintSystem::<Bn256>::new();

        let instance = FranklinCircuit {
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

        instance.synthesize(&mut cs).unwrap();

        println!("{}", cs.find_unconstrained());

        println!("{}", cs.num_constraints());

        if let Some(err) = cs.which_is_unsatisfied() {
            panic!("ERROR satisfying in {}", err);
        }
    }
}
