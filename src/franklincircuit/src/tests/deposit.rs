#[test]
fn test_deposit_franklin_in_empty_leaf() {
    use super::*;
    use crate::account::*;
    use crate::circuit::FranklinCircuit;
    use crate::operation::*;
    use crate::utils::*;
    use bellman::{Circuit, ConstraintSystem, SynthesisError};
    use crypto::digest::Digest;
    use crypto::sha2::Sha256;
    use ff::{BitIterator, Field, PrimeField, PrimeFieldRepr};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
    use franklin_crypto::circuit::float_point::convert_to_float;
    use franklin_crypto::circuit::test::*;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};
    use franklinmodels::circuit::account::{Balance, CircuitAccount};
    use franklinmodels::params as franklin_constants;
    use franklinmodels::{CircuitAccountTree, CircuitBalanceTree};
    use merkle_tree::hasher::Hasher;
    use merkle_tree::PedersenHasher;
    use pairing::bn256::*;
    use rand::{Rng, SeedableRng, XorShiftRng};

    let params = &AltJubjubBn256::new();
    let p_g = FixedGenerators::SpendingKeyGenerator;
    let validator_address = Fr::from_str("7").unwrap();

    let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
    let mut balance_tree = CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32);
    let balance_root = balance_tree.root_hash();
    // println!("test balance root: {}", balance_root);
    // println!("test subaccount root: {}", subaccount_root);
    let phasher = PedersenHasher::<Bn256>::default();
    let default_subtree_hash = balance_root;
    // println!("test subtree root: {}", default_subtree_hash);
    let zero_account = CircuitAccount {
        nonce: Fr::zero(),
        pub_x: Fr::zero(),
        pub_y: Fr::zero(),
        subtree_root_hash: default_subtree_hash,
    };
    let mut tree = CircuitAccountTree::new_with_leaf(
        franklin_constants::ACCOUNT_TREE_DEPTH as u32,
        zero_account,
    );
    let initial_root = tree.root_hash();
    println!("Initial root = {}", initial_root);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::ACCOUNT_TREE_DEPTH);

    let sender_sk = PrivateKey::<Bn256>(rng.gen());
    let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
    let (sender_x, sender_y) = sender_pk.0.into_xy();
    println!("x = {}, y = {}", sender_x, sender_y);

    // give some funds to sender and make zero balance for recipient

    // let sender_leaf_number = 1;

    let mut sender_leaf_number: u32 = rng.gen();
    sender_leaf_number %= capacity;
    let sender_leaf_number_fe = Fr::from_str(&sender_leaf_number.to_string()).unwrap();
    println!(
        "old leaf hash is {}",
        tree.get_hash((
            franklin_constants::ACCOUNT_TREE_DEPTH as u32,
            sender_leaf_number
        ))
    );
    let transfer_amount: u128 = 500;

    let transfer_amount_as_field_element = Fr::from_str(&transfer_amount.to_string()).unwrap();

    let transfer_amount_bits = convert_to_float(
        transfer_amount,
        *franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        *franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let transfer_amount_encoded: Fr = le_bit_vector_into_field_element(&transfer_amount_bits);

    let fee: u128 = 0;

    let fee_as_field_element = Fr::from_str(&fee.to_string()).unwrap();

    let fee_bits = convert_to_float(
        fee,
        *franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        *franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

    let token: u32 = 2;
    let token_fe = Fr::from_str(&token.to_string()).unwrap();

    balance_tree.insert(
        token,
        Balance {
            value: transfer_amount_as_field_element,
        },
    );
    let after_deposit_balance_root = balance_tree.root_hash();

    let after_deposit_subtree_hash = after_deposit_balance_root;

    let sender_leaf = CircuitAccount::<Bn256> {
        subtree_root_hash: after_deposit_subtree_hash.clone(),
        nonce: Fr::zero(),
        pub_x: sender_x.clone(),
        pub_y: sender_y.clone(),
    };

    tree.insert(sender_leaf_number, sender_leaf.clone());
    let new_root = tree.root_hash();

    println!("New root = {}", new_root);

    assert!(initial_root != new_root);
    println!(
        "updated leaf hash is {}",
        tree.get_hash((
            franklin_constants::ACCOUNT_TREE_DEPTH as u32,
            sender_leaf_number
        ))
    );

    let sig_msg = Fr::from_str("2").unwrap(); //dummy sig msg cause skipped on deposit proof
    let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
    sig_bits.reverse();
    sig_bits.truncate(80);

    // println!(" capacity {}",<Bn256 as JubjubEngine>::Fs::Capacity);
    let signature = sign(&sig_bits, &sender_sk, p_g, params, rng);
    //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));

    let deposit_tx_type = Fr::from_str("1").unwrap();
    let mut pubdata_bits = vec![];
    append_be_fixed_width(
        &mut pubdata_bits,
        &deposit_tx_type,
        *franklin_constants::TX_TYPE_BIT_WIDTH,
    );

    append_be_fixed_width(
        &mut pubdata_bits,
        &sender_leaf_number_fe,
        franklin_constants::ACCOUNT_TREE_DEPTH,
    );
    append_be_fixed_width(
        &mut pubdata_bits,
        &token_fe,
        *franklin_constants::TOKEN_EXT_BIT_WIDTH,
    );
    append_be_fixed_width(
        &mut pubdata_bits,
        &transfer_amount_encoded,
        franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
            + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
    );

    append_be_fixed_width(
        &mut pubdata_bits,
        &fee_encoded,
        franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
    );

    let mut new_pubkey_bits = vec![];
    append_be_fixed_width(
        &mut new_pubkey_bits,
        &sender_y,
        franklin_constants::FR_BIT_WIDTH - 1,
    );
    append_be_fixed_width(&mut new_pubkey_bits, &sender_x, 1);
    let new_pubkey_hash = phasher.hash_bits(new_pubkey_bits);

    append_be_fixed_width(
        &mut pubdata_bits,
        &new_pubkey_hash,
        *franklin_constants::NEW_PUBKEY_HASH_WIDTH,
    );
    assert_eq!(pubdata_bits.len(), 37 * 8);
    pubdata_bits.resize(40 * 8, false);

    let pubdata_chunks: Vec<Fr> = pubdata_bits
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect::<Vec<_>>();

    let audit_path: Vec<Option<Fr>> = tree
        .merkle_path(sender_leaf_number)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let audit_balance_path: Vec<Option<Fr>> = balance_tree
        .merkle_path(token)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let op_args = OperationArguments {
        a: Some(transfer_amount_as_field_element.clone()),
        b: Some(fee_as_field_element.clone()),
        amount: Some(transfer_amount_encoded.clone()),
        fee: Some(fee_encoded.clone()),
        new_pub_x: Some(sender_x.clone()),
        new_pub_y: Some(sender_y.clone()),
    };
    let operation_branch_before = OperationBranch {
        address: Some(sender_leaf_number_fe),
        token: Some(token_fe),
        witness: OperationBranchWitness {
            account_witness: AccountWitness {
                nonce: Some(Fr::zero()),
                pub_x: Some(Fr::zero()),
                pub_y: Some(Fr::zero()),
            },
            account_path: audit_path.clone(),
            balance_value: Some(Fr::zero()),
            balance_subtree_path: audit_balance_path.clone(),
        },
    };
    let operation_branch_after = OperationBranch::<Bn256> {
        address: Some(sender_leaf_number_fe),
        token: Some(token_fe),
        witness: OperationBranchWitness {
            account_witness: AccountWitness {
                nonce: Some(Fr::zero()),
                pub_x: Some(sender_x.clone()),
                pub_y: Some(sender_y.clone()),
            },
            account_path: audit_path.clone(),
            balance_value: Some(transfer_amount_as_field_element.clone()),
            balance_subtree_path: audit_balance_path.clone(),
        },
    };
    let operation_zero = Operation {
        new_root: Some(new_root.clone()),
        tx_type: Some(Fr::from_str("1").unwrap()),
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(sender_x.clone()),
        signer_pub_key_y: Some(sender_y.clone()),
        args: op_args.clone(),
        lhs: operation_branch_before.clone(),
        rhs: operation_branch_before.clone(),
    };

    let operation_one = Operation {
        new_root: Some(new_root.clone()),
        tx_type: Some(Fr::from_str("1").unwrap()),
        chunk: Some(Fr::from_str("1").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[1]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(sender_x.clone()),
        signer_pub_key_y: Some(sender_y.clone()),
        args: op_args.clone(),
        lhs: operation_branch_after.clone(),
        rhs: operation_branch_after.clone(),
    };

    let operation_two = Operation {
        new_root: Some(new_root.clone()),
        tx_type: Some(Fr::from_str("1").unwrap()),
        chunk: Some(Fr::from_str("2").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[2]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(sender_x.clone()),
        signer_pub_key_y: Some(sender_y.clone()),
        args: op_args.clone(),
        lhs: operation_branch_after.clone(),
        rhs: operation_branch_after.clone(),
    };

    let operation_three = Operation {
        new_root: Some(new_root.clone()),
        tx_type: Some(Fr::from_str("1").unwrap()),
        chunk: Some(Fr::from_str("3").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[3]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(sender_x.clone()),
        signer_pub_key_y: Some(sender_y.clone()),
        args: op_args.clone(),
        lhs: operation_branch_after.clone(),
        rhs: operation_branch_after.clone(),
    };
    let operation_four = Operation {
        new_root: Some(new_root.clone()),
        tx_type: Some(Fr::from_str("1").unwrap()),
        chunk: Some(Fr::from_str("4").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[4]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(sender_x.clone()),
        signer_pub_key_y: Some(sender_y.clone()),
        args: op_args.clone(),
        lhs: operation_branch_after.clone(),
        rhs: operation_branch_after.clone(),
    };
    let mut public_data_initial_bits = vec![];

    // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

    let block_number_bits: Vec<bool> = BitIterator::new(Fr::one().into_repr()).collect();
    for _ in 0..256 - block_number_bits.len() {
        public_data_initial_bits.push(false);
    }
    public_data_initial_bits.extend(block_number_bits.into_iter());

    let validator_id_bits: Vec<bool> = BitIterator::new(validator_address.into_repr()).collect();
    for _ in 0..256 - validator_id_bits.len() {
        public_data_initial_bits.push(false);
    }
    public_data_initial_bits.extend(validator_id_bits.into_iter());

    assert_eq!(public_data_initial_bits.len(), 512);

    let mut h = Sha256::new();

    let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

    h.input(&bytes_to_hash);

    let mut hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    println!("Initial hash hex {}", hex::encode(hash_result));

    let mut packed_old_root_bits = vec![];
    let old_root_bits: Vec<bool> = BitIterator::new(initial_root.into_repr()).collect();
    for _ in 0..256 - old_root_bits.len() {
        packed_old_root_bits.push(false);
    }
    packed_old_root_bits.extend(old_root_bits);

    let packed_old_root_bytes = be_bit_vector_into_bytes(&packed_old_root_bits);

    let mut packed_with_old_root = vec![];
    packed_with_old_root.extend(hash_result.iter());
    packed_with_old_root.extend(packed_old_root_bytes);

    h = Sha256::new();
    h.input(&packed_with_old_root);
    hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    let mut packed_new_root_bits = vec![];
    let new_root_bits: Vec<bool> = BitIterator::new(new_root.into_repr()).collect();
    for _ in 0..256 - new_root_bits.len() {
        packed_new_root_bits.push(false);
    }
    packed_new_root_bits.extend(new_root_bits);

    let packed_new_root_bytes = be_bit_vector_into_bytes(&packed_new_root_bits);

    let mut packed_with_new_root = vec![];
    packed_with_new_root.extend(hash_result.iter());
    packed_with_new_root.extend(packed_new_root_bytes);

    h = Sha256::new();
    h.input(&packed_with_new_root);
    hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    println!("hash with new root as hex {}", hex::encode(hash_result));

    let mut final_bytes = vec![];
    let pubdata_bytes = be_bit_vector_into_bytes(&pubdata_bits);
    final_bytes.extend(hash_result.iter());
    final_bytes.extend(pubdata_bytes);

    h = Sha256::new();
    h.input(&final_bytes);
    hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    println!("final hash as hex {}", hex::encode(hash_result));

    hash_result[0] &= 0x1f; // temporary solution, this nullifies top bits to be encoded into field element correctly

    let mut repr = Fr::zero().into_repr();
    repr.read_be(&hash_result[..])
        .expect("pack hash as field element");

    let public_data_commitment = Fr::from_repr(repr).unwrap();

    println!(
        "Final data commitment as field element = {}",
        public_data_commitment
    );
    {
        let mut cs = TestConstraintSystem::<Bn256>::new();

        let instance = FranklinCircuit {
            params,
            old_root: Some(initial_root),
            new_root: Some(new_root),
            operations: vec![
                operation_zero,
                operation_one,
                operation_two,
                operation_three,
                operation_four,
            ],
            pub_data_commitment: Some(public_data_commitment),
            block_number: Some(Fr::one()),
            validator_address: Some(validator_address),
        };

        instance.synthesize(&mut cs).unwrap();

        println!("{}", cs.find_unconstrained());

        println!("{}", cs.num_constraints());

        let err = cs.which_is_unsatisfied();
        if err.is_some() {
            panic!("ERROR satisfying in {}", err.unwrap());
        }
    }
}
