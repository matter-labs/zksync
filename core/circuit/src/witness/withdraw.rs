use super::utils::*;

use crate::operation::*;

use crate::franklin_crypto::bellman::pairing::ff::{Field, PrimeField};

use crate::franklin_crypto::circuit::float_point::convert_to_float;
use crate::franklin_crypto::jubjub::JubjubEngine;
use crate::operation::SignatureData;
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{
    append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element,
};

use crate::franklin_crypto::bellman::pairing::bn256::*;
use models::node::WithdrawOp;
use models::params as franklin_constants;
use models::primitives::big_decimal_to_u128;

pub struct WithdrawData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub account_address: u32,
    pub eth_address: Fr,
}
pub struct WithdrawWitness<E: JubjubEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}
impl<E: JubjubEngine> WithdrawWitness<E> {
    pub fn get_pubdata(&self) -> Vec<bool> {
        let mut pubdata_bits = vec![];
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.tx_type.unwrap(),
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.address.unwrap(),
            franklin_constants::ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.full_amount.unwrap(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            franklin_constants::ETH_ADDRESS_BIT_WIDTH,
        );
        pubdata_bits.resize(6 * franklin_constants::CHUNK_BIT_WIDTH, false);
        pubdata_bits
    }
    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("3").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.eth_address.unwrap(),
            franklin_constants::ETH_ADDRESS_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.full_amount.unwrap(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.fee.unwrap(),
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}
pub fn apply_withdraw_tx(
    tree: &mut CircuitAccountTree,
    withdraw: &WithdrawOp,
) -> WithdrawWitness<Bn256> {
    let withdraw_data = WithdrawData {
        amount: big_decimal_to_u128(&withdraw.tx.amount),
        fee: big_decimal_to_u128(&withdraw.tx.fee),
        token: u32::from(withdraw.tx.token),
        account_address: withdraw.account_id,
        eth_address: eth_address_to_fr(&withdraw.tx.to),
    };
    // le_bit_vector_into_field_element()
    apply_withdraw(tree, &withdraw_data)
}
pub fn apply_withdraw(
    tree: &mut CircuitAccountTree,
    withdraw: &WithdrawData,
) -> WithdrawWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    debug!("Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, withdraw.account_address, withdraw.token);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_address_fe = Fr::from_str(&withdraw.account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&withdraw.token.to_string()).unwrap();
    let amount_as_field_element = Fr::from_str(&withdraw.amount.to_string()).unwrap();

    let amount_bits = convert_to_float(
        withdraw.amount,
        franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

    let fee_as_field_element = Fr::from_str(&withdraw.fee.to_string()).unwrap();

    let fee_bits = convert_to_float(
        withdraw.fee,
        franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

    //calculate a and b

    //applying withdraw

    let (account_witness_before, account_witness_after, balance_before, balance_after) =
        apply_leaf_operation(
            tree,
            withdraw.account_address,
            withdraw.token,
            |acc| {
                acc.nonce.add_assign(&Fr::from_str("1").unwrap());
            },
            |bal| {
                bal.value.sub_assign(&amount_as_field_element);
                bal.value.sub_assign(&fee_as_field_element);
            },
        );

    let after_root = tree.root_hash();
    debug!("After root = {}", after_root);
    let (audit_path_after, audit_balance_path_after) =
        get_audits(tree, withdraw.account_address, withdraw.token);

    let a = balance_before;
    let mut b = amount_as_field_element;
    b.add_assign(&fee_as_field_element);

    WithdrawWitness {
        before: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_before,
                account_path: audit_path_before,
                balance_value: Some(balance_before),
                balance_subtree_path: audit_balance_path_before,
            },
        },
        after: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_after,
                account_path: audit_path_after,
                balance_value: Some(balance_after),
                balance_subtree_path: audit_balance_path_after,
            },
        },
        args: OperationArguments {
            eth_address: Some(withdraw.eth_address),
            amount_packed: Some(amount_encoded),
            full_amount: Some(amount_as_field_element),
            fee: Some(fee_encoded),
            pub_nonce: Some(Fr::zero()),
            a: Some(a),
            b: Some(b),
            new_pub_key_hash: Some(Fr::zero()),
        },
        before_root: Some(before_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("3").unwrap()),
    }
}
pub fn calculate_withdraw_operations_from_witness(
    withdraw_witness: &WithdrawWitness<Bn256>,
    first_sig_msg: &Fr,
    second_sig_msg: &Fr,
    third_sig_msg: &Fr,
    signature_data: &SignatureData,
    signer_pub_key_packed: &[Option<bool>],
) -> Vec<Operation<Bn256>> {
    let pubdata_chunks: Vec<_> = withdraw_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();

    let operation_zero = Operation {
        new_root: withdraw_witness.after_root,
        tx_type: withdraw_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: withdraw_witness.args.clone(),
        lhs: withdraw_witness.before.clone(),
        rhs: withdraw_witness.before.clone(),
    };

    let operation_one = Operation {
        new_root: withdraw_witness.after_root,
        tx_type: withdraw_witness.tx_type,
        chunk: Some(Fr::from_str("1").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[1]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: withdraw_witness.args.clone(),
        lhs: withdraw_witness.after.clone(),
        rhs: withdraw_witness.after.clone(),
    };

    let operation_two = Operation {
        new_root: withdraw_witness.after_root,
        tx_type: withdraw_witness.tx_type,
        chunk: Some(Fr::from_str("2").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[2]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: withdraw_witness.args.clone(),
        lhs: withdraw_witness.after.clone(),
        rhs: withdraw_witness.after.clone(),
    };

    let operation_three = Operation {
        new_root: withdraw_witness.after_root,
        tx_type: withdraw_witness.tx_type,
        chunk: Some(Fr::from_str("3").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[3]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: withdraw_witness.args.clone(),
        lhs: withdraw_witness.after.clone(),
        rhs: withdraw_witness.after.clone(),
    };
    let operation_four = Operation {
        new_root: withdraw_witness.after_root,
        tx_type: withdraw_witness.tx_type,
        chunk: Some(Fr::from_str("4").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[4]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: withdraw_witness.args.clone(),
        lhs: withdraw_witness.after.clone(),
        rhs: withdraw_witness.after.clone(),
    };
    let operation_five = Operation {
        new_root: withdraw_witness.after_root,
        tx_type: withdraw_witness.tx_type,
        chunk: Some(Fr::from_str("5").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[5]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: withdraw_witness.args.clone(),
        lhs: withdraw_witness.after.clone(),
        rhs: withdraw_witness.after.clone(),
    };
    vec![
        operation_zero,
        operation_one,
        operation_two,
        operation_three,
        operation_four,
        operation_five,
    ]
}
#[cfg(test)]
mod test {
    use super::*;

    use crate::witness::test_utils::{check_circuit, test_genesis_plasma_state};
    use bigdecimal::BigDecimal;
    use models::node::Account;
    use web3::types::Address;

    #[test]
    #[ignore]
    fn test_withdraw() {
        use testkit::zksync_account::ZksyncAccount;

        let zksync_account = ZksyncAccount::rand();
        let account_id = 1;
        let account_address = zksync_account.address;
        let account = {
            let mut account = Account::default_with_address(&account_address);
            account.add_balance(0, &BigDecimal::from(10));
            account.pub_key_hash = zksync_account.pubkey_hash.clone();
            account
        };

        let (mut plasma_state, mut circuit_account_tree) =
            test_genesis_plasma_state(vec![(account_id, account)]);
        let fee_account_id = 0;
        let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

        let withdraw_op = WithdrawOp {
            tx: zksync_account
                .sign_withdraw(
                    0,
                    "",
                    BigDecimal::from(7),
                    BigDecimal::from(3),
                    &Address::zero(),
                    None,
                    true,
                )
                .0,
            account_id,
        };

        println!("node root hash before op: {:?}", plasma_state.root_hash());
        let (fee, _) = plasma_state
            .apply_withdraw_op(&withdraw_op)
            .expect("transfer should be success");
        println!("node root hash after op: {:?}", plasma_state.root_hash());
        plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);
        println!("node root hash after fee: {:?}", plasma_state.root_hash());
        println!(
            "node withdraw tx bytes: {}",
            hex::encode(&withdraw_op.tx.get_bytes())
        );

        let withdraw_witness = apply_withdraw_tx(&mut witness_accum.account_tree, &withdraw_op);
        let sign_packed = withdraw_op
            .tx
            .signature
            .signature
            .serialize_packed()
            .expect("signature serialize");
        let (first_sig_msg, second_sig_msg, third_sig_msg, signature_data, signer_packed_key_bits) =
            prepare_sig_data(
                &sign_packed,
                &withdraw_op.tx.get_bytes(),
                &withdraw_op.tx.signature.pub_key,
            )
            .expect("prepare signature data");
        let withdraw_operations = calculate_withdraw_operations_from_witness(
            &withdraw_witness,
            &first_sig_msg,
            &second_sig_msg,
            &third_sig_msg,
            &signature_data,
            &signer_packed_key_bits,
        );
        let pub_data_from_witness = withdraw_witness.get_pubdata();

        witness_accum.add_operation_with_pubdata(withdraw_operations, pub_data_from_witness);
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
