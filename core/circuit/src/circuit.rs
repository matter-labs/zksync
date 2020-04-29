use crate::account::AccountContent;
use crate::account::AccountWitness;
use crate::allocated_structures::*;
use crate::element::CircuitElement;
use crate::franklin_crypto::bellman::pairing::ff::{Field, PrimeField};
use crate::franklin_crypto::bellman::{Circuit, ConstraintSystem, SynthesisError};
use crate::franklin_crypto::circuit::boolean::Boolean;
use crate::franklin_crypto::circuit::ecc;
use crate::franklin_crypto::circuit::sha256;
use crate::operation::Operation;
use crate::signature::*;
use crate::utils::{allocate_numbers_vec, allocate_sum, multi_and, pack_bits_to_element};

use crate::franklin_crypto::circuit::expression::Expression;
use crate::franklin_crypto::circuit::multipack;
use crate::franklin_crypto::circuit::num::AllocatedNum;
use crate::franklin_crypto::circuit::polynomial_lookup::{do_the_lookup, generate_powers};
use crate::franklin_crypto::circuit::rescue;
use crate::franklin_crypto::circuit::Assignment;
use crate::franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};
use crate::franklin_crypto::rescue::RescueEngine;
use models::node::operations::{ChangePubKeyOp, NoopOp};
use models::node::{CloseOp, DepositOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp};
use models::params::{self, FR_BIT_WIDTH_PADDED};

const DIFFERENT_TRANSACTIONS_TYPE_NUMBER: usize = 8;
pub struct FranklinCircuit<'a, E: RescueEngine + JubjubEngine> {
    pub rescue_params: &'a <E as RescueEngine>::Params,
    pub jubjub_params: &'a <E as JubjubEngine>::Params,
    pub operation_batch_size: usize,
    /// The old root of the tree
    pub old_root: Option<E::Fr>,

    /// The new root of the tree
    pub new_root: Option<E::Fr>,
    pub block_number: Option<E::Fr>,
    pub validator_address: Option<E::Fr>,

    pub pub_data_commitment: Option<E::Fr>,
    pub operations: Vec<Operation<E>>,

    pub validator_balances: Vec<Option<E::Fr>>,
    pub validator_audit_path: Vec<Option<E::Fr>>,
    pub validator_account: AccountWitness<E>,
}

impl<'a, E: RescueEngine + JubjubEngine> std::clone::Clone for FranklinCircuit<'a, E> {
    fn clone(&self) -> Self {
        Self {
            rescue_params: self.rescue_params,
            jubjub_params: self.jubjub_params,
            operation_batch_size: self.operation_batch_size,
            old_root: self.old_root,
            new_root: self.new_root,
            block_number: self.block_number,
            validator_address: self.validator_address,
            pub_data_commitment: self.pub_data_commitment,
            operations: self.operations.clone(),

            validator_balances: self.validator_balances.clone(),
            validator_audit_path: self.validator_audit_path.clone(),
            validator_account: self.validator_account.clone(),
        }
    }
}

struct PreviousData<E: RescueEngine> {
    op_data: AllocatedOperationData<E>,
}

