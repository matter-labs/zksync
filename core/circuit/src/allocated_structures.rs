use crate::account;
use crate::account::AccountContent;
use crate::element::{CircuitElement, CircuitPubkey};
use crate::operation::{Operation, OperationBranch};
use crate::utils;
use bellman::{ConstraintSystem, SynthesisError};
use franklin_crypto::circuit::float_point::parse_with_exponent_le;
use models::params as franklin_constants;

use franklin_crypto::circuit::boolean::Boolean;
use franklin_crypto::circuit::num::AllocatedNum;

use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::JubjubEngine;

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
        let account_address = CircuitElement::from_fe_strict(
            cs.namespace(|| "account_address"),
            || Ok(operation_branch.address.grab()?),
            franklin_constants::ACCOUNT_TREE_DEPTH,
        )?;
        let account_address = account_address.pad(franklin_constants::ACCOUNT_ID_BIT_WIDTH);

        let account_audit_path = utils::allocate_audit_path(
            cs.namespace(|| "account_audit_path"),
            &operation_branch.witness.account_path,
        )?;
        assert_eq!(
            account_audit_path.len(),
            franklin_constants::ACCOUNT_TREE_DEPTH
        );

        let account = account::AccountContent::from_witness(
            cs.namespace(|| "allocate account_content"),
            &operation_branch.witness.account_witness,
        )?;

        let balance = CircuitElement::from_fe_strict(
            cs.namespace(|| "balance"),
            || Ok(operation_branch.witness.balance_value.grab()?),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;

        let token = CircuitElement::from_fe_strict(
            cs.namespace(|| "token"),
            || Ok(operation_branch.token.grab()?),
            franklin_constants::BALANCE_TREE_DEPTH,
        )?;
        let token = token.pad(franklin_constants::TOKEN_BIT_WIDTH);
        let balance_audit_path = utils::allocate_audit_path(
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
    pub chunk_number: AllocatedNum<E>, //TODO: don't need bit representation here, though make sense to unify probably
    pub tx_type: CircuitElement<E>,
}

#[derive(Clone)]
pub struct AllocatedOperationData<E: JubjubEngine> {
    // pub new_pubkey: CircuitPubkey<E>,
    pub signer_pubkey: CircuitPubkey<E>,
    pub amount_packed: CircuitElement<E>,
    pub fee_packed: CircuitElement<E>,
    pub amount: CircuitElement<E>,
    pub fee: CircuitElement<E>,
    pub sig_msg: CircuitElement<E>,
    pub new_pubkey_hash: CircuitElement<E>,
    pub ethereum_key: CircuitElement<E>,
    pub a: CircuitElement<E>,
    pub b: CircuitElement<E>,
}

impl<E: JubjubEngine> AllocatedOperationData<E> {
    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        op: &Operation<E>,
        params: &E::Params, //TODO: probably move out
    ) -> Result<AllocatedOperationData<E>, SynthesisError> {
        let ethereum_key = CircuitElement::from_fe_strict(
            cs.namespace(|| "ethereum_key"),
            || op.args.ethereum_key.grab(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        )?;
        let amount_packed = CircuitElement::from_fe_strict(
            cs.namespace(|| "amount_packed"),
            || op.args.amount.grab(),
            franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH
                + franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        )?;
        let fee_packed = CircuitElement::from_fe_strict(
            cs.namespace(|| "fee_packed"),
            || op.args.fee.grab(),
            franklin_constants::FEE_EXPONENT_BIT_WIDTH + franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        )?;
        //        println!(
        //            "fee_packed in allocated_operation_data equals {}",
        //            fee_packed.get_number().get_value().grab()?
        //        );

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
        let amount = CircuitElement::from_number(
            cs.namespace(|| "amount"),
            amount_parsed,
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;
        let fee = CircuitElement::from_number(
            cs.namespace(|| "fee"),
            fee_parsed,
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;

        let sig_msg = CircuitElement::from_fe_strict(
            cs.namespace(|| "signature_message_x"),
            || op.sig_msg.grab(),
            franklin_constants::FR_BIT_WIDTH,
        )?; //TODO: not sure if this is correct length
        let sig_pubkey = CircuitPubkey::from_xy_fe(
            cs.namespace(|| "signer_pubkey"),
            || op.signer_pub_key_x.grab(),
            || op.signer_pub_key_y.grab(),
            &params,
        )?;

        let new_pubkey_hash = CircuitElement::from_fe_strict(
            cs.namespace(|| "new_pubkey_hash"),
            || op.args.new_pub_key_hash.grab(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        )?;
        // let new_pubkey_hash = new_pubkey.get_hash().clone();

        let a = CircuitElement::from_fe_strict(
            cs.namespace(|| "a"),
            || op.args.a.grab(),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;
        let b = CircuitElement::from_fe_strict(
            cs.namespace(|| "b"),
            || op.args.b.grab(),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;

        Ok(AllocatedOperationData {
            ethereum_key,
            signer_pubkey: sig_pubkey,
            amount_packed,
            fee_packed,
            fee,
            amount,
            sig_msg,
            new_pubkey_hash,
            a,
            b,
        })
    }
}
