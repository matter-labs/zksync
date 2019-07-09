
#[test]
fn test_transfer() {
    use super::*;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklinmodels::params as franklin_constants;

    use crate::account::*;
    use crate::circuit::FranklinCircuit;
    use crate::operation::*;
    use crate::utils::*;
    use bellman::{Circuit, ConstraintSystem, SynthesisError};
    use crypto::digest::Digest;
    use crypto::sha2::Sha256;
    use ff::Field;
    use ff::{BitIterator, PrimeField, PrimeFieldRepr};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
    use franklin_crypto::circuit::float_point::convert_to_float;
    use franklin_crypto::circuit::test::*;
    use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};
    use franklinmodels::circuit::account::{Balance, CircuitAccount};
    use franklinmodels::{CircuitAccountTree, CircuitBalanceTree};
    use merkle_tree::hasher::Hasher;
    use merkle_tree::PedersenHasher;
    use pairing::bn256::*;
    use rand::{Rng, SeedableRng, XorShiftRng};

    let params = &AltJubjubBn256::new();
    let p_g = FixedGenerators::SpendingKeyGenerator;

    let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
    let mut from_balance_tree =
        CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32);
    let from_balance_root = from_balance_tree.root_hash();

    let mut to_balance_tree =
        CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32);

    let validator_address = Fr::from_str("7").unwrap();
    let phasher = PedersenHasher::<Bn256>::default();
    let default_subtree_hash = from_balance_root;
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

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::ACCOUNT_TREE_DEPTH);

    let from_sk = PrivateKey::<Bn256>(rng.gen());
    let from_pk = PublicKey::from_private(&from_sk, p_g, params);
    let (from_x, from_y) = from_pk.0.into_xy();
    println!("x = {}, y = {}", from_x, from_y);

    let to_sk = PrivateKey::<Bn256>(rng.gen());
    let to_pk = PublicKey::from_private(&to_sk, p_g, params);
    let (to_x, to_y) = to_pk.0.into_xy();
    println!("x = {}, y = {}", to_x, to_y);

    // give some funds to sender and make zero balance for recipient

    // let sender_leaf_number = 1;

    let mut from_leaf_number: u32 = rng.gen();
    from_leaf_number %= capacity;
    let from_leaf_number_fe = Fr::from_str(&from_leaf_number.to_string()).unwrap();

    let mut to_leaf_number: u32 = rng.gen();
    to_leaf_number %= capacity;
    let to_leaf_number_fe = Fr::from_str(&to_leaf_number.to_string()).unwrap();

    let from_balance_before: u128 = 2000;

    let from_balance_before_as_field_element =
        Fr::from_str(&from_balance_before.to_string()).unwrap();

    let to_balance_before: u128 = 2100;

    let to_balance_before_as_field_element = Fr::from_str(&to_balance_before.to_string()).unwrap();

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

    from_balance_tree.insert(
        token,
        Balance {
            value: from_balance_before_as_field_element,
        },
    );

    let from_base_balance_root = from_balance_tree.root_hash();

    let from_leaf_before = CircuitAccount::<Bn256> {
        subtree_root_hash: from_base_balance_root.clone(),
        nonce: Fr::zero(),
        pub_x: from_x.clone(),
        pub_y: from_y.clone(),
    };

    to_balance_tree.insert(
        token,
        Balance {
            value: to_balance_before_as_field_element,
        },
    );
    let to_base_balance_root = to_balance_tree.root_hash();
    let to_leaf_before = CircuitAccount::<Bn256> {
        subtree_root_hash: to_base_balance_root.clone(),
        nonce: Fr::zero(),
        pub_x: to_x.clone(),
        pub_y: to_y.clone(),
    };
    tree.insert(from_leaf_number, from_leaf_before.clone());
    tree.insert(to_leaf_number, to_leaf_before.clone());
    println!(
        "hash from leaf {}",
        tree.get_hash((
            franklin_constants::ACCOUNT_TREE_DEPTH as u32,
            from_leaf_number
        ))
    );

    let from_audit_path_before: Vec<Option<Fr>> = tree
        .merkle_path(from_leaf_number)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let to_audit_path_before: Vec<Option<Fr>> = tree
        .merkle_path(to_leaf_number)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let from_audit_balance_path_before: Vec<Option<Fr>> = from_balance_tree
        .merkle_path(token)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let to_audit_balance_path_before: Vec<Option<Fr>> = to_balance_tree
        .merkle_path(token)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let initial_root = tree.root_hash();
    println!("Initial root = {}", initial_root);

    let mut from_balance_after = from_balance_before_as_field_element.clone();
    from_balance_after.sub_assign(&transfer_amount_as_field_element);

    from_balance_tree.insert(
        token,
        Balance {
            value: from_balance_after,
        },
    );

    let mut from_nonce_after_transfer = from_leaf_before.nonce.clone();
    from_nonce_after_transfer.add_assign(&Fr::from_str("1").unwrap());

    let from_leaf_after = CircuitAccount::<Bn256> {
        subtree_root_hash: from_balance_tree.root_hash(),
        nonce: from_nonce_after_transfer,
        pub_x: from_x.clone(),
        pub_y: from_y.clone(),
    };
    tree.insert(from_leaf_number, from_leaf_after.clone());
    let intermediate_root = tree.root_hash();

    let mut to_balance_after = to_balance_before_as_field_element.clone();
    to_balance_after.add_assign(&transfer_amount_as_field_element);

    to_balance_tree.insert(
        token,
        Balance {
            value: to_balance_after,
        },
    );

    let to_nonce_after_transfer = to_leaf_before.nonce.clone();

    let to_leaf_after = CircuitAccount::<Bn256> {
        subtree_root_hash: to_balance_tree.root_hash(),
        nonce: to_nonce_after_transfer,
        pub_x: to_x.clone(),
        pub_y: to_y.clone(),
    };
    tree.insert(to_leaf_number, to_leaf_after.clone());
    let final_root = tree.root_hash();

    // construct signature
    let mut sig_bits = vec![];

    let transfer_tx_type = Fr::from_str("5").unwrap();
    append_le_fixed_width(
        &mut sig_bits,
        &transfer_tx_type,
        *franklin_constants::TX_TYPE_BIT_WIDTH,
    );
    append_le_fixed_width(
        &mut sig_bits,
        &from_leaf_number_fe,
        franklin_constants::ACCOUNT_TREE_DEPTH,
    );
    append_le_fixed_width(
        &mut sig_bits,
        &token_fe,
        *franklin_constants::BALANCE_TREE_DEPTH,
    );
    append_le_fixed_width(
        &mut sig_bits,
        &from_leaf_before.nonce,
        franklin_constants::NONCE_BIT_WIDTH,
    );
    append_le_fixed_width(
        &mut sig_bits,
        &transfer_amount_encoded,
        franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
            + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
    );
    append_le_fixed_width(
        &mut sig_bits,
        &fee_encoded,
        franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
    );
    let sig_msg = le_bit_vector_into_field_element::<Fr>(&sig_bits);
    let sig_msg_hash = phasher.hash_bits(sig_bits.clone());
    let mut sig_msg_hash_bits = vec![];
    append_le_fixed_width(
        &mut sig_msg_hash_bits,
        &sig_msg_hash,
        franklin_constants::FR_BIT_WIDTH - 8,
    ); //TODO: not clear what capacity is

    println!(
        "test sig_msg_hash={} sig_msg_hash_bits.len={}",
        sig_msg_hash,
        sig_msg_hash_bits.len()
    );

    // construct pubdata
    let mut pubdata_bits = vec![];
    append_be_fixed_width(
        &mut pubdata_bits,
        &transfer_tx_type,
        *franklin_constants::TX_TYPE_BIT_WIDTH,
    );

    append_be_fixed_width(
        &mut pubdata_bits,
        &from_leaf_number_fe,
        franklin_constants::ACCOUNT_TREE_DEPTH,
    );
    append_be_fixed_width(
        &mut pubdata_bits,
        &token_fe,
        *franklin_constants::TOKEN_EXT_BIT_WIDTH,
    );
    append_be_fixed_width(
        &mut pubdata_bits,
        &to_leaf_number_fe,
        franklin_constants::ACCOUNT_TREE_DEPTH,
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
    assert_eq!(pubdata_bits.len(), 13 * 8);
    pubdata_bits.resize(16 * 8, false); //TODO verify if right padding is okay

    let pub_first_chunk_bits = pubdata_bits[0..franklin_constants::CHUNK_BIT_WIDTH].to_vec();
    let pub_first_chunk = le_bit_vector_into_field_element::<Fr>(&pub_first_chunk_bits);
    println!("pub_first_chunk {}", pub_first_chunk);
    let pub_second_chunk_bits = pubdata_bits
        [franklin_constants::CHUNK_BIT_WIDTH..2 * franklin_constants::CHUNK_BIT_WIDTH]
        .to_vec();
    let pub_second_chunk = le_bit_vector_into_field_element::<Fr>(&pub_second_chunk_bits);
    println!("pub_second_chunk {}", pub_second_chunk);
    // println!(" capacity {}",<Bn256 as JubjubEngine>::Fs::Capacity);

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
    let new_root_bits: Vec<bool> = BitIterator::new(final_root.into_repr()).collect();
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

    let signature = sign(&sig_bits, &from_sk, p_g, params, rng);

    //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));

    let _from_audit_path_after: Vec<Option<Fr>> = tree
        .merkle_path(from_leaf_number)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let to_audit_path_after: Vec<Option<Fr>> = tree
        .merkle_path(to_leaf_number)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let from_audit_balance_path_after: Vec<Option<Fr>> = from_balance_tree
        .merkle_path(token)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let _to_audit_balance_path_after: Vec<Option<Fr>> = to_balance_tree
        .merkle_path(token)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let mut sum_amount_fee = transfer_amount_as_field_element.clone();
    sum_amount_fee.add_assign(&fee_as_field_element);

    let op_args = OperationArguments::<Bn256> {
        a: Some(from_balance_before_as_field_element),
        b: Some(sum_amount_fee.clone()),
        amount: Some(transfer_amount_encoded.clone()),
        fee: Some(fee_encoded.clone()),
        new_pub_x: Some(from_x.clone()),
        new_pub_y: Some(from_y.clone()),
    };

    let from_operation_branch_before = OperationBranch::<Bn256> {
        address: Some(from_leaf_number_fe),
        token: Some(token_fe),
        witness: OperationBranchWitness {
            account_witness: AccountWitness {
                nonce: Some(from_leaf_before.nonce),
                pub_x: Some(from_leaf_before.pub_x),
                pub_y: Some(from_leaf_before.pub_y),
            },
            account_path: from_audit_path_before.clone(),
            balance_value: Some(from_balance_before_as_field_element.clone()),
            balance_subtree_path: from_audit_balance_path_before.clone(),
        },
    };

    let from_operation_branch_after = OperationBranch::<Bn256> {
        address: Some(from_leaf_number_fe),
        token: Some(token_fe),
        witness: OperationBranchWitness {
            account_witness: AccountWitness {
                nonce: Some(from_leaf_after.nonce),
                pub_x: Some(from_leaf_after.pub_x),
                pub_y: Some(from_leaf_after.pub_y),
            },
            account_path: from_audit_path_before.clone(),
            balance_value: Some(from_balance_after.clone()),
            balance_subtree_path: from_audit_balance_path_after.clone(),
        },
    };

    let to_operation_branch_before = OperationBranch::<Bn256> {
        address: Some(to_leaf_number_fe),
        token: Some(token_fe),
        witness: OperationBranchWitness {
            account_witness: AccountWitness {
                nonce: Some(to_leaf_before.nonce),
                pub_x: Some(to_leaf_before.pub_x),
                pub_y: Some(to_leaf_before.pub_y),
            },
            account_path: to_audit_path_before.clone(),
            balance_value: Some(to_balance_before_as_field_element.clone()),
            balance_subtree_path: to_audit_balance_path_before.clone(),
        },
    };

    let to_operation_branch_after = OperationBranch::<Bn256> {
        address: Some(to_leaf_number_fe),
        token: Some(token_fe),
        witness: OperationBranchWitness {
            account_witness: AccountWitness {
                nonce: Some(to_leaf_before.nonce),
                pub_x: Some(to_leaf_before.pub_x),
                pub_y: Some(to_leaf_before.pub_y),
            },
            account_path: to_audit_path_after.clone(),
            balance_value: Some(to_balance_before_as_field_element.clone()),
            balance_subtree_path: to_audit_balance_path_before.clone(),
        },
    };

    let operation_zero = Operation {
        new_root: Some(intermediate_root.clone()),
        tx_type: Some(Fr::from_str("5").unwrap()),
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pub_first_chunk),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(from_x.clone()),
        signer_pub_key_y: Some(from_y.clone()),
        args: op_args.clone(),
        lhs: from_operation_branch_before.clone(),
        rhs: to_operation_branch_before.clone(),
    };

    let operation_one = Operation {
        new_root: Some(final_root.clone()),
        tx_type: Some(Fr::from_str("5").unwrap()),
        chunk: Some(Fr::from_str("1").unwrap()),
        pubdata_chunk: Some(pub_second_chunk),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(from_x.clone()),
        signer_pub_key_y: Some(from_y.clone()),
        args: op_args.clone(),
        lhs: from_operation_branch_after.clone(),
        rhs: to_operation_branch_after.clone(),
    };

    {
        let mut cs = TestConstraintSystem::<Bn256>::new();

        let instance = FranklinCircuit {
            params,
            old_root: Some(initial_root),
            new_root: Some(final_root),
            operations: vec![operation_zero, operation_one],
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
