use super::utils::*;
use crate::franklin_crypto::bellman::pairing::bn256::*;
use crate::franklin_crypto::bellman::pairing::ff::{Field, PrimeField};
use crate::franklin_crypto::circuit::float_point::convert_to_float;
use crate::franklin_crypto::rescue::RescueEngine;
use crate::operation::SignatureData;
use crate::operation::*;
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{
    append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element,
};
use models::node::TransferToNewOp;
use models::params as franklin_constants;

pub struct TransferToNewData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub from_account_address: u32,
    pub to_account_address: u32,
    pub new_address: Fr,
}

pub struct TransferToNewWitness<E: RescueEngine> {
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

impl<E: RescueEngine> TransferToNewWitness<E> {
    pub fn get_pubdata(&self) -> Vec<bool> {
        let mut pubdata_bits = vec![];
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.tx_type.unwrap(),
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.from_before.address.unwrap(),
            franklin_constants::ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.from_before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.amount_packed.unwrap(),
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            franklin_constants::ETH_ADDRESS_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.to_before.address.unwrap(),
            franklin_constants::ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );
        pubdata_bits.resize(5 * franklin_constants::CHUNK_BIT_WIDTH, false);
        pubdata_bits
    }
    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("5").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self
                .from_before
                .witness
                .account_witness
                .pub_key_hash
                .unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.new_pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );

        append_be_fixed_width(
            &mut sig_bits,
            &self.from_before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.amount_packed.unwrap(),
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.fee.unwrap(),
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.from_before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}

pub fn apply_transfer_to_new_tx(
    tree: &mut CircuitAccountTree,
    transfer_to_new: &TransferToNewOp,
) -> TransferToNewWitness<Bn256> {
    let transfer_data = TransferToNewData {
        amount: transfer_to_new.tx.amount.to_string().parse().unwrap(),
        fee: transfer_to_new.tx.fee.to_string().parse().unwrap(),
        token: u32::from(transfer_to_new.tx.token),
        from_account_address: transfer_to_new.from,
        to_account_address: transfer_to_new.to,
        new_address: eth_address_to_fr(&transfer_to_new.tx.to),
    };
    // le_bit_vector_into_field_element()
    apply_transfer_to_new(tree, &transfer_data)
}
pub fn apply_transfer_to_new(
    tree: &mut CircuitAccountTree,
    transfer_to_new: &TransferToNewData,
) -> TransferToNewWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    debug!("Initial root = {}", before_root);
    let (audit_path_from_before, audit_balance_path_from_before) = get_audits(
        tree,
        transfer_to_new.from_account_address,
        transfer_to_new.token,
    );

    let (audit_path_to_before, audit_balance_path_to_before) = get_audits(
        tree,
        transfer_to_new.to_account_address,
        transfer_to_new.token,
    );

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_address_from_fe =
        Fr::from_str(&transfer_to_new.from_account_address.to_string()).unwrap();
    let account_address_to_fe =
        Fr::from_str(&transfer_to_new.to_account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&transfer_to_new.token.to_string()).unwrap();
    let amount_as_field_element = Fr::from_str(&transfer_to_new.amount.to_string()).unwrap();

    let amount_bits = convert_to_float(
        transfer_to_new.amount,
        franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

    debug!("test_transfer_to_new.fee {}", transfer_to_new.fee);
    let fee_as_field_element = Fr::from_str(&transfer_to_new.fee.to_string()).unwrap();
    debug!(
        "test transfer_to_new fee_as_field_element = {}",
        fee_as_field_element
    );
    let fee_bits = convert_to_float(
        transfer_to_new.fee,
        franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);
    debug!("fee_encoded in test_transfer_to_new {}", fee_encoded);
    //applying first transfer part
    let (
        account_witness_from_before,
        account_witness_from_intermediate,
        balance_from_before,
        balance_from_intermediate,
    ) = apply_leaf_operation(
        tree,
        transfer_to_new.from_account_address,
        transfer_to_new.token,
        |acc| {
            acc.nonce.add_assign(&Fr::from_str("1").unwrap());
        },
        |bal| {
            bal.value.sub_assign(&amount_as_field_element);
            bal.value.sub_assign(&fee_as_field_element)
        },
    );

    let intermediate_root = tree.root_hash();
    debug!("Intermediate root = {}", intermediate_root);

    let (audit_path_from_intermediate, audit_balance_path_from_intermediate) = get_audits(
        tree,
        transfer_to_new.from_account_address,
        transfer_to_new.token,
    );

    let (audit_path_to_intermediate, audit_balance_path_to_intermediate) = get_audits(
        tree,
        transfer_to_new.to_account_address,
        transfer_to_new.token,
    );

    let (
        account_witness_to_intermediate,
        account_witness_to_after,
        balance_to_intermediate,
        balance_to_after,
    ) = apply_leaf_operation(
        tree,
        transfer_to_new.to_account_address,
        transfer_to_new.token,
        |acc| {
            assert!((acc.address == Fr::zero()));
            acc.address = transfer_to_new.new_address;
        },
        |bal| bal.value.add_assign(&amount_as_field_element),
    );
    let after_root = tree.root_hash();
    let (audit_path_from_after, audit_balance_path_from_after) = get_audits(
        tree,
        transfer_to_new.from_account_address,
        transfer_to_new.token,
    );

    let (audit_path_to_after, audit_balance_path_to_after) = get_audits(
        tree,
        transfer_to_new.to_account_address,
        transfer_to_new.token,
    );

    //calculate a and b
    let a = balance_from_before;
    let mut b = amount_as_field_element;
    b.add_assign(&fee_as_field_element);
    TransferToNewWitness {
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
            eth_address: Some(transfer_to_new.new_address),
            amount_packed: Some(amount_encoded),
            full_amount: Some(amount_as_field_element),
            fee: Some(fee_encoded),
            a: Some(a),
            b: Some(b),
            pub_nonce: Some(Fr::zero()),
            new_pub_key_hash: Some(Fr::zero()),
        },
        before_root: Some(before_root),
        intermediate_root: Some(intermediate_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("2").unwrap()),
    }
}

