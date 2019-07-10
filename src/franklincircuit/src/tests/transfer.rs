use super::utils::*;
use crate::account::*;
use crate::circuit::FranklinCircuit;
use crate::operation::*;
use crate::utils::*;
use bellman::Circuit;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use ff::{BitIterator, Field, PrimeField, PrimeFieldRepr};
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use franklin_crypto::circuit::float_point::convert_to_float;
use franklin_crypto::circuit::test::*;
use franklin_crypto::eddsa::{PrivateKey, PublicKey};
use franklin_crypto::jubjub::FixedGenerators;
use franklin_crypto::jubjub::JubjubEngine;
use franklinmodels::circuit::account::{
    Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree,
};
use franklinmodels::params as franklin_constants;
use merkle_tree::hasher::Hasher;
use merkle_tree::PedersenHasher;
use pairing::bn256::*;
use rand::{Rng, SeedableRng, XorShiftRng};

pub struct TransferData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub from_account_address: u32,
    pub to_account_address: u32,
}
pub struct TransferWitness<E: JubjubEngine> {
    pub from_before: OperationBranch<E>,
    pub from_intermediate: OperationBranch<E>,
    pub from_after: OperationBranch<E>,
    pub to_before: OperationBranch<E>,
    pub to_intermediate: OperationBranch<E>,
    pub to_after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub intermediate_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}
impl<E: JubjubEngine> TransferWitness<E> {
    pub fn get_pubdata(&self) -> Vec<bool> {
        // construct pubdata
        let mut pubdata_bits = vec![];
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.tx_type.unwrap(),
            *franklin_constants::TX_TYPE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.from_before.address.unwrap(),
            franklin_constants::ACCOUNT_TREE_DEPTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.from_before.token.unwrap(),
            *franklin_constants::TOKEN_EXT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.to_before.address.unwrap(),
            franklin_constants::ACCOUNT_TREE_DEPTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.amount.unwrap(),
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );
        assert_eq!(pubdata_bits.len(), 13 * 8);
        pubdata_bits.resize(16 * 8, false); //TODO verify if right padding is okay
        pubdata_bits
    }
}

