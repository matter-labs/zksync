use crate::account;
use crate::account::AccountContent;
use crate::element::CircuitElement;
use crate::operation::{Operation, OperationBranch};
use crate::utils;
use crate::franklin_crypto::bellman::{ConstraintSystem, SynthesisError};
use crate::franklin_crypto::circuit::float_point::parse_with_exponent_le;
use models::params as franklin_constants;

use crate::franklin_crypto::circuit::boolean::Boolean;
use crate::franklin_crypto::circuit::num::AllocatedNum;

use crate::franklin_crypto::bellman::pairing::ff::PrimeField;
use crate::franklin_crypto::circuit::Assignment;
use crate::franklin_crypto::jubjub::JubjubEngine;
pub struct AllocatedOperationBranch<E: JubjubEngine> {
    pub account: AccountContent<E>,
    pub account_audit_path: Vec<AllocatedNum<E>>, //we do not need their bit representations
    pub account_address: CircuitElement<E>,
    pub balance: CircuitElement<E>,
    pub balance_audit_path: Vec<AllocatedNum<E>>,
    pub token: CircuitElement<E>,
}

impl<E: JubjubEngine> AllocatedOperationBranch<E> {
    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        operation_branch: &OperationBranch<E>,
    ) -> Result<AllocatedOperationBranch<E>, SynthesisError> {
        let account_address = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "account_address"),
            || Ok(operation_branch.address.grab()?),
            franklin_constants::account_tree_depth(),
        )?;
        let account_address = account_address.pad(franklin_constants::ACCOUNT_ID_BIT_WIDTH);

        let account_audit_path = utils::allocate_numbers_vec(
            cs.namespace(|| "account_audit_path"),
            &operation_branch.witness.account_path,
        )?;
        assert_eq!(
            account_audit_path.len(),
            franklin_constants::account_tree_depth()
        );

        let account = account::AccountContent::from_witness(
            cs.namespace(|| "allocate account_content"),
            &operation_branch.witness.account_witness,
        )?;

        let balance = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "balance"),
            || Ok(operation_branch.witness.balance_value.grab()?),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;

        let token = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "token"),
            || Ok(operation_branch.token.grab()?),
            franklin_constants::BALANCE_TREE_DEPTH,
        )?;
        let token = token.pad(franklin_constants::TOKEN_BIT_WIDTH);
        let balance_audit_path = utils::allocate_numbers_vec(
            cs.namespace(|| "balance_audit_path"),
            &operation_branch.witness.balance_subtree_path,
        )?;
        assert_eq!(
            balance_audit_path.len(),
            franklin_constants::BALANCE_TREE_DEPTH
        );

        Ok(AllocatedOperationBranch {
            account,
            account_audit_path,
            account_address,
            balance,
            token,
            balance_audit_path,
        })
    }
}

pub struct AllocatedChunkData<E: JubjubEngine> {
    pub is_chunk_last: Boolean,
    pub is_chunk_first: Boolean,
    pub chunk_number: AllocatedNum<E>, //TODO: don't need bit representation here, though make sense to unify probably
    pub tx_type: CircuitElement<E>,
}

#[derive(Clone)]
pub struct AllocatedOperationData<E: JubjubEngine> {
    pub amount_packed: CircuitElement<E>,
    pub fee_packed: CircuitElement<E>,
    pub amount_unpacked: CircuitElement<E>,
    pub full_amount: CircuitElement<E>,
    pub fee: CircuitElement<E>,
    pub first_sig_msg: CircuitElement<E>,
    pub second_sig_msg: CircuitElement<E>,
    pub third_sig_msg: CircuitElement<E>,
    pub new_pubkey_hash: CircuitElement<E>,
    pub ethereum_key: CircuitElement<E>,
    pub pub_nonce: CircuitElement<E>,
    pub a: CircuitElement<E>,
    pub b: CircuitElement<E>,
}