// Implementation of our circuit:
impl<'a, E: RescueEngine + JubjubEngine> Circuit<E> for FranklinCircuit<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let zero = AllocatedNum::alloc(cs.namespace(|| "allocate element equal to zero"), || {
            Ok(E::Fr::zero())
        })?;

        zero.assert_zero(cs.namespace(|| "enforce zero on the zero element"))?;

        // we only need this for consistency of first operation
        let zero_circuit_element = CircuitElement::unsafe_empty_of_some_length(zero.clone(), 256);

        let mut prev = PreviousData {
            op_data: AllocatedOperationData::empty_from_zero(zero.clone())?,
        };
        // this is only public input to our circuit
        let public_data_commitment =
            AllocatedNum::alloc(cs.namespace(|| "public_data_commitment"), || {
                self.pub_data_commitment.grab()
            })?;
        public_data_commitment.inputize(cs.namespace(|| "inputize pub_data"))?;

        let validator_address_padded = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "validator_address"),
            || self.validator_address.grab(),
            params::ACCOUNT_ID_BIT_WIDTH,
        )?;

        let validator_address_bits = validator_address_padded.get_bits_le();
        assert_eq!(validator_address_bits.len(), params::ACCOUNT_ID_BIT_WIDTH);

        let mut validator_balances = allocate_numbers_vec(
            cs.namespace(|| "validator_balances"),
            &self.validator_balances,
        )?;
        assert_eq!(validator_balances.len(), params::total_tokens());

        let validator_audit_path = allocate_numbers_vec(
            cs.namespace(|| "validator_audit_path"),
            &self.validator_audit_path,
        )?;
        assert_eq!(validator_audit_path.len(), params::account_tree_depth());

        let validator_account = AccountContent::from_witness(
            cs.namespace(|| "validator account"),
            &self.validator_account,
        )?;

        let mut rolling_root =
            AllocatedNum::alloc(cs.namespace(|| "rolling_root"), || self.old_root.grab())?;

        let old_root =
            CircuitElement::from_number(cs.namespace(|| "old_root"), rolling_root.clone())?;
        // first chunk of block should always have number 0
        let mut next_chunk_number = zero;

        // declare vector of fees, that will be collected during block processing
        let mut fees = vec![];
        let fees_len = params::total_tokens();
        for _ in 0..fees_len {
            fees.push(zero_circuit_element.get_number());
        }
        // vector of pub_data_bits that will be aggregated during block processing
        let mut block_pub_data_bits = vec![];

        let mut allocated_chunk_data: AllocatedChunkData<E> = AllocatedChunkData {
            is_chunk_last: Boolean::constant(false),
            is_chunk_first: Boolean::constant(false),
            chunk_number: zero_circuit_element.get_number(),
            tx_type: zero_circuit_element,
        };

        // Main cycle that processes operations:
        for (i, operation) in self.operations.iter().enumerate() {
            let cs = &mut cs.namespace(|| format!("chunk number {}", i));

            let (next_chunk, chunk_data) = self.verify_correct_chunking(
                &operation,
                &next_chunk_number,
                cs.namespace(|| "verify_correct_chunking"),
            )?;

            allocated_chunk_data = chunk_data;
            next_chunk_number = next_chunk;
            let operation_pub_data_chunk = CircuitElement::from_fe_with_known_length(
                cs.namespace(|| "operation_pub_data_chunk"),
                || operation.clone().pubdata_chunk.grab(),
                params::CHUNK_BIT_WIDTH,
            )?;
            block_pub_data_bits.extend(operation_pub_data_chunk.get_bits_le());

            let lhs =
                AllocatedOperationBranch::from_witness(cs.namespace(|| "lhs"), &operation.lhs)?;
            let rhs =
                AllocatedOperationBranch::from_witness(cs.namespace(|| "rhs"), &operation.rhs)?;
            let mut current_branch = self.select_branch(
                cs.namespace(|| "select appropriate branch"),
                &lhs,
                &rhs,
                operation,
                &allocated_chunk_data,
            )?;
            // calculate root for given account data
            let (state_root, is_account_empty, _subtree_root) = check_account_data(
                cs.namespace(|| "calculate account root"),
                &current_branch,
                self.rescue_params,
            )?;

            // ensure root hash of state before applying operation is correct
            cs.enforce(
                || "root state before applying operation is valid",
                |lc| lc + state_root.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + rolling_root.get_variable(),
            );
            self.execute_op(
                cs.namespace(|| "execute_op"),
                &mut current_branch,
                &lhs,
                &rhs,
                &operation,
                &allocated_chunk_data,
                &is_account_empty,
                &operation_pub_data_chunk.get_number(),
                // &subtree_root, // Close disable
                &mut fees,
                &mut prev,
            )?;
            let (new_state_root, _, _) = check_account_data(
                cs.namespace(|| "calculate new account root"),
                &current_branch,
                self.rescue_params,
            )?;

            rolling_root = new_state_root;
        }

        cs.enforce(
            || "ensure last chunk of the block is a last chunk of corresponding transaction",
            |_| {
                allocated_chunk_data
                    .is_chunk_last
                    .lc(CS::one(), E::Fr::one())
            },
            |lc| lc + CS::one(),
            |lc| lc + CS::one(),
        );

        // calculate operator's balance_tree root hash from whole tree representation
        let old_operator_balance_root = calculate_root_from_full_representation_fees(
            cs.namespace(|| "calculate_root_from_full_representation_fees before"),
            &validator_balances,
            self.rescue_params,
        )?;

        let mut operator_account_data = vec![];
        let old_operator_state_root = {
            let balance_root = CircuitElement::from_number(
                cs.namespace(|| "old_operator_balance_root_ce"),
                old_operator_balance_root,
            )?;
            calc_account_state_tree_root(
                cs.namespace(|| "old_operator_state_root"),
                &balance_root,
                &self.rescue_params,
            )?
        };
        operator_account_data.extend(validator_account.nonce.get_bits_le());
        operator_account_data.extend(validator_account.pub_key_hash.get_bits_le());
        operator_account_data.extend(validator_account.address.get_bits_le());
        operator_account_data
            .extend(old_operator_state_root.into_padded_le_bits(FR_BIT_WIDTH_PADDED));

        let root_from_operator = allocate_merkle_root(
            cs.namespace(|| "root from operator_account"),
            &operator_account_data,
            &validator_address_bits,
            &validator_audit_path,
            self.rescue_params,
        )?;

        // ensure that this operator leaf is correct for our tree state
        cs.enforce(
            || "root before applying fees is correct",
            |lc| lc + root_from_operator.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + rolling_root.get_variable(),
        );

        //apply fees to operator balances
        for i in 0..fees_len {
            validator_balances[i] = allocate_sum(
                cs.namespace(|| format!("validator balance number i {}", i)),
                &validator_balances[i],
                &fees[i],
            )?;
        }

        // calculate operator's balance_tree root from all leafs
        let new_operator_balance_root = calculate_root_from_full_representation_fees(
            cs.namespace(|| "calculate_root_from_full_representation_fees after"),
            &validator_balances,
            self.rescue_params,
        )?;

        let mut operator_account_data = vec![];
        let new_operator_state_root = {
            let balance_root = CircuitElement::from_number(
                cs.namespace(|| "new_operator_balance_root_ce"),
                new_operator_balance_root,
            )?;
            calc_account_state_tree_root(
                cs.namespace(|| "new_operator_state_root"),
                &balance_root,
                &self.rescue_params,
            )?
        };
        operator_account_data.extend(validator_account.nonce.get_bits_le());
        operator_account_data.extend(validator_account.pub_key_hash.get_bits_le());
        operator_account_data.extend(validator_account.address.get_bits_le());
        operator_account_data
            .extend(new_operator_state_root.into_padded_le_bits(FR_BIT_WIDTH_PADDED));

        let root_from_operator_after_fees = allocate_merkle_root(
            cs.namespace(|| "root from operator_account after fees"),
            &operator_account_data,
            &validator_address_bits,
            &validator_audit_path,
            self.rescue_params,
        )?;

        let final_root = CircuitElement::from_number(
            cs.namespace(|| "final_root"),
            root_from_operator_after_fees,
        )?;

        {
            // Now it's time to pack the initial SHA256 hash due to Ethereum BE encoding
            // and start rolling the hash

            let mut initial_hash_data: Vec<Boolean> = vec![];

            let block_number = CircuitElement::from_fe_with_known_length(
                cs.namespace(|| "block_number"),
                || self.block_number.grab(),
                params::BLOCK_NUMBER_BIT_WIDTH,
            )?;

            initial_hash_data.extend(block_number.into_padded_be_bits(256));

            initial_hash_data.extend(validator_address_padded.into_padded_be_bits(256));

            assert_eq!(initial_hash_data.len(), 512);

            let mut hash_block = sha256::sha256(
                cs.namespace(|| "initial rolling sha256"),
                &initial_hash_data,
            )?;

            let mut pack_bits = vec![];
            pack_bits.extend(hash_block);
            pack_bits.extend(old_root.into_padded_be_bits(256));

            hash_block = sha256::sha256(cs.namespace(|| "hash old_root"), &pack_bits)?;

            let mut pack_bits = vec![];
            pack_bits.extend(hash_block);
            pack_bits.extend(final_root.into_padded_be_bits(256));

            hash_block = sha256::sha256(cs.namespace(|| "hash with new_root"), &pack_bits)?;

            let mut pack_bits = vec![];
            pack_bits.extend(hash_block);
            pack_bits.extend(block_pub_data_bits.into_iter());

            hash_block = sha256::sha256(cs.namespace(|| "final hash public"), &pack_bits)?;

            // // now pack and enforce equality to the input

            hash_block.reverse();
            hash_block.truncate(E::Fr::CAPACITY as usize);

            let final_hash = pack_bits_to_element(cs.namespace(|| "final_hash"), &hash_block)?;
            cs.enforce(
                || "enforce external data hash equality",
                |lc| lc + public_data_commitment.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + final_hash.get_variable(),
            );
        }
        Ok(())
    }
}
impl<'a, E: RescueEngine + JubjubEngine> FranklinCircuit<'a, E> {
    fn verify_correct_chunking<CS: ConstraintSystem<E>>(
        &self,
        op: &Operation<E>,
        next_chunk_number: &AllocatedNum<E>,
        mut cs: CS,
    ) -> Result<(AllocatedNum<E>, AllocatedChunkData<E>), SynthesisError> {
        let tx_type = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "tx_type"),
            || op.tx_type.grab(),
            params::TX_TYPE_BIT_WIDTH,
        )?;

        let max_chunks_powers = generate_powers(
            cs.namespace(|| "generate powers of max chunks"),
            &tx_type.get_number(),
            DIFFERENT_TRANSACTIONS_TYPE_NUMBER,
        )?;

        let max_chunks_last_coeffs = generate_maxchunk_polynomial::<E>();

        let max_chunk = do_the_lookup(
            cs.namespace(|| "max_chunk"),
            &max_chunks_last_coeffs,
            &max_chunks_powers,
        )?;
        let operation_chunk_number =
            AllocatedNum::alloc(cs.namespace(|| "operation_chunk_number"), || {
                op.chunk.grab()
            })?;

        cs.enforce(
            || "correct_sequence",
            |lc| {
                lc + operation_chunk_number.clone().get_variable()
                    - next_chunk_number.get_variable()
            },
            |lc| lc + CS::one(),
            |lc| lc,
        );
        let is_chunk_last = Boolean::from(Expression::equals(
            cs.namespace(|| "is_chunk_last"),
            &operation_chunk_number,
            &max_chunk,
        )?);

        let is_chunk_first = Boolean::from(Expression::equals(
            cs.namespace(|| "is_chunk_first"),
            &operation_chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let subseq_chunk = Expression::from(&operation_chunk_number) + Expression::u64::<CS>(1);

        let next_chunk_number = Expression::conditionally_select(
            cs.namespace(|| "determine next_chunk_number"),
            Expression::constant::<CS>(E::Fr::zero()),
            subseq_chunk,
            &is_chunk_last,
        )?;

        Ok((
            next_chunk_number,
            AllocatedChunkData {
                chunk_number: operation_chunk_number,
                is_chunk_last,
                tx_type,
                is_chunk_first,
            },
        ))
    }

    fn select_branch<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        first: &AllocatedOperationBranch<E>,
        second: &AllocatedOperationBranch<E>,
        _op: &Operation<E>,
        chunk_data: &AllocatedChunkData<E>,
    ) -> Result<AllocatedOperationBranch<E>, SynthesisError> {
        let deposit_tx_type = Expression::u64::<CS>(1);
        let left_side = Expression::constant::<CS>(E::Fr::zero());

        let cur_side = Expression::select_ifeq(
            cs.namespace(|| "select corresponding branch"),
            &chunk_data.tx_type.get_number(),
            deposit_tx_type,
            left_side.clone(),
            &chunk_data.chunk_number,
        )?;

        let is_left = Boolean::from(Expression::equals(
            cs.namespace(|| "is_left"),
            left_side.clone(),
            &cur_side,
        )?);
        Ok(AllocatedOperationBranch {
            account: AccountContent {
                nonce: CircuitElement::conditionally_select(
                    cs.namespace(|| "chosen_nonce"),
                    &first.account.nonce,
                    &second.account.nonce,
                    &is_left,
                )?,
                pub_key_hash: CircuitElement::conditionally_select(
                    cs.namespace(|| "chosen pubkey"),
                    &first.account.pub_key_hash,
                    &second.account.pub_key_hash,
                    &is_left,
                )?,
                address: CircuitElement::conditionally_select(
                    cs.namespace(|| "chosen address"),
                    &first.account.address,
                    &second.account.address,
                    &is_left,
                )?,
            },
            account_audit_path: select_vec_ifeq(
                cs.namespace(|| "account_audit_path"),
                left_side.clone(),
                &cur_side,
                &first.account_audit_path,
                &second.account_audit_path,
            )?,
            account_id: CircuitElement::conditionally_select(
                cs.namespace(|| "chosen account_address"),
                &first.account_id,
                &second.account_id,
                &is_left,
            )?,
            balance: CircuitElement::conditionally_select(
                cs.namespace(|| "chosen balance"),
                &first.balance,
                &second.balance,
                &is_left,
            )?,
            balance_audit_path: select_vec_ifeq(
                cs.namespace(|| "balance_audit_path"),
                left_side,
                &cur_side,
                &first.balance_audit_path,
                &second.balance_audit_path,
            )?,
            token: CircuitElement::conditionally_select(
                cs.namespace(|| "chosen token"),
                &first.token,
                &second.token,
                &is_left,
            )?,
        })
    }

    fn execute_op<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        mut cur: &mut AllocatedOperationBranch<E>,
        lhs: &AllocatedOperationBranch<E>,
        rhs: &AllocatedOperationBranch<E>,
        op: &Operation<E>,
        chunk_data: &AllocatedChunkData<E>,
        is_account_empty: &Boolean,
        ext_pubdata_chunk: &AllocatedNum<E>,
        // subtree_root: &CircuitElement<E>, // Close disable
        fees: &mut [AllocatedNum<E>],
        prev: &mut PreviousData<E>,
    ) -> Result<(), SynthesisError> {
        cs.enforce(
            || "left and right tokens are equal",
            |lc| lc + lhs.token.get_number().get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + rhs.token.get_number().get_variable(),
        );
        let public_generator = self
            .jubjub_params
            .generator(FixedGenerators::SpendingKeyGenerator)
            .clone();

        let generator = ecc::EdwardsPoint::witness(
            cs.namespace(|| "allocate public generator"),
            Some(public_generator),
            self.jubjub_params,
        )?;

        let op_data =
            AllocatedOperationData::from_witness(cs.namespace(|| "allocated_operation_data"), op)?;
        // ensure op_data is equal to previous
        {
            let mut is_op_data_correct_flags = vec![];
            is_op_data_correct_flags.push(CircuitElement::equals(
                cs.namespace(|| "is a equal to previous"),
                &op_data.a,
                &prev.op_data.a,
            )?);
            is_op_data_correct_flags.push(CircuitElement::equals(
                cs.namespace(|| "is b equal to previous"),
                &op_data.b,
                &prev.op_data.b,
            )?);
            is_op_data_correct_flags.push(CircuitElement::equals(
                cs.namespace(|| "is amount_packed equal to previous"),
                &op_data.amount_packed,
                &prev.op_data.amount_packed,
            )?);
            is_op_data_correct_flags.push(CircuitElement::equals(
                cs.namespace(|| "is fee_packed equal to previous"),
                &op_data.fee_packed,
                &prev.op_data.fee_packed,
            )?);
            is_op_data_correct_flags.push(CircuitElement::equals(
                cs.namespace(|| "is eth_address equal to previous"),
                &op_data.eth_address,
                &prev.op_data.eth_address,
            )?);
            is_op_data_correct_flags.push(CircuitElement::equals(
                cs.namespace(|| "is new_pubkey_hash equal to previous"),
                &op_data.new_pubkey_hash,
                &prev.op_data.new_pubkey_hash,
            )?);
            is_op_data_correct_flags.push(CircuitElement::equals(
                cs.namespace(|| "is full_amount equal to previous"),
                &op_data.full_amount,
                &prev.op_data.full_amount,
            )?);

            let is_op_data_equal_to_previous = multi_and(
                cs.namespace(|| "is_op_data_equal_to_previous"),
                &is_op_data_correct_flags,
            )?;

            let is_op_data_correct = multi_or(
                cs.namespace(|| "is_op_data_correct"),
                &[
                    is_op_data_equal_to_previous,
                    chunk_data.is_chunk_first.clone(),
                ],
            )?;
            Boolean::enforce_equal(
                cs.namespace(|| "ensure op_data is correctly formed"),
                &is_op_data_correct,
                &Boolean::constant(true),
            )?;
        }
        prev.op_data = op_data.clone();

        let signer_key = unpack_point_if_possible(
            cs.namespace(|| "unpack pubkey"),
            &op.signer_pub_key_packed,
            self.rescue_params,
            self.jubjub_params,
        )?;
        let signature_data = verify_circuit_signature(
            cs.namespace(|| "verify circuit signature"),
            &op_data,
            &signer_key,
            op.signature_data.clone(),
            self.rescue_params,
            self.jubjub_params,
            generator,
        )?;

        let diff_a_b =
            Expression::from(&op_data.a.get_number()) - Expression::from(&op_data.b.get_number());

        let diff_a_b_bits = diff_a_b.into_bits_le_fixed(
            cs.namespace(|| "balance-fee bits"),
            params::BALANCE_BIT_WIDTH,
        )?;

        let diff_a_b_bits_repacked = Expression::from_le_bits::<CS>(&diff_a_b_bits);

        let is_a_geq_b = Boolean::from(Expression::equals(
            cs.namespace(|| "is_a_geq_b: diff equal to repacked"),
            diff_a_b,
            diff_a_b_bits_repacked,
        )?);

        let mut op_flags = vec![];
        op_flags.push(self.deposit(
            cs.namespace(|| "deposit"),
            &mut cur,
            &chunk_data,
            &is_account_empty,
            &op_data,
            &ext_pubdata_chunk,
        )?);
        op_flags.push(self.transfer(
            cs.namespace(|| "transfer"),
            &mut cur,
            &lhs,
            &rhs,
            &chunk_data,
            &is_a_geq_b,
            &is_account_empty,
            &op_data,
            &signer_key,
            &ext_pubdata_chunk,
            &signature_data.is_verified,
        )?);
        op_flags.push(self.transfer_to_new(
            cs.namespace(|| "transfer_to_new"),
            &mut cur,
            &lhs,
            &rhs,
            &chunk_data,
            &is_a_geq_b,
            &is_account_empty,
            &op_data,
            &signer_key,
            &ext_pubdata_chunk,
            &signature_data.is_verified,
        )?);
        op_flags.push(self.withdraw(
            cs.namespace(|| "withdraw"),
            &mut cur,
            &chunk_data,
            &is_a_geq_b,
            &op_data,
            &signer_key,
            &ext_pubdata_chunk,
            &signature_data.is_verified,
        )?);
        // Close disable.
        //  op_flags.push(self.close_account(
        //      cs.namespace(|| "close_account"),
        //      &mut cur,
        //      &chunk_data,
        //      &ext_pubdata_chunk,
        //      &op_data,
        //      &signer_key,
        //      &subtree_root,
        //      &signature_data.is_verified,
        //  )?);
        op_flags.push(self.full_exit(
            cs.namespace(|| "full_exit"),
            &mut cur,
            &chunk_data,
            &op_data,
            &ext_pubdata_chunk,
        )?);
        op_flags.push(self.change_pubkey_offchain(
            cs.namespace(|| "change_pubkey_offchain"),
            &mut cur,
            &chunk_data,
            &op_data,
            &ext_pubdata_chunk,
        )?);
        op_flags.push(self.noop(cs.namespace(|| "noop"), &chunk_data, &ext_pubdata_chunk)?);

        let op_valid = multi_or(cs.namespace(|| "op_valid"), &op_flags)?;

        Boolean::enforce_equal(
            cs.namespace(|| "op_valid is true"),
            &op_valid,
            &Boolean::constant(true),
        )?;
        for (i, fee) in fees.iter_mut().enumerate().take(params::total_tokens()) {
            let sum = Expression::from(&*fee) + Expression::from(&op_data.fee.get_number());

            let is_token_correct = Boolean::from(Expression::equals(
                cs.namespace(|| format!("is token equal to number {}", i)),
                &lhs.token.get_number(),
                Expression::constant::<CS>(E::Fr::from_str(&i.to_string()).unwrap()),
            )?);

            let should_update = Boolean::and(
                cs.namespace(|| format!("should update fee number {}", i)),
                &is_token_correct,
                &chunk_data.is_chunk_last.clone(),
            )?;

            *fee = Expression::conditionally_select(
                cs.namespace(|| format!("update fee number {}", i)),
                sum,
                &fee.clone(),
                &should_update,
            )?;
        }

        Ok(())
    }

    fn withdraw<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        is_a_geq_b: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_sig_verified: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        let mut base_valid_flags = vec![];
        //construct pubdata
        let mut pubdata_bits = vec![];

        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
        pubdata_bits.extend(cur.account_id.get_bits_be()); //ACCOUNT_TREE_DEPTH=24
        pubdata_bits.extend(cur.token.get_bits_be()); //TOKEN_BIT_WIDTH=16
        pubdata_bits.extend(op_data.full_amount.get_bits_be()); //AMOUNT_PACKED=24
        pubdata_bits.extend(op_data.fee_packed.get_bits_be()); //FEE_PACKED=8
        pubdata_bits.extend(op_data.eth_address.get_bits_be()); //ETH_ADDRESS=160
                                                                //        assert_eq!(pubdata_bits.len(), 30 * 8);
        pubdata_bits.resize(
            WithdrawOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        // construct signature message

        let mut serialized_tx_bits = vec![];

        serialized_tx_bits.extend(chunk_data.tx_type.get_bits_be());
        serialized_tx_bits.extend(cur.account_id.get_bits_be());
        serialized_tx_bits.extend(cur.account.address.get_bits_be());
        serialized_tx_bits.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits.extend(cur.token.get_bits_be());
        serialized_tx_bits.extend(op_data.full_amount.get_bits_be());
        serialized_tx_bits.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits.extend(cur.account.nonce.get_bits_be());

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            WithdrawOp::CHUNKS,
        )?;

        //TODO: this flag is used too often, we better compute it above
        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_equal"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);
        base_valid_flags.push(is_pubdata_chunk_correct);

        // verify correct tx_code
        let is_withdraw = Boolean::from(Expression::equals(
            cs.namespace(|| "is_withdraw"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(WithdrawOp::OP_CODE)),
        )?);
        base_valid_flags.push(is_withdraw);

        let is_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_serialized_tx_correct"),
            serialized_tx_bits,
            &op_data,
        )?;
        let is_signed_correctly = multi_and(
            cs.namespace(|| "is_signed_correctly"),
            &[is_serialized_tx_correct, is_sig_verified.clone()],
        )?;

        let is_sig_correct = multi_or(
            cs.namespace(|| "sig is valid or not first chunk"),
            &[is_signed_correctly, is_first_chunk.clone().not()],
        )?;
        base_valid_flags.push(is_sig_correct);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correct"),
            &signer_key.pubkey.get_hash(),
            &cur.account.pub_key_hash,
        )?;
        base_valid_flags.push(is_signer_valid);

        // base_valid_flags.push(_is_signer_valid);
        let is_base_valid = multi_and(cs.namespace(|| "valid base withdraw"), &base_valid_flags)?;

        let mut lhs_valid_flags = vec![];
        lhs_valid_flags.push(is_first_chunk.clone());

        lhs_valid_flags.push(is_base_valid.clone());

        // check operation arguments
        let is_a_correct =
            CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;

        lhs_valid_flags.push(is_a_correct);

        let sum_amount_fee = Expression::from(&op_data.full_amount.get_number())
            + Expression::from(&op_data.fee.get_number());

        let is_b_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_b_correct"),
            &op_data.b.get_number(),
            sum_amount_fee.clone(),
        )?);
        lhs_valid_flags.push(is_b_correct);
        lhs_valid_flags.push(is_a_geq_b.clone());

        lhs_valid_flags.push(no_nonce_overflow(
            cs.namespace(|| "no nonce overflow"),
            &cur.account.nonce.get_number(),
        )?);
        debug!("lhs_valid_withdraw_begin");
        let lhs_valid = multi_and(cs.namespace(|| "is_lhs_valid"), &lhs_valid_flags)?;
        debug!("lhs_valid_withdraw_end");

        let mut ohs_valid_flags = vec![];
        ohs_valid_flags.push(is_base_valid);
        ohs_valid_flags.push(is_first_chunk.not());
        let is_ohs_valid = multi_and(cs.namespace(|| "is_ohs_valid"), &ohs_valid_flags)?;

        let tx_valid = multi_or(
            cs.namespace(|| "tx_valid"),
            &[lhs_valid.clone(), is_ohs_valid],
        )?;

        let updated_balance = Expression::from(&cur.balance.get_number()) - sum_amount_fee;

        //mutate current branch if it is first chunk of valid withdraw transaction
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "mutated balance"),
            updated_balance,
            &cur.balance,
            &lhs_valid,
        )?;
        cur.balance
            .enforce_length(cs.namespace(|| "mutated balance is still correct length"))?;

        let updated_nonce =
            Expression::from(&cur.account.nonce.get_number()) + Expression::u64::<CS>(1);

        //update nonce
        cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "update cur nonce"),
            updated_nonce,
            &cur.account.nonce,
            &lhs_valid,
        )?;

        Ok(tx_valid)
    }

    fn full_exit<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        op_data: &AllocatedOperationData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        // Execute first chunk

        //TODO: this flag is used too often, we better compute it above
        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        // MUST be true for all chunks
        let is_pubdata_chunk_correct = {
            //construct pubdata
            let pubdata_bits = {
                let mut pub_data = Vec::new();
                pub_data.extend(chunk_data.tx_type.get_bits_be()); //1
                pub_data.extend(cur.account_id.get_bits_be()); //3
                pub_data.extend(op_data.eth_address.get_bits_be()); //20
                pub_data.extend(cur.token.get_bits_be()); // 2
                pub_data.extend(op_data.full_amount.get_bits_be());
                pub_data.resize(
                    FullExitOp::CHUNKS * params::CHUNK_BIT_WIDTH,
                    Boolean::constant(false),
                );
                pub_data
            };

            let pubdata_chunk = select_pubdata_chunk(
                cs.namespace(|| "select_pubdata_chunk"),
                &pubdata_bits,
                &chunk_data.chunk_number,
                FullExitOp::CHUNKS,
            )?;

            Boolean::from(Expression::equals(
                cs.namespace(|| "is_pubdata_equal"),
                &pubdata_chunk,
                ext_pubdata_chunk,
            )?)
        };

        let is_base_valid = {
            let mut base_valid_flags = Vec::new();

            debug!(
                "is_pubdata_chunk_correct {:?}",
                is_pubdata_chunk_correct.get_value()
            );
            base_valid_flags.push(is_pubdata_chunk_correct);
            // MUST be true
            let is_full_exit = Boolean::from(Expression::equals(
                cs.namespace(|| "is_full_exit"),
                &chunk_data.tx_type.get_number(),
                Expression::u64::<CS>(u64::from(FullExitOp::OP_CODE)), //full_exit tx code
            )?);

            base_valid_flags.push(is_full_exit);
            multi_and(cs.namespace(|| "valid base full_exit"), &base_valid_flags)?
        };

        // SHOULD be true for successful exit
        // otherwise it is impossible to decide from pub data if nonce should be updated
        let is_address_correct = CircuitElement::equals(
            cs.namespace(|| "is_address_correct"),
            &cur.account.address,
            &op_data.eth_address,
        )?;

        // MUST be true for correct op. First chunk is correct and tree update can be executed.
        let first_chunk_valid = {
            let mut flags = Vec::new();
            flags.push(is_first_chunk.clone());
            flags.push(is_base_valid.clone());
            flags.push(no_nonce_overflow(
                cs.namespace(|| "no nonce overflow"),
                &cur.account.nonce.get_number(),
            )?);
            multi_and(cs.namespace(|| "first_chunk_valid"), &flags)?
        };

        // Full exit was a success, update account is the first chunk.
        let success_account_update = multi_and(
            cs.namespace(|| "success_account_update"),
            &[first_chunk_valid.clone(), is_address_correct],
        )?;

        //mutate current branch if it is first chunk of a successful withdraw transaction
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "mutated balance"),
            Expression::constant::<CS>(E::Fr::zero()),
            &cur.balance,
            &success_account_update,
        )?;

        // Check other chunks
        let other_chunks_valid = {
            let mut flags = Vec::new();
            flags.push(is_base_valid);
            flags.push(is_first_chunk.not());
            multi_and(cs.namespace(|| "other_chunks_valid"), &flags)?
        };

        // MUST be true for correct (successful or not) full exit
        let tx_valid = multi_or(
            cs.namespace(|| "tx_valid"),
            &[first_chunk_valid, other_chunks_valid],
        )?;
        Ok(tx_valid)
    }

    fn deposit<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        //construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
        pubdata_bits.extend(cur.account_id.get_bits_be()); //ACCOUNT_TREE_DEPTH=24
        pubdata_bits.extend(cur.token.get_bits_be()); //TOKEN_BIT_WIDTH=16
        pubdata_bits.extend(op_data.full_amount.get_bits_be()); //AMOUNT_PACKED=24
        pubdata_bits.extend(op_data.eth_address.get_bits_be()); //ETH_KEY_BIT_WIDTH=160
        pubdata_bits.resize(
            DepositOp::CHUNKS * params::CHUNK_BIT_WIDTH, //TODO: move to constant
            Boolean::constant(false),
        );

        //useful below
        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let mut is_valid_flags = vec![];

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            DepositOp::CHUNKS,
        )?;

        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_equal"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);
        is_valid_flags.push(is_pubdata_chunk_correct);

        // verify correct tx_code
        let is_deposit = Boolean::from(Expression::equals(
            cs.namespace(|| "is_deposit"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(DepositOp::OP_CODE)),
        )?);
        is_valid_flags.push(is_deposit);

        // verify if address is to previous one (if existed)
        let is_pub_equal_to_previous = CircuitElement::equals(
            cs.namespace(|| "is_address_equal_to_previous"),
            &op_data.eth_address,
            &cur.account.address,
        )?;

        //keys are same or account is empty
        let is_pubkey_correct = Boolean::and(
            cs.namespace(|| "acc not empty and keys are not the same"),
            &is_pub_equal_to_previous.not(),
            &is_account_empty.not(),
        )?
        .not();
        is_valid_flags.push(is_pubkey_correct);

        //verify correct amounts
        let is_a_correct = CircuitElement::equals(
            cs.namespace(|| "a == amount"),
            &op_data.full_amount,
            &op_data.a,
        )?;

        is_valid_flags.push(is_a_correct);

        let tx_valid = multi_and(cs.namespace(|| "is_tx_valid"), &is_valid_flags)?;

        let is_valid_first = Boolean::and(
            cs.namespace(|| "is valid and first"),
            &tx_valid,
            &is_first_chunk,
        )?;

        let updated_balance = Expression::from(&cur.balance.get_number())
            + Expression::from(&op_data.full_amount.get_number());

        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "mutated balance"),
            updated_balance,
            &cur.balance,
            &is_valid_first,
        )?;

        // update pub_key
        cur.account.address = CircuitElement::conditionally_select(
            cs.namespace(|| "mutated_pubkey"),
            &op_data.eth_address,
            &cur.account.address,
            &is_valid_first,
        )?;
        Ok(tx_valid)
    }

    fn change_pubkey_offchain<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        op_data: &AllocatedOperationData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        //construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
        pubdata_bits.extend(cur.account_id.get_bits_be()); //ACCOUNT_TREE_DEPTH=24
        pubdata_bits.extend(op_data.new_pubkey_hash.get_bits_be()); //ETH_KEY_BIT_WIDTH=160
        pubdata_bits.extend(op_data.eth_address.get_bits_be()); //ETH_KEY_BIT_WIDTH=160
                                                                // NOTE: nonce if verified implicitly here. Current account nonce goes to pubdata and to contract.
        pubdata_bits.extend(op_data.pub_nonce.get_bits_be()); //TOKEN_BIT_WIDTH=16
        pubdata_bits.resize(
            ChangePubKeyOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        //useful below
        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let mut is_valid_flags = vec![];

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            ChangePubKeyOp::CHUNKS,
        )?;

        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_equal"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);
        is_valid_flags.push(is_pubdata_chunk_correct);

        // verify correct tx_code
        let is_change_pubkey_offchain = Boolean::from(Expression::equals(
            cs.namespace(|| "is_change_pubkey_offchain"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(ChangePubKeyOp::OP_CODE)),
        )?);
        is_valid_flags.push(is_change_pubkey_offchain);

        // verify if address is to previous one (if existed)
        let is_address_correct = CircuitElement::equals(
            cs.namespace(|| "is_address_correct"),
            &op_data.eth_address,
            &cur.account.address,
        )?;

        is_valid_flags.push(is_address_correct);

        let tx_valid = multi_and(cs.namespace(|| "is_tx_valid"), &is_valid_flags)?;

        let is_pub_nonce_valid = CircuitElement::equals(
            cs.namespace(|| "is_pub_nonce_valid"),
            &cur.account.nonce,
            &op_data.pub_nonce,
        )?;
        let no_nonce_overflow = no_nonce_overflow(
            cs.namespace(|| "no nonce overflow"),
            &cur.account.nonce.get_number(),
        )?;
        let is_valid_first = multi_and(
            cs.namespace(|| "is_valid_first"),
            &[
                tx_valid.clone(),
                is_first_chunk,
                is_pub_nonce_valid,
                no_nonce_overflow,
            ],
        )?;

        // update pub_key
        cur.account.pub_key_hash = CircuitElement::conditionally_select(
            cs.namespace(|| "mutated_pubkey_hash"),
            &op_data.new_pubkey_hash,
            &cur.account.pub_key_hash,
            &is_valid_first,
        )?;

        //update nonce
        cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "update cur nonce"),
            Expression::from(&cur.account.nonce.get_number()) + Expression::u64::<CS>(1),
            &cur.account.nonce,
            &is_valid_first,
        )?;

        Ok(tx_valid)
    }

    // Close disable
    // fn close_account<CS: ConstraintSystem<E>>(
    //     &self,
    //     mut cs: CS,
    //     cur: &mut AllocatedOperationBranch<E>,
    //     chunk_data: &AllocatedChunkData<E>,
    //     ext_pubdata_chunk: &AllocatedNum<E>,
    //     op_data: &AllocatedOperationData<E>,
    //     signer_key: &AllocatedSignerPubkey<E>,
    //     subtree_root: &CircuitElement<E>,
    //     is_sig_verified: &Boolean,
    // ) -> Result<Boolean, SynthesisError> {
    //     let mut is_valid_flags = vec![];
    //     //construct pubdata
    //     let mut pubdata_bits = vec![];
    //     pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
    //     pubdata_bits.extend(cur.account_address.get_bits_be()); //ACCOUNT_TREE_DEPTH=24
    //     pubdata_bits.resize(
    //         params::CHUNK_BIT_WIDTH,
    //         Boolean::constant(false),
    //     );

    //     // construct signature message preimage (serialized_tx)
    //     let mut serialized_tx_bits = vec![];
    //     serialized_tx_bits.extend(chunk_data.tx_type.get_bits_be());
    //     serialized_tx_bits.extend(cur.account.pub_key_hash.get_bits_be());
    //     serialized_tx_bits.extend(cur.account.nonce.get_bits_be());

    //     let pubdata_chunk = select_pubdata_chunk(
    //         cs.namespace(|| "select_pubdata_chunk"),
    //         &pubdata_bits,
    //         &chunk_data.chunk_number,
    //         1,
    //     )?;

    //     let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
    //         cs.namespace(|| "is_pubdata_equal"),
    //         &pubdata_chunk,
    //         ext_pubdata_chunk,
    //     )?);
    //     is_valid_flags.push(is_pubdata_chunk_correct);

    //     let is_close_account = Boolean::from(Expression::equals(
    //         cs.namespace(|| "is_deposit"),
    //         &chunk_data.tx_type.get_number(),
    //         Expression::u64::<CS>(4), //close_account tx_type
    //     )?);
    //     is_valid_flags.push(is_close_account.clone());

    //     let tmp = CircuitAccount::<E>::empty_balances_root_hash();
    //     let mut r_repr = E::Fr::zero().into_repr();
    //     r_repr.read_be(&tmp[..]).unwrap();
    //     let empty_root = E::Fr::from_repr(r_repr).unwrap();

    //     let are_balances_empty = Boolean::from(Expression::equals(
    //         cs.namespace(|| "are_balances_empty"),
    //         &subtree_root.get_number(),
    //         Expression::constant::<CS>(empty_root), //This is precalculated root_hash of subtree with empty balances
    //     )?);
    //     is_valid_flags.push(are_balances_empty);

    //     let is_serialized_tx_correct = verify_signature_message_construction(
    //         cs.namespace(|| "is_serialized_tx_correct"),
    //         serialized_tx_bits,
    //         &op_data,
    //     )?;

    //     is_valid_flags.push(is_serialized_tx_correct);
    //     is_valid_flags.push(is_sig_verified.clone());
    //     let is_signer_valid = CircuitElement::equals(
    //         cs.namespace(|| "signer_key_correct"),
    //         &signer_key.pubkey.get_hash(),
    //         &cur.account.pub_key_hash, //earlier we ensured that this new_pubkey_hash is equal to current if existed
    //     )?;

    //     is_valid_flags.push(is_signer_valid);

    //     let tx_valid = multi_and(cs.namespace(|| "is_tx_valid"), &is_valid_flags)?;

    //     // below we conditionally update state if it is valid operation

    //     // update pub_key
    //     cur.account.pub_key_hash = CircuitElement::conditionally_select_with_number_strict(
    //         cs.namespace(|| "mutated_pubkey"),
    //         Expression::constant::<CS>(E::Fr::zero()),
    //         &cur.account.pub_key_hash,
    //         &tx_valid,
    //     )?;
    //     // update nonce
    //     cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
    //         cs.namespace(|| "update cur nonce"),
    //         Expression::constant::<CS>(E::Fr::zero()),
    //         &cur.account.nonce,
    //         &tx_valid,
    //     )?;

    //     Ok(tx_valid)
    // }

    fn noop<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        chunk_data: &AllocatedChunkData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        let mut is_valid_flags = vec![];
        //construct pubdata (it's all 0 for noop)
        let mut pubdata_bits = vec![];
        pubdata_bits.resize(params::CHUNK_BIT_WIDTH, Boolean::constant(false));

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            1,
        )?;

        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_equal"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);
        is_valid_flags.push(is_pubdata_chunk_correct);

        let is_noop = Boolean::from(Expression::equals(
            cs.namespace(|| "is_noop"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(0), //noop tx_type
        )?);
        is_valid_flags.push(is_noop);

        let tx_valid = multi_and(cs.namespace(|| "is_tx_valid"), &is_valid_flags)?;

        Ok(tx_valid)
    }

    fn transfer_to_new<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        lhs: &AllocatedOperationBranch<E>,
        rhs: &AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        is_a_geq_b: &Boolean,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_sig_verified: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //8
        pubdata_bits.extend(lhs.account_id.get_bits_be()); //24
        pubdata_bits.extend(cur.token.get_bits_be()); //16
        pubdata_bits.extend(op_data.amount_packed.get_bits_be()); //24
        pubdata_bits.extend(op_data.eth_address.get_bits_be()); //160
        pubdata_bits.extend(rhs.account_id.get_bits_be()); //24
        pubdata_bits.extend(op_data.fee_packed.get_bits_be()); //8
        pubdata_bits.resize(
            TransferToNewOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        // construct signature message preimage (serialized_tx)
        let mut serialized_tx_bits = vec![];
        let tx_code = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "transfer_to_new_code_ce"),
            || Ok(E::Fr::from_str(&TransferOp::OP_CODE.to_string()).unwrap()),
            8,
        )?; //we use here transfer tx_code to allow user sign message without knowing whether it is transfer_to_new or transfer
        serialized_tx_bits.extend(tx_code.get_bits_be());
        serialized_tx_bits.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits.extend(lhs.account.address.get_bits_be());
        serialized_tx_bits.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits.extend(cur.token.get_bits_be());
        serialized_tx_bits.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits.extend(cur.account.nonce.get_bits_be());

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            TransferToNewOp::CHUNKS,
        )?;
        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_correct"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);

        let mut lhs_valid_flags = vec![];
        lhs_valid_flags.push(is_pubdata_chunk_correct.clone());

        let is_transfer = Boolean::from(Expression::equals(
            cs.namespace(|| "is_transfer"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(TransferToNewOp::OP_CODE)),
        )?);
        lhs_valid_flags.push(is_transfer.clone());

        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);
        lhs_valid_flags.push(is_first_chunk.clone());

        let is_a_correct =
            CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;
        lhs_valid_flags.push(is_a_correct);

        let sum_amount_fee = Expression::from(&op_data.amount_unpacked.get_number())
            + Expression::from(&op_data.fee.get_number());
        let is_b_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_b_correct"),
            &op_data.b.get_number(),
            sum_amount_fee.clone(),
        )?);

        lhs_valid_flags.push(is_b_correct);
        lhs_valid_flags.push(is_a_geq_b.clone());

        lhs_valid_flags.push(no_nonce_overflow(
            cs.namespace(|| "no nonce overflow"),
            &cur.account.nonce.get_number(),
        )?);

        let is_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_serialized_tx_correct"),
            serialized_tx_bits,
            &op_data,
        )?;
        debug!(
            "is_serialized_tx_correct: {:?}",
            is_serialized_tx_correct.get_value()
        );
        let is_signed_correctly = multi_and(
            cs.namespace(|| "is_signed_correctly"),
            &[is_serialized_tx_correct, is_sig_verified.clone()],
        )?;

        debug!("is_sig_verified: {:?}", is_sig_verified.get_value());

        let is_sig_correct = multi_or(
            cs.namespace(|| "sig is valid or not first chunk"),
            &[is_signed_correctly, is_first_chunk.clone().not()],
        )?;
        lhs_valid_flags.push(is_sig_correct);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correect"),
            &signer_key.pubkey.get_hash(),
            &lhs.account.pub_key_hash,
        )?;
        debug!(
            "signer_key.pubkey.get_hash(): {:?}",
            signer_key.pubkey.get_hash().get_number().get_value()
        );
        debug!(
            "signer_key.pubkey.get_x(): {:?}",
            signer_key.pubkey.get_x().get_number().get_value()
        );

        debug!(
            "signer_key.pubkey.get_y(): {:?}",
            signer_key.pubkey.get_y().get_number().get_value()
        );

        debug!(
            "lhs.account.pub_key_hash: {:?}",
            lhs.account.pub_key_hash.get_number().get_value()
        );
        debug!("is_signer_valid: {:?}", is_signer_valid.get_value());

        lhs_valid_flags.push(is_signer_valid);
        let lhs_valid = multi_and(cs.namespace(|| "lhs_valid"), &lhs_valid_flags)?;
        let updated_balance_value = Expression::from(&cur.balance.get_number()) - sum_amount_fee;

        let updated_nonce =
            Expression::from(&cur.account.nonce.get_number()) + Expression::u64::<CS>(1);

        //update cur values if lhs is valid
        //update nonce
        cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "update cur nonce"),
            updated_nonce,
            &cur.account.nonce,
            &lhs_valid,
        )?;

        //update balance
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance"),
            updated_balance_value,
            &cur.balance,
            &lhs_valid,
        )?;

        let mut rhs_valid_flags = vec![];

        let is_second_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_second_chunk"),
            &chunk_data.chunk_number,
            Expression::u64::<CS>(1),
        )?);
        rhs_valid_flags.push(is_pubdata_chunk_correct.clone());
        rhs_valid_flags.push(is_second_chunk.clone());
        rhs_valid_flags.push(is_transfer.clone());
        rhs_valid_flags.push(is_account_empty.clone());
        let rhs_valid = multi_and(cs.namespace(|| "rhs_valid"), &rhs_valid_flags)?;

        cur.balance = CircuitElement::conditionally_select(
            cs.namespace(|| "mutated balance"),
            &op_data.amount_unpacked,
            &cur.balance,
            &rhs_valid,
        )?;
        cur.balance
            .enforce_length(cs.namespace(|| "mutated balance is still correct length"))?; // TODO: this is actually redundant, cause they are both enforced to be of appropriate length

        cur.account.address = CircuitElement::conditionally_select(
            cs.namespace(|| "mutated_pubkey"),
            &op_data.eth_address,
            &cur.account.address,
            &rhs_valid,
        )?;

        let mut ohs_valid_flags = vec![];
        ohs_valid_flags.push(is_pubdata_chunk_correct);
        ohs_valid_flags.push(is_first_chunk.not());
        ohs_valid_flags.push(is_second_chunk.not());
        ohs_valid_flags.push(is_transfer);

        let is_ohs_valid = multi_and(cs.namespace(|| "is_ohs_valid"), &ohs_valid_flags)?;

        let is_op_valid = multi_or(
            cs.namespace(|| "is_op_valid"),
            &[is_ohs_valid, lhs_valid, rhs_valid],
        )?;
        Ok(is_op_valid)
    }

    fn transfer<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        lhs: &AllocatedOperationBranch<E>,
        rhs: &AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        is_a_geq_b: &Boolean,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_sig_verified: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be());
        pubdata_bits.extend(lhs.account_id.get_bits_be());
        pubdata_bits.extend(cur.token.get_bits_be());
        pubdata_bits.extend(rhs.account_id.get_bits_be());
        pubdata_bits.extend(op_data.amount_packed.get_bits_be());
        pubdata_bits.extend(op_data.fee_packed.get_bits_be());

        pubdata_bits.resize(
            TransferOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        // construct signature message preimage (serialized_tx)

        let mut serialized_tx_bits = vec![];

        serialized_tx_bits.extend(chunk_data.tx_type.get_bits_be());
        serialized_tx_bits.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits.extend(lhs.account.address.get_bits_be());
        serialized_tx_bits.extend(rhs.account.address.get_bits_be());
        serialized_tx_bits.extend(cur.token.get_bits_be());
        serialized_tx_bits.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits.extend(cur.account.nonce.get_bits_be());

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            TransferOp::CHUNKS,
        )?;
        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_correct"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);

        // verify correct tx_code

        let is_transfer = Boolean::from(Expression::equals(
            cs.namespace(|| "is_transfer"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(TransferOp::OP_CODE)), // transfer tx_type
        )?);

        let mut lhs_valid_flags = vec![];

        lhs_valid_flags.push(is_pubdata_chunk_correct.clone());
        lhs_valid_flags.push(is_transfer.clone());

        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);
        lhs_valid_flags.push(is_first_chunk);

        // check operation arguments
        let is_a_correct =
            CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;

        lhs_valid_flags.push(is_a_correct);

        let sum_amount_fee = Expression::from(&op_data.amount_unpacked.get_number())
            + Expression::from(&op_data.fee.get_number());

        let is_b_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_b_correct"),
            &op_data.b.get_number(),
            sum_amount_fee.clone(),
        )?);

        lhs_valid_flags.push(is_b_correct);
        lhs_valid_flags.push(is_a_geq_b.clone());
        lhs_valid_flags.push(is_sig_verified.clone());
        lhs_valid_flags.push(no_nonce_overflow(
            cs.namespace(|| "no nonce overflow"),
            &cur.account.nonce.get_number(),
        )?);

        let is_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_serialized_tx_correct"),
            serialized_tx_bits,
            &op_data,
        )?;
        lhs_valid_flags.push(is_serialized_tx_correct);

        // TODO: add flag for is account address is correct(!)
        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correct"),
            &signer_key.pubkey.get_hash(),
            &lhs.account.pub_key_hash,
        )?;
        lhs_valid_flags.push(is_signer_valid);

        // lhs_valid_flags.push(_is_signer_valid);

        let lhs_valid = multi_and(cs.namespace(|| "lhs_valid"), &lhs_valid_flags)?;

        let updated_balance = Expression::from(&cur.balance.get_number()) - sum_amount_fee;

        let updated_nonce =
            Expression::from(&cur.account.nonce.get_number()) + Expression::u64::<CS>(1);

        //update cur values if lhs is valid
        //update nonce
        cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "update cur nonce"),
            updated_nonce,
            &cur.account.nonce,
            &lhs_valid,
        )?;

        //update balance
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance"),
            updated_balance,
            &cur.balance,
            &lhs_valid,
        )?;

        // rhs
        let mut rhs_valid_flags = vec![];
        rhs_valid_flags.push(is_transfer);

        let is_chunk_second = Boolean::from(Expression::equals(
            cs.namespace(|| "is_chunk_second"),
            &chunk_data.chunk_number,
            Expression::u64::<CS>(1),
        )?);
        rhs_valid_flags.push(is_chunk_second);
        rhs_valid_flags.push(is_account_empty.not());

        rhs_valid_flags.push(is_pubdata_chunk_correct);
        let is_rhs_valid = multi_and(cs.namespace(|| "is_rhs_valid"), &rhs_valid_flags)?;

        // calculate new rhs balance value
        let updated_balance = Expression::from(&cur.balance.get_number())
            + Expression::from(&op_data.amount_unpacked.get_number());

        //update balance
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated_balance rhs"),
            updated_balance,
            &cur.balance,
            &is_rhs_valid,
        )?;

        Ok(Boolean::and(
            cs.namespace(|| "lhs_valid nand rhs_valid"),
            &lhs_valid.not(),
            &is_rhs_valid.not(),
        )?
        .not())
    }
}

