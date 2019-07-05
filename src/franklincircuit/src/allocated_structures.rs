use crate::account;
use crate::account::AccountContent;
use crate::element::{CircuitElement, CircuitPubkey};
use crate::operation::{Operation, OperationBranch};
use crate::utils;
use bellman::{ConstraintSystem, SynthesisError};
use franklin_crypto::circuit::float_point::parse_with_exponent_le;
use franklin_crypto::circuit::pedersen_hash;
use franklinmodels::params as franklin_constants;

use franklin_crypto::circuit::boolean::{AllocatedBit, Boolean};
use franklin_crypto::circuit::num::AllocatedNum;

use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::{JubjubEngine};

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
            *franklin_constants::BALANCE_TREE_DEPTH,
        )?;
        let balance_audit_path = utils::allocate_audit_path(
            cs.namespace(|| "balance_audit_path"),
            &operation_branch.witness.balance_subtree_path,
        )?;

        Ok(AllocatedOperationBranch {
            account: account,
            account_audit_path: account_audit_path,
            account_address: account_address,
            balance: balance,
            token: token,
            balance_audit_path: balance_audit_path,
        })
    }
}

pub struct AllocatedChunkData<E: JubjubEngine> {
    pub is_chunk_last: AllocatedBit,
    pub chunk_number: AllocatedNum<E>, //TODO: don't need bit representation here, though make sense to unify probably
    pub tx_type: CircuitElement<E>,
}

pub struct AllocatedOperationData<E: JubjubEngine> {
    pub new_pubkey: CircuitPubkey<E>,
    pub signer_pubkey: CircuitPubkey<E>,
    pub amount_packed: CircuitElement<E>,
    pub fee_packed: CircuitElement<E>,
    pub amount: CircuitElement<E>,
    pub fee: CircuitElement<E>,
    pub sig_msg: CircuitElement<E>,
    pub new_pubkey_hash: CircuitElement<E>,
    pub a: CircuitElement<E>,
    pub b: CircuitElement<E>,
}

impl<E: JubjubEngine> AllocatedOperationData<E> {
    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        op: &Operation<E>,
        params: &E::Params, //TODO: probably move out
    ) -> Result<AllocatedOperationData<E>, SynthesisError> {
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

        let amount_parsed = parse_with_exponent_le(
            cs.namespace(|| "parse amount"),
            &amount_packed.get_bits_le(),
            *franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
            *franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )?;

        let fee_parsed = parse_with_exponent_le(
            cs.namespace(|| "parse fee"),
            &fee_packed.get_bits_le(),
            *franklin_constants::FEE_EXPONENT_BIT_WIDTH,
            *franklin_constants::FEE_MANTISSA_BIT_WIDTH,
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
        )?;

        let new_pubkey = CircuitPubkey::from_xy_fe(
            cs.namespace(|| "new_pubkey"),
            || op.args.new_pub_x.grab(),
            || op.args.new_pub_y.grab(),
        )?;

        let new_pubkey_bits = new_pubkey.get_packed_key();

        let new_pubkey_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "new_pubkey_hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &new_pubkey_bits,
            params,
        )?
        .get_x()
        .clone();

        //length not enforced, cause we intentionally truncate data here
        let new_pubkey_hash_ce = CircuitElement::from_number(
            cs.namespace(|| "new_pubkehy_hash_ce"),
            new_pubkey_hash,
            *franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        )?;

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
            new_pubkey: new_pubkey,
            signer_pubkey: sig_pubkey,
            amount_packed: amount_packed,
            fee_packed: fee_packed,
            fee: fee,
            amount: amount,
            sig_msg: sig_msg,
            new_pubkey_hash: new_pubkey_hash_ce,
            a: a,
            b: b,
        })
    }
}
