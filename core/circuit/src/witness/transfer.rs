use super::utils::*;
use crate::operation::SignatureData;
use crate::operation::*;
use ff::{Field, PrimeField};
use franklin_crypto::circuit::float_point::convert_to_float;
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{append_be_fixed_width, le_bit_vector_into_field_element};
use models::params as franklin_constants;
use pairing::bn256::*;

use models::node::TransferOp;
use models::primitives::big_decimal_to_u128;

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
            &self.to_before.address.unwrap(),
            franklin_constants::ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.amount_packed.unwrap(),
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );
        pubdata_bits.resize(2 * franklin_constants::CHUNK_BIT_WIDTH, false); //TODO verify if right padding is okay
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
            &self.to_before.witness.account_witness.pub_key_hash.unwrap(),
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
pub fn apply_transfer_tx(
    tree: &mut CircuitAccountTree,
    transfer: &TransferOp,
) -> TransferWitness<Bn256> {
    let transfer_data = TransferData {
        amount: big_decimal_to_u128(&transfer.tx.amount),
        fee: big_decimal_to_u128(&transfer.tx.fee),
        token: u32::from(transfer.tx.token),
        from_account_address: transfer.from,
        to_account_address: transfer.to,
    };
    // le_bit_vector_into_field_element()
    apply_transfer(tree, &transfer_data)
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
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_address_from_fe = Fr::from_str(&transfer.from_account_address.to_string()).unwrap();
    let account_address_to_fe = Fr::from_str(&transfer.to_account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&transfer.token.to_string()).unwrap();
    let amount_as_field_element = Fr::from_str(&transfer.amount.to_string()).unwrap();

    let amount_bits = convert_to_float(
        transfer.amount,
        franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

    let fee_as_field_element = Fr::from_str(&transfer.fee.to_string()).unwrap();

    let fee_bits = convert_to_float(
        transfer.fee,
        franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        franklin_constants::FEE_MANTISSA_BIT_WIDTH,
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
        transfer.to_account_address,
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
    let a = balance_from_before;
    let mut b = amount_as_field_element;
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
            ethereum_key: Some(Fr::zero()),
            amount_packed: Some(amount_encoded),
            full_amount: Some(amount_as_field_element),
            fee: Some(fee_encoded),
            pub_nonce: Some(Fr::zero()),
            a: Some(a),
            b: Some(b),
            new_pub_key_hash: Some(Fr::zero()),
        },
        before_root: Some(before_root),
        intermediate_root: Some(intermediate_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("5").unwrap()),
    }
}

pub fn calculate_transfer_operations_from_witness(
    transfer_witness: &TransferWitness<Bn256>,
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
        eth_signature_data: ETHSignatureData::init_empty(),
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
        eth_signature_data: ETHSignatureData::init_empty(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: transfer_witness.args.clone(),
        lhs: transfer_witness.from_intermediate.clone(),
        rhs: transfer_witness.to_intermediate.clone(),
    };
    vec![operation_zero, operation_one]
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::witness::test_utils::{check_circuit, test_genesis_plasma_state};
    use bigdecimal::BigDecimal;
    use models::node::Account;
    use testkit::zksync_account::ZksyncAccount;

    #[test]
    #[ignore]
    fn test_transfer_success() {
        let from_zksync_account = ZksyncAccount::rand();
        let from_account_id = 1;
        let from_account_address = from_zksync_account.address;
        let from_account = {
            let mut account = Account::default_with_address(&from_account_address);
            account.add_balance(0, &BigDecimal::from(10));
            account.pub_key_hash = from_zksync_account.pubkey_hash.clone();
            account
        };

        let to_account_id = 2;
        let to_account_address = "2222222222222222222222222222222222222222".parse().unwrap();
        let to_account = Account::default_with_address(&to_account_address);

        let (mut plasma_state, mut witness_accum) = test_genesis_plasma_state(vec![
            (from_account_id, from_account),
            (to_account_id, to_account),
        ]);

        let transfer_op = TransferOp {
            tx: from_zksync_account.sign_transfer(
                0,
                BigDecimal::from(7),
                BigDecimal::from(3),
                &to_account_address,
                None,
                true,
            ),
            from: from_account_id,
            to: to_account_id,
        };

        let (fee, _) = plasma_state
            .apply_transfer_op(&transfer_op)
            .expect("transfer should be success");
        plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);

        let transfer_witness = apply_transfer_tx(&mut witness_accum.account_tree, &transfer_op);
        let sign_packed = transfer_op
            .tx
            .signature
            .signature
            .serialize_packed()
            .expect("signature serialize");
        let (first_sig_msg, second_sig_msg, third_sig_msg, signature_data, signer_packed_key_bits) =
            prepare_sig_data(
                &sign_packed,
                &transfer_op.tx.get_bytes(),
                &transfer_op.tx.signature.pub_key,
            )
            .expect("prepare signature data");
        let transfer_operations = calculate_transfer_operations_from_witness(
            &transfer_witness,
            &first_sig_msg,
            &second_sig_msg,
            &third_sig_msg,
            &signature_data,
            &signer_packed_key_bits,
        );
        let pub_data_from_witness = transfer_witness.get_pubdata();

        witness_accum.add_operation_with_pubdata(transfer_operations, pub_data_from_witness);
        witness_accum.collect_fees(&[fee]);
        witness_accum.calculate_pubdata_commitment();

        assert_eq!(
            plasma_state.root_hash(),
            witness_accum
                .root_after_fees
                .expect("witness accum after root hash empty"),
            "root hash in state keeper and witness generation code mismatch"
        );

        check_circuit(witness_accum.into_circuit_instance());
    }
}