pub fn check_account_data<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    cur: &AllocatedOperationBranch<E>,
    params: &E::Params,
) -> Result<(AllocatedNum<E>, Boolean, CircuitElement<E>), SynthesisError> {
    //first we prove calculate root of the subtree to obtain account_leaf_data:
    let (cur_account_leaf_bits, is_account_empty, subtree_root) = allocate_account_leaf_bits(
        cs.namespace(|| "allocate current_account_leaf_hash"),
        cur,
        params,
    )?;
    Ok((
        allocate_merkle_root(
            cs.namespace(|| "account_merkle_root"),
            &cur_account_leaf_bits,
            &cur.account_id.get_bits_le(),
            &cur.account_audit_path,
            params,
        )?,
        is_account_empty,
        subtree_root,
    ))
}

/// Account tree state will be extended in the future, so for current balance tree we
/// append emtpy hash to reserve place for the future tree before hashing.
pub fn calc_account_state_tree_root<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    balance_root: &CircuitElement<E>,
    params: &E::Params,
) -> Result<CircuitElement<E>, SynthesisError> {
    let state_tree_root_input = balance_root.get_number();
    let empty_root_padding =
        AllocatedNum::zero(cs.namespace(|| "allocate zero element for padding"))?;

    let mut sponge_output = rescue::rescue_hash(
        cs.namespace(|| "hash state root and balance root"),
        &[state_tree_root_input, empty_root_padding],
        params,
    )?;

    assert_eq!(sponge_output.len(), 1);
    let state_tree_root = sponge_output.pop().expect("must get a single element");

    CircuitElement::from_number(cs.namespace(|| "total_subtree_root_ce"), state_tree_root)
}