pub fn calculate_transfer_to_new_operations_from_witness(
    transfer_witness: &TransferToNewWitness<Bn256>,
    first_sig_msg: &Fr,
    second_sig_msg: &Fr,
    third_sig_msg: &Fr,
    signature_data: &SignatureData,
    signer_pub_key_packed: &[Option<bool>],
) -> Vec<Operation<Bn256>> {
    let pubdata_chunks: Vec<_> = transfer_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();

    let operation_zero = Operation {
        new_root: transfer_witness.intermediate_root,
        tx_type: transfer_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: transfer_witness.args.clone(),
        lhs: transfer_witness.from_before.clone(),
        rhs: transfer_witness.to_before.clone(),
    };

    let operation_one = Operation {
        new_root: transfer_witness.after_root,
        tx_type: transfer_witness.tx_type,
        chunk: Some(Fr::from_str("1").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[1]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: transfer_witness.args.clone(),
        lhs: transfer_witness.from_intermediate.clone(),
        rhs: transfer_witness.to_intermediate.clone(),
    };

    let operation_two = Operation {
        new_root: transfer_witness.after_root,
        tx_type: transfer_witness.tx_type,
        chunk: Some(Fr::from_str("2").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[2]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: transfer_witness.args.clone(),
        lhs: transfer_witness.from_after.clone(),
        rhs: transfer_witness.to_after.clone(),
    };

    let operation_three = Operation {
        new_root: transfer_witness.after_root,
        tx_type: transfer_witness.tx_type,
        chunk: Some(Fr::from_str("3").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[3]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: transfer_witness.args.clone(),
        lhs: transfer_witness.from_after.clone(),
        rhs: transfer_witness.to_after.clone(),
    };

    let operation_four = Operation {
        new_root: transfer_witness.after_root,
        tx_type: transfer_witness.tx_type,
        chunk: Some(Fr::from_str("4").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[4]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: transfer_witness.args.clone(),
        lhs: transfer_witness.from_after.clone(),
        rhs: transfer_witness.to_after.clone(),
    };
    vec![
        operation_zero,
        operation_one,
        operation_two,
        operation_three,
        operation_four,
    ]
}
