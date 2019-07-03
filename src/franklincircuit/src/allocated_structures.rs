use crate::account;
use crate::account::{AccountContentBase, AccountContentBitForm};
use crate::operation::{Operation, OperationBranch, OperationBranchWitness};
use crate::utils;
use franklinmodels::params as franklin_constants;

use bellman::{ConstraintSystem, SynthesisError};

use franklin_crypto::circuit::boolean::{AllocatedBit, Boolean};
use franklin_crypto::circuit::num::AllocatedNum;

use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};

pub struct AllocatedOperationBranch<E: JubjubEngine> {
    pub base: AllocatedOperationBranchBase<E>,
    pub bits: AllocatedOperationBranchBitForm,
}

pub struct AllocatedOperationBranchBase<E: JubjubEngine> {
    pub account: AccountContentBase<E>,
    pub account_audit_path: Vec<AllocatedNum<E>>,
    pub account_address: AllocatedNum<E>,

    pub balance_value: AllocatedNum<E>,
    pub balance_audit_path: Vec<AllocatedNum<E>>,
    pub token: AllocatedNum<E>,
}

//TODO: we should limit bit_widths here. yes, constraint bit lengths
impl<E: JubjubEngine> AllocatedOperationBranchBase<E> {
    pub fn make_bit_form<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
    ) -> Result<AllocatedOperationBranchBitForm, SynthesisError> {
        let mut account_address_bits = self
            .account_address
            .into_bits_le(cs.namespace(|| "account_address_bits"))?;
        account_address_bits.truncate(*franklin_constants::ACCOUNT_TREE_DEPTH);

        let mut token_bits = self.token.into_bits_le(cs.namespace(|| "token_bits"))?;
        token_bits.truncate(*franklin_constants::BALANCE_TREE_DEPTH);

        let account_bit_form = self
            .account
            .make_bit_form(cs.namespace(|| "account_bit_form"))?;

        let mut balance_bit_form = self
            .balance_value
            .into_bits_le(cs.namespace(|| "balance_value_bits"))?;
        balance_bit_form.truncate(*franklin_constants::BALANCE_BIT_WIDTH);

        Ok(AllocatedOperationBranchBitForm {
            account: account_bit_form,
            account_address: account_address_bits,
            token: token_bits,
            balance_value: balance_bit_form,
        })
    }
}
pub struct AllocatedOperationBranchBitForm {
    pub account: AccountContentBitForm,
    pub account_address: Vec<Boolean>,

    pub token: Vec<Boolean>,
    pub balance_value: Vec<Boolean>,
}

pub struct AllocatedChunkData<E: JubjubEngine> {
    pub is_chunk_last: AllocatedBit,
    pub chunk_number: AllocatedNum<E>,
    pub tx_type: AllocatedNum<E>,
    pub tx_type_bits: Vec<Boolean>,
}

pub fn allocate_operation_branch<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    operation_branch: &OperationBranch<E>,
) -> Result<AllocatedOperationBranch<E>, SynthesisError> {
    let account_address_allocated =
        AllocatedNum::alloc(cs.namespace(|| "account_address"), || {
            operation_branch.address.grab()
        })?;

    let allocated_account_audit_path = utils::allocate_audit_path(
        cs.namespace(|| "account_audit_path"),
        &operation_branch.witness.account_path,
    )?;
    assert_eq!(
        allocated_account_audit_path.len(),
        *franklin_constants::ACCOUNT_TREE_DEPTH
    );

    let account_base = account::make_account_content_base(
        cs.namespace(|| "allocate account_content"),
        &operation_branch.witness.account_witness,
    )?;

    let balance_value_allocated = AllocatedNum::alloc(cs.namespace(|| "balance_value"), || {
        operation_branch.witness.balance_value.grab()
    })?;
    let token_allocated =
        AllocatedNum::alloc(cs.namespace(|| "token"), || operation_branch.token.grab())?;

    let allocated_balance_audit_path = utils::allocate_audit_path(
        cs.namespace(|| "balance_audit_path"),
        &operation_branch.witness.balance_subtree_path,
    )?;

    let operation_branch_data = AllocatedOperationBranchBase {
        account_address: account_address_allocated,
        account: account_base,
        account_audit_path: allocated_account_audit_path,
        balance_value: balance_value_allocated,
        balance_audit_path: allocated_balance_audit_path,
        token: token_allocated,
    };
    let operation_branch_bit_form =
        operation_branch_data.make_bit_form(cs.namespace(|| "operation_branch_data_bit_form"))?;

    Ok(AllocatedOperationBranch {
        base: operation_branch_data,
        bits: operation_branch_bit_form,
    })
}

pub struct AllocatedOperationData<E: JubjubEngine> {
    pub new_pubkey_x: AllocatedNum<E>,
    pub new_pubkey_y: AllocatedNum<E>,
    pub amount: AllocatedNum<E>,
    pub fee: AllocatedNum<E>,
    pub sig_msg: AllocatedNum<E>,
    pub sig_msg_bits: Vec<Boolean>,
    pub new_pubkey_hash: Vec<Boolean>,
    pub fee_packed: Vec<Boolean>,
    pub amount_packed: Vec<Boolean>,
    pub signer_pub_x: AllocatedNum<E>,
    pub signer_pub_y: AllocatedNum<E>,
    pub a: AllocatedNum<E>,
    pub b: AllocatedNum<E>,
}