pub fn allocate_account_leaf_bits<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    branch: &AllocatedOperationBranch<E>,
    params: &E::Params,
) -> Result<(Vec<Boolean>, Boolean, CircuitElement<E>), SynthesisError> {
    //first we prove calculate root of the subtree to obtain account_leaf_data:

    let balance_data = &branch.balance.get_bits_le();
    let balance_root = allocate_merkle_root(
        cs.namespace(|| "balance_subtree_root"),
        balance_data,
        &branch.token.get_bits_le(),
        &branch.balance_audit_path,
        params,
    )?;

    let mut account_data = vec![];
    account_data.extend(branch.account.nonce.get_bits_le());
    account_data.extend(branch.account.pub_key_hash.get_bits_le());
    account_data.extend(branch.account.address.get_bits_le());

    let account_data_packed =
        pack_bits_to_element(cs.namespace(|| "account_data_packed"), &account_data)?;

    let is_account_empty = Expression::equals(
        cs.namespace(|| "is_account_empty"),
        &account_data_packed,
        Expression::constant::<CS>(E::Fr::zero()),
    )?;
    let balance_subtree_root =
        CircuitElement::from_number(cs.namespace(|| "balance_subtree_root_ce"), balance_root)?;
    let state_tree_root = calc_account_state_tree_root(
        cs.namespace(|| "state_tree_root"),
        &balance_subtree_root,
        params,
    )?;

    // this is safe and just allows the convention. TODO: may be cut to Fr width only?
    account_data.extend(state_tree_root.into_padded_le_bits(params::FR_BIT_WIDTH_PADDED)); // !!!!!

    Ok((
        account_data,
        Boolean::from(is_account_empty),
        balance_subtree_root,
    ))
}