pub fn apply_transfer(
    tree: &mut CircuitAccountTree,
    transfer: &TransferData,
) -> TransferWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    println!("Initial root = {}", before_root);
    let (audit_path_from_before, audit_balance_path_from_before) =
        get_audits(tree, transfer.from_account_address, transfer.token);

    let (audit_path_to_before, audit_balance_path_to_before) =
        get_audits(tree, transfer.to_account_address, transfer.token);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::ACCOUNT_TREE_DEPTH);
    let account_address_from_fe = Fr::from_str(&transfer.from_account_address.to_string()).unwrap();
    let account_address_to_fe = Fr::from_str(&transfer.to_account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&transfer.token.to_string()).unwrap();
    let amount_as_field_element = Fr::from_str(&transfer.amount.to_string()).unwrap();

    let amount_bits = convert_to_float(
        transfer.amount,
        *franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        *franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

    let fee_as_field_element = Fr::from_str(&transfer.fee.to_string()).unwrap();

    let fee_bits = convert_to_float(
        transfer.fee,
        *franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        *franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

    //applying first transfer part
    let (
        account_witness_from_before,
        account_witness_from_intermediate,
        balance_from_before,
        balance_from_intermediate,
    ) = apply_leaf_operation(
        tree,
        transfer.from_account_address,
        transfer.token,
        |acc| {
            acc.nonce.add_assign(&Fr::from_str("1").unwrap());
        },
        |bal| {
            bal.value.sub_assign(&amount_as_field_element);
            bal.value.sub_assign(&fee_as_field_element)
        },
    );

    let intermediate_root = tree.root_hash();
    println!("Intermediate root = {}", intermediate_root);

    let (audit_path_from_intermediate, audit_balance_path_from_intermediate) =
        get_audits(tree, transfer.from_account_address, transfer.token);

    let (audit_path_to_intermediate, audit_balance_path_to_intermediate) =
        get_audits(tree, transfer.to_account_address, transfer.token);

    let (
        account_witness_to_intermediate,
        account_witness_to_after,
        balance_to_intermediate,
        balance_to_after,
    ) = apply_leaf_operation(
        tree,
        transfer.from_account_address,
        transfer.token,
        |_| {},
        |bal| bal.value.add_assign(&amount_as_field_element),
    );
    let after_root = tree.root_hash();
    let (audit_path_from_after, audit_balance_path_from_after) =
        get_audits(tree, transfer.from_account_address, transfer.token);

    let (audit_path_to_after, audit_balance_path_to_after) =
        get_audits(tree, transfer.to_account_address, transfer.token);

    //calculate a and b
    let a = balance_from_before.clone();
    let mut b = amount_as_field_element.clone();
    b.add_assign(&fee_as_field_element);

    TransferWitness {
        from_before: OperationBranch {
            address: Some(account_address_from_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_from_before,
                account_path: audit_path_from_before,
                balance_value: Some(balance_from_before),
                balance_subtree_path: audit_balance_path_from_before,
            },
        },
        from_intermediate: OperationBranch {
            address: Some(account_address_from_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_from_intermediate.clone(),
                account_path: audit_path_from_intermediate,
                balance_value: Some(balance_from_intermediate),
                balance_subtree_path: audit_balance_path_from_intermediate,
            },
        },
        from_after: OperationBranch {
            address: Some(account_address_from_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_from_intermediate,
                account_path: audit_path_from_after,
                balance_value: Some(balance_from_intermediate),
                balance_subtree_path: audit_balance_path_from_after,
            },
        },
        to_before: OperationBranch {
            address: Some(account_address_to_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_to_intermediate.clone(),
                account_path: audit_path_to_before,
                balance_value: Some(balance_to_intermediate),
                balance_subtree_path: audit_balance_path_to_before,
            },
        },
        to_intermediate: OperationBranch {
            address: Some(account_address_to_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_to_intermediate,
                account_path: audit_path_to_intermediate,
                balance_value: Some(balance_to_intermediate),
                balance_subtree_path: audit_balance_path_to_intermediate,
            },
        },
        to_after: OperationBranch {
            address: Some(account_address_to_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_to_after,
                account_path: audit_path_to_after,
                balance_value: Some(balance_to_after),
                balance_subtree_path: audit_balance_path_to_after,
            },
        },
        args: OperationArguments {
            amount: Some(amount_encoded),
            fee: Some(fee_encoded),
            a: Some(a),
            b: Some(b),
            new_pub_x: Some(Fr::zero()),
            new_pub_y: Some(Fr::zero()), //shouldn't matter for transfer operation
        },
        before_root: Some(before_root),
        intermediate_root: Some(intermediate_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("5").unwrap()),
    }
}
#[test]
fn test_transfer() {
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklinmodels::params as franklin_constants;

    use crate::account::*;
    use crate::circuit::FranklinCircuit;
    use crate::operation::*;
    use crate::utils::*;
    use bellman::Circuit;
    use crypto::digest::Digest;
    use crypto::sha2::Sha256;
    use ff::Field;
    use ff::{BitIterator, PrimeField, PrimeFieldRepr};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
    use franklin_crypto::circuit::float_point::convert_to_float;
    use franklin_crypto::circuit::test::*;
    use franklin_crypto::jubjub::FixedGenerators;
    use franklinmodels::circuit::account::{
        Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree,
    };
    use merkle_tree::hasher::Hasher;
    use merkle_tree::PedersenHasher;
    use pairing::bn256::*;
    use rand::{Rng, SeedableRng, XorShiftRng};

    let params = &AltJubjubBn256::new();
    let p_g = FixedGenerators::SpendingKeyGenerator;

    let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);

    let validator_address = Fr::from_str("7").unwrap();
    let phasher = PedersenHasher::<Bn256>::default();

    let mut tree = CircuitAccountTree::new(franklin_constants::ACCOUNT_TREE_DEPTH as u32);

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
    let block_number = Fr::from_str("1").unwrap();
    // prepare state, so that we could make transfer
    let mut from_balance_tree =
        CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32);
    let mut to_balance_tree =
        CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32);
    from_balance_tree.insert(
        token,
        Balance {
            value: from_balance_before_as_field_element,
        },
    );

    let from_leaf_initial = CircuitAccount::<Bn256> {
        subtree: from_balance_tree,
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
    let to_leaf_initial = CircuitAccount::<Bn256> {
        subtree: to_balance_tree,
        nonce: Fr::zero(),
        pub_x: to_x.clone(),
        pub_y: to_y.clone(),
    };
    tree.insert(from_leaf_number, from_leaf_initial);
    tree.insert(to_leaf_number, to_leaf_initial);

    let transfer_witness = apply_transfer(
        &mut tree,
        &TransferData {
            amount: transfer_amount,
            fee: fee,
            token: token,
            from_account_address: from_leaf_number,
            to_account_address: to_leaf_number,
        },
    );
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
        &transfer_witness
            .from_after
            .witness
            .account_witness
            .nonce
            .unwrap(),
        // &transfer_witness.nonce,
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
    let public_data_commitment = public_data_commitment::<Bn256>(
        &transfer_witness.get_pubdata(),
        transfer_witness.before_root,
        transfer_witness.after_root,
        Some(validator_address),
        Some(block_number),
    );
    let pubdata_chunks: Vec<_> = transfer_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();
    let signature = sign(&sig_bits, &from_sk, p_g, params, rng);

    let operation_zero = Operation {
        new_root: transfer_witness.intermediate_root,
        tx_type: transfer_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(from_x.clone()),
        signer_pub_key_y: Some(from_y.clone()),
        args: transfer_witness.args.clone(),
        lhs: transfer_witness.from_before,
        rhs: transfer_witness.to_before,
    };

    let operation_one = Operation {
        new_root: transfer_witness.after_root,
        tx_type: transfer_witness.tx_type,
        chunk: Some(Fr::from_str("1").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[1]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(from_x.clone()),
        signer_pub_key_y: Some(from_y.clone()),
        args: transfer_witness.args,
        lhs: transfer_witness.from_intermediate,
        rhs: transfer_witness.to_intermediate,
    };

    {
        let mut cs = TestConstraintSystem::<Bn256>::new();

        let instance = FranklinCircuit {
            params,
            old_root: transfer_witness.before_root,
            new_root: transfer_witness.after_root,
            operations: vec![operation_zero, operation_one],
            pub_data_commitment: Some(public_data_commitment),
            block_number: Some(block_number),
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