impl<E: JubjubEngine> AllocatedOperationData<E> {
    pub fn empty_from_zero(zero_element: AllocatedNum<E>) -> Result<Self, SynthesisError>
    {
        let ethereum_key = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        );

        let full_amount = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        let amount_packed = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH
                + franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        );

        let fee_packed = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::FEE_EXPONENT_BIT_WIDTH + franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        );

        let amount_unpacked = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        let fee = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        let first_sig_msg = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            E::Fr::CAPACITY as usize,
        );

        let second_sig_msg = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            E::Fr::CAPACITY as usize,
        );

        let third_sig_msg = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::MAX_CIRCUIT_PEDERSEN_HASH_BITS - (2 * E::Fr::CAPACITY as usize), //TODO: think of more consistent constant flow
        );

        let new_pubkey_hash = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );

        let pub_nonce = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::NONCE_BIT_WIDTH,
        );

        let a = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        let b = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        Ok(AllocatedOperationData {
            ethereum_key,
            pub_nonce,
            amount_packed,
            fee_packed,
            fee,
            amount_unpacked,
            full_amount,
            first_sig_msg,
            second_sig_msg,
            third_sig_msg,
            new_pubkey_hash,
            a,
            b,
        })
    }

    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        op: &Operation<E>,
        _params: &E::Params, //TODO: probably move out
    ) -> Result<AllocatedOperationData<E>, SynthesisError> {
        let ethereum_key = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "ethereum_key"),
            || op.args.ethereum_key.grab(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        )?;

        let full_amount = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "full_amount"),
            || op.args.full_amount.grab(),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;
        let amount_packed = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "amount_packed"),
            || op.args.amount_packed.grab(),
            franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH
                + franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        )?;
        let fee_packed = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "fee_packed"),
            || op.args.fee.grab(),
            franklin_constants::FEE_EXPONENT_BIT_WIDTH + franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        )?;

        let amount_parsed = parse_with_exponent_le(
            cs.namespace(|| "parse amount"),
            &amount_packed.get_bits_le(),
            franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )?;

        let fee_parsed = parse_with_exponent_le(
            cs.namespace(|| "parse fee"),
            &fee_packed.get_bits_le(),
            franklin_constants::FEE_EXPONENT_BIT_WIDTH,
            franklin_constants::FEE_MANTISSA_BIT_WIDTH,
            10,
        )?;
        let amount_unpacked = CircuitElement::from_number_with_known_length(
            cs.namespace(|| "amount"),
            amount_parsed,
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;
        let fee = CircuitElement::from_number_with_known_length(
            cs.namespace(|| "fee"),
            fee_parsed,
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;

        let first_sig_msg = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "first_part_signature_message"),
            || op.first_sig_msg.grab(),
            E::Fr::CAPACITY as usize,
        )?;

        let second_sig_msg = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "second_part_signature_message"),
            || op.second_sig_msg.grab(),
            E::Fr::CAPACITY as usize, //TODO: think of more consistent constant flow
        )?;

        let third_sig_msg = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "third_part_signature_message"),
            || op.third_sig_msg.grab(),
            franklin_constants::MAX_CIRCUIT_PEDERSEN_HASH_BITS - (2 * E::Fr::CAPACITY as usize), //TODO: think of more consistent constant flow
        )?;

        let new_pubkey_hash = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "new_pubkey_hash"),
            || op.args.new_pub_key_hash.grab(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        )?;

        let pub_nonce = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "pub_nonce"),
            || op.args.pub_nonce.grab(),
            franklin_constants::NONCE_BIT_WIDTH,
        )?;
        let a = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "a"),
            || op.args.a.grab(),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;
        let b = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "b"),
            || op.args.b.grab(),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;

        Ok(AllocatedOperationData {
            ethereum_key,
            pub_nonce,
            amount_packed,
            fee_packed,
            fee,
            amount_unpacked,
            full_amount,
            first_sig_msg,
            second_sig_msg,
            third_sig_msg,
            new_pubkey_hash,
            a,
            b,
        })
    }
}