pub fn allocate_merkle_root<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    leaf_bits: &[Boolean],
    index: &[Boolean],
    audit_path: &[AllocatedNum<E>],
    params: &E::Params,
) -> Result<AllocatedNum<E>, SynthesisError> {
    // only first bits of index are considered valuable
    assert!(index.len() >= audit_path.len());
    let index = &index[0..audit_path.len()];

    let leaf_packed = multipack::pack_into_witness(
        cs.namespace(|| "pack leaf bits into field elements"),
        &leaf_bits,
    )?;

    let mut account_leaf_hash = rescue::rescue_hash(
        cs.namespace(|| "account leaf content hash"),
        &leaf_packed,
        params,
    )?;

    assert_eq!(account_leaf_hash.len(), 1);

    let mut cur_hash = account_leaf_hash.pop().expect("must get a single element");

    // Ascend the merkle tree authentication path
    for (i, direction_bit) in index.iter().enumerate() {
        let cs = &mut cs.namespace(|| format!("from merkle tree hash {}", i));

        // "direction_bit" determines if the current subtree
        // is the "right" leaf at this depth of the tree.

        // Witness the authentication path element adjacent
        // at this depth.
        let path_element = &audit_path[i];

        // Swap the two if the current subtree is on the right
        let (xl, xr) = AllocatedNum::conditionally_reverse(
            cs.namespace(|| "conditional reversal of preimage"),
            &cur_hash,
            path_element,
            &direction_bit,
        )?;

        // we do not use any personalization here cause
        // our tree is of a fixed height and hash function
        // is resistant to padding attacks
        let mut sponge_output = rescue::rescue_hash(
            cs.namespace(|| format!("hash tree level {}", i)),
            &[xl, xr],
            params,
        )?;

        assert_eq!(sponge_output.len(), 1);
        cur_hash = sponge_output.pop().expect("must get a single element");
    }

    Ok(cur_hash)
}

fn select_vec_ifeq<
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
    EX1: Into<Expression<E>>,
    EX2: Into<Expression<E>>,
>(
    mut cs: CS,
    a: EX1,
    b: EX2,
    x: &[AllocatedNum<E>],
    y: &[AllocatedNum<E>],
) -> Result<Vec<AllocatedNum<E>>, SynthesisError> {
    assert_eq!(x.len(), y.len());
    let a: Expression<E> = a.into();
    let b: Expression<E> = b.into();
    let mut resulting_vector = vec![];
    for (i, (t_x, t_y)) in x.iter().zip(y.iter()).enumerate() {
        let temp = Expression::select_ifeq(
            cs.namespace(|| format!("iteration {}", i)),
            a.clone(),
            b.clone(),
            t_x,
            t_y,
        )?;
        resulting_vector.push(temp);
    }
    Ok(resulting_vector)
}

fn select_pubdata_chunk<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    pubdata_bits: &[Boolean],
    chunk_number: &AllocatedNum<E>,
    total_chunks: usize,
) -> Result<AllocatedNum<E>, SynthesisError> {
    assert_eq!(pubdata_bits.len(), total_chunks * params::CHUNK_BIT_WIDTH);
    let mut result =
        AllocatedNum::alloc(
            cs.namespace(|| "result pubdata chunk"),
            || Ok(E::Fr::zero()),
        )?;

    for i in 0..total_chunks {
        let cs = &mut cs.namespace(|| format!("chunk number {}", i));
        let pub_chunk_bits =
            pubdata_bits[i * params::CHUNK_BIT_WIDTH..(i + 1) * params::CHUNK_BIT_WIDTH].to_vec();
        let current_chunk =
            pack_bits_to_element(cs.namespace(|| "chunk as field element"), &pub_chunk_bits)?;

        result = Expression::select_ifeq(
            cs.namespace(|| "select if correct chunk number"),
            Expression::u64::<CS>(i as u64),
            chunk_number,
            &current_chunk,
            &result,
        )?;
    }

    Ok(result)
}

fn multi_or<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    x: &[Boolean],
) -> Result<Boolean, SynthesisError> {
    let mut result = Boolean::constant(false);

    for (i, bool_x) in x.iter().enumerate() {
        result = Boolean::and(
            cs.namespace(|| format!("multi or iteration number: {}", i)),
            &result.not(),
            &bool_x.not(),
        )?
        .not();
    }

    Ok(result)
}

//TODO: we can use fees: &[Expression<E>] if needed, though no real need
fn calculate_root_from_full_representation_fees<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    fees: &[AllocatedNum<E>],
    params: &E::Params,
) -> Result<AllocatedNum<E>, SynthesisError> {
    assert_eq!(fees.len(), params::total_tokens());
    let mut fee_hashes = vec![];
    for (index, fee) in fees.iter().cloned().enumerate() {
        let cs = &mut cs.namespace(|| format!("fee hashing index number {}", index));

        fee.limit_number_of_bits(
            cs.namespace(|| "ensure that fees are short enough"),
            params::BALANCE_BIT_WIDTH,
        )?;

        let mut sponge_output =
            rescue::rescue_hash(cs.namespace(|| "hash the fee leaf content"), &[fee], params)?;

        assert_eq!(sponge_output.len(), 1);

        let tmp = sponge_output.pop().expect("must get a single element");

        fee_hashes.push(tmp);
    }
    let mut hash_vec = fee_hashes;

    for i in 0..params::balance_tree_depth() {
        let cs = &mut cs.namespace(|| format!("merkle tree level index number {}", i));
        let chunks = hash_vec.chunks(2);
        let mut new_hashes = vec![];
        for (chunk_number, x) in chunks.enumerate() {
            let cs = &mut cs.namespace(|| format!("chunk number {}", chunk_number));

            let mut sponge_output = rescue::rescue_hash(cs, &x, params)?;

            assert_eq!(sponge_output.len(), 1);

            let tmp = sponge_output.pop().expect("must get a single element");

            new_hashes.push(tmp);
        }
        hash_vec = new_hashes;
    }
    assert_eq!(hash_vec.len(), 1);
    Ok(hash_vec[0].clone())
}

fn generate_maxchunk_polynomial<E: JubjubEngine>() -> Vec<E::Fr> {
    use crate::franklin_crypto::interpolation::interpolate;

    let get_xy = |op_type: u8, op_chunks: usize| {
        let x = E::Fr::from_str(&op_type.to_string()).unwrap();
        let y = E::Fr::from_str(&(op_chunks - 1).to_string()).unwrap();
        (x, y)
    };

    let mut points: Vec<(E::Fr, E::Fr)> = vec![];

    points.push(get_xy(NoopOp::OP_CODE, NoopOp::CHUNKS));
    points.push(get_xy(CloseOp::OP_CODE, CloseOp::CHUNKS));
    points.push(get_xy(TransferOp::OP_CODE, TransferOp::CHUNKS));
    points.push(get_xy(DepositOp::OP_CODE, DepositOp::CHUNKS));
    points.push(get_xy(WithdrawOp::OP_CODE, WithdrawOp::CHUNKS));
    points.push(get_xy(TransferToNewOp::OP_CODE, TransferToNewOp::CHUNKS));
    points.push(get_xy(FullExitOp::OP_CODE, FullExitOp::CHUNKS));
    points.push(get_xy(ChangePubKeyOp::OP_CODE, ChangePubKeyOp::CHUNKS));

    let interpolation = interpolate::<E>(&points[..]).expect("must interpolate");
    assert_eq!(interpolation.len(), DIFFERENT_TRANSACTIONS_TYPE_NUMBER);

    interpolation
}

fn no_nonce_overflow<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    nonce: &AllocatedNum<E>,
) -> Result<Boolean, SynthesisError> {
    Ok(Boolean::from(Expression::equals(
        cs.namespace(|| "is nonce at max"),
        nonce,
        Expression::constant::<CS>(E::Fr::from_str(&std::u32::MAX.to_string()).unwrap()),
    )?)
    .not())
}
