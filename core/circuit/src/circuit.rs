use crate::account::AccountContent;
use crate::account::AccountWitness;
use crate::allocated_structures::*;
use crate::element::{CircuitElement, CircuitPubkey};
use crate::operation::Operation;
use crate::utils::{allocate_audit_path, allocate_sum, pack_bits_to_element};
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField, PrimeFieldRepr};
use franklin_crypto::circuit::baby_eddsa::EddsaSignature;
use franklin_crypto::circuit::boolean::Boolean;
use franklin_crypto::circuit::ecc;
use franklin_crypto::circuit::sha256;

use franklin_crypto::circuit::expression::Expression;
use franklin_crypto::circuit::num::AllocatedNum;
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::circuit::polynomial_lookup::{do_the_lookup, generate_powers};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};
use models::circuit::account::CircuitAccount;
use models::params as franklin_constants;

const DIFFERENT_TRANSACTIONS_TYPE_NUMBER: usize = 6;
#[derive(Clone)]
pub struct FranklinCircuit<'a, E: JubjubEngine> {
    pub params: &'a E::Params,
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

struct PreviousData<E: JubjubEngine> {
    op_data: AllocatedOperationData<E>,
}

// Implementation of our circuit:
impl<'a, E: JubjubEngine> Circuit<E> for FranklinCircuit<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        //this is needed for technical purposes and convenience only
        let zero = AllocatedNum::alloc(cs.namespace(|| "zero"), || Ok(E::Fr::zero()))?;
        zero.assert_zero(cs.namespace(|| "zero==0"))?;

        // we only need this for consistency of first operation
        let zero_circuit_element = CircuitElement::from_number(
            cs.namespace(|| "zero_circuit_element"),
            zero.clone(),
            franklin_constants::FR_BIT_WIDTH,
        )?;
        let mut prev = PreviousData {
            op_data: AllocatedOperationData {
                ethereum_key: zero_circuit_element.clone(),
                new_pubkey_hash: zero_circuit_element.clone(),
                signer_pubkey: CircuitPubkey::from_xy(
                    cs.namespace(|| "dummy signer_pubkey"),
                    zero.clone(),
                    zero.clone(),
                    &self.params,
                )?,
                amount_packed: zero_circuit_element.clone(),
                fee_packed: zero_circuit_element.clone(),
                fee: zero_circuit_element.clone(),
                amount: zero_circuit_element.clone(),
                first_sig_msg: zero_circuit_element.clone(),
                second_sig_msg: zero_circuit_element.clone(),
                a: zero_circuit_element.clone(),
                b: zero_circuit_element.clone(),
            },
        };
        // this is only public input to our circuit
        let public_data_commitment =
            AllocatedNum::alloc(cs.namespace(|| "public_data_commitment"), || {
                self.pub_data_commitment.grab()
            })?;
        public_data_commitment.inputize(cs.namespace(|| "inputize pub_data"))?;
        let validator_address_padded =
            CircuitElement::from_fe_padded(cs.namespace(|| "validator_address"), || {
                self.validator_address.grab()
            })?;
        let mut validator_address = validator_address_padded.get_bits_le();
        validator_address.truncate(franklin_constants::ACCOUNT_TREE_DEPTH);
        let mut validator_balances = allocate_audit_path(
            cs.namespace(|| "validator_balances"),
            &self.validator_balances,
        )?;
        assert_eq!(
            validator_balances.len(),
            (1 << franklin_constants::BALANCE_TREE_DEPTH) as usize
        );

        let validator_audit_path = allocate_audit_path(
            cs.namespace(|| "validator_audit_path"),
            &self.validator_audit_path,
        )?;
        assert_eq!(
            validator_audit_path.len(),
            franklin_constants::ACCOUNT_TREE_DEPTH as usize
        );

        let validator_account = AccountContent::from_witness(
            cs.namespace(|| "validator account"),
            &self.validator_account,
        )?;
        let mut rolling_root =
            AllocatedNum::alloc(cs.namespace(|| "rolling_root"), || self.old_root.grab())?;

        let old_root =
            CircuitElement::from_number_padded(cs.namespace(|| "old_root"), rolling_root.clone())?;
        // first chunk of block should always have number 0
        let mut next_chunk_number = zero.clone();

        // declare vector of fees, that will be collected during block processing
        let mut fees = vec![];
        let fees_len = 1 << franklin_constants::BALANCE_TREE_DEPTH;
        for _ in 0..fees_len {
            fees.push(zero.clone());
        }
        // vector of pub_data_bits that will be aggregated during block processing
        let mut block_pub_data_bits = vec![];

        let mut allocated_chunk_data: AllocatedChunkData<E> = AllocatedChunkData {
            is_chunk_last: Boolean::constant(false),
            chunk_number: zero.clone(),
            tx_type: zero_circuit_element,
        };
        for (i, operation) in self.operations.iter().enumerate() {
            println!("\n operation number {} processing started \n", i);
            let cs = &mut cs.namespace(|| format!("chunk number {}", i));

            let (next_chunk, chunk_data) = self.verify_correct_chunking(
                &operation,
                &next_chunk_number,
                cs.namespace(|| "verify_correct_chunking"),
            )?;

            allocated_chunk_data = chunk_data;
            next_chunk_number = next_chunk;
            let operation_pub_data_chunk = CircuitElement::from_fe_strict(
                cs.namespace(|| "operation_pub_data_chunk"),
                || operation.clone().pubdata_chunk.grab(),
                franklin_constants::CHUNK_BIT_WIDTH,
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
            let (state_root, is_account_empty, subtree_root) = self
                .check_account_data(cs.namespace(|| "calculate account root"), &current_branch)?;

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
                &subtree_root,
                &mut fees,
                &mut prev,
            )?;
            let (new_state_root, _, _) = self.check_account_data(
                cs.namespace(|| "calculate new account root"),
                &current_branch,
            )?;
            let operation_new_root =
                AllocatedNum::alloc(cs.namespace(|| "op_new_root"), || operation.new_root.grab())?;

            // ensure that root hash of the state is correct after applying operation
            cs.enforce(
                || "new root is correct",
                |lc| lc + new_state_root.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + operation_new_root.get_variable(),
            );
            rolling_root = new_state_root;
        }

        cs.enforce(
            || "ensure op_data correct",
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
            self.params,
        )?;

        let mut operator_account_data = vec![];
        let mut old_operator_balance_root_bits = old_operator_balance_root
            .into_bits_le(cs.namespace(|| "old_operator_balance_root_bits"))?;
        old_operator_balance_root_bits.resize(
            franklin_constants::FR_BIT_WIDTH_PADDED,
            Boolean::constant(false),
        );
        operator_account_data.extend(validator_account.nonce.get_bits_le());
        operator_account_data.extend(validator_account.pub_key_hash.get_bits_le());
        operator_account_data.extend(old_operator_balance_root_bits);

        let root_from_operator = allocate_merkle_root(
            cs.namespace(|| "root from operator_account"),
            &operator_account_data,
            &validator_address,
            &validator_audit_path,
            self.params,
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
            self.params,
        )?;

        let mut operator_account_data = vec![];
        let mut new_operator_balance_root_bits = new_operator_balance_root
            .into_bits_le(cs.namespace(|| "new_operator_balance_root_bits"))?;
        new_operator_balance_root_bits.resize(
            franklin_constants::FR_BIT_WIDTH_PADDED,
            Boolean::constant(false),
        );

        operator_account_data.extend(validator_account.nonce.get_bits_le());
        operator_account_data.extend(validator_account.pub_key_hash.get_bits_le());
        operator_account_data.extend(new_operator_balance_root_bits);

        let root_from_operator_after_fees = allocate_merkle_root(
            cs.namespace(|| "root from operator_account after fees"),
            &operator_account_data,
            &validator_address,
            &validator_audit_path,
            self.params,
        )?;

        let final_root = CircuitElement::from_number_padded(
            cs.namespace(|| "final_root"),
            root_from_operator_after_fees.clone(),
        )?;
        {
            // Now it's time to pack the initial SHA256 hash due to Ethereum BE encoding
            // and start rolling the hash

            let mut initial_hash_data: Vec<Boolean> = vec![];

            let block_number =
                CircuitElement::from_fe_padded(cs.namespace(|| "block_number"), || {
                    self.block_number.grab()
                })?;

            initial_hash_data.extend(block_number.get_bits_be());

            initial_hash_data.extend(validator_address_padded.get_bits_be());

            assert_eq!(initial_hash_data.len(), 512);

            let mut hash_block = sha256::sha256(
                cs.namespace(|| "initial rolling sha256"),
                &initial_hash_data,
            )?;

            let mut pack_bits = vec![];
            pack_bits.extend(hash_block);
            pack_bits.extend(old_root.get_bits_be());

            hash_block = sha256::sha256(cs.namespace(|| "hash old_root"), &pack_bits)?;

            let mut pack_bits = vec![];
            pack_bits.extend(hash_block);
            pack_bits.extend(final_root.get_bits_be());

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
impl<'a, E: JubjubEngine> FranklinCircuit<'a, E> {
    fn verify_correct_chunking<CS: ConstraintSystem<E>>(
        &self,
        op: &Operation<E>,
        next_chunk_number: &AllocatedNum<E>,
        mut cs: CS,
    ) -> Result<(AllocatedNum<E>, AllocatedChunkData<E>), SynthesisError> {
        let tx_type = CircuitElement::from_fe_strict(
            cs.namespace(|| "tx_type"),
            || op.tx_type.grab(),
            franklin_constants::TX_TYPE_BIT_WIDTH,
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
            },
            account_audit_path: select_vec_ifeq(
                cs.namespace(|| "account_audit_path"),
                left_side.clone(),
                &cur_side,
                &first.account_audit_path,
                &second.account_audit_path,
            )?,
            account_address: CircuitElement::conditionally_select(
                cs.namespace(|| "chosen account_address"),
                &first.account_address,
                &second.account_address,
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

    fn check_account_data<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &AllocatedOperationBranch<E>,
    ) -> Result<(AllocatedNum<E>, Boolean, CircuitElement<E>), SynthesisError> {
        //first we prove calculate root of the subtree to obtain account_leaf_data:
        let (cur_account_leaf_bits, is_account_empty, subtree_root) = self
            .allocate_account_leaf_bits(
                cs.namespace(|| "allocate current_account_leaf_hash"),
                cur,
            )?;
        Ok((
            allocate_merkle_root(
                cs.namespace(|| "account_merkle_root"),
                &cur_account_leaf_bits,
                &cur.account_address.get_bits_le(),
                &cur.account_audit_path,
                self.params,
            )?,
            is_account_empty,
            subtree_root,
        ))
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
        subtree_root: &CircuitElement<E>,
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
            .params
            .generator(FixedGenerators::SpendingKeyGenerator)
            .clone();
        let generator = ecc::EdwardsPoint::witness(
            cs.namespace(|| "allocate public generator"),
            Some(public_generator),
            self.params,
        )?;

        let op_data = AllocatedOperationData::from_witness(
            cs.namespace(|| "allocated_operation_data"),
            op,
            self.params,
        )?;
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
                cs.namespace(|| "is ethereum_key equal to previous"),
                &op_data.ethereum_key,
                &prev.op_data.ethereum_key,
            )?);
            is_op_data_correct_flags.push(CircuitElement::equals(
                cs.namespace(|| "is new_pubkey_hash equal to previous"),
                &op_data.new_pubkey_hash,
                &prev.op_data.new_pubkey_hash,
            )?);
            let is_op_data_equal_to_previous = multi_and(
                cs.namespace(|| "is_op_data_equal_to_previous"),
                &is_op_data_correct_flags,
            )?;
            let is_chunk_first = Boolean::from(Expression::equals(
                cs.namespace(|| "is_chunk_first"),
                &chunk_data.chunk_number,
                Expression::constant::<CS>(E::Fr::zero()),
            )?);
            let is_op_data_correct = multi_or(
                cs.namespace(|| "is_op_data_correct"),
                &[is_op_data_equal_to_previous, is_chunk_first],
            )?;
            cs.enforce(
                || "ensure op_data correct",
                |_| is_op_data_correct.lc(CS::one(), E::Fr::one()),
                |lc| lc + CS::one(),
                |lc| lc + CS::one(),
            );
        }
        prev.op_data = op_data.clone();
        let sender_pk = ecc::EdwardsPoint::interpret(
            cs.namespace(|| "signer public key"),
            &op_data.signer_pubkey.get_x().get_number(),
            &op_data.signer_pubkey.get_y().get_number(),
            self.params,
        )?;

        let signature_r_x = AllocatedNum::alloc(cs.namespace(|| "signature r_x witness"), || {
            Ok(op.signature.get()?.r.into_xy().0)
        })?;

        let signature_r_y = AllocatedNum::alloc(cs.namespace(|| "signature r_y witness"), || {
            Ok(op.signature.get()?.r.into_xy().1)
        })?;

        let signature_r = ecc::EdwardsPoint::interpret(
            cs.namespace(|| "signature r as point"),
            &signature_r_x,
            &signature_r_y,
            self.params,
        )?;

        let signature_s = AllocatedNum::alloc(cs.namespace(|| "signature s witness"), || {
            Ok(op.signature.get()?.s)
        })?;

        let signature = EddsaSignature {
            r: signature_r,
            s: signature_s,
            pk: sender_pk,
        };
        let mut temp_bits = op_data.first_sig_msg.get_bits_le();
        temp_bits.extend(op_data.second_sig_msg.get_bits_le());
        let sig_msg = pedersen_hash::pedersen_hash(
            cs.namespace(|| "sig_msg"),
            pedersen_hash::Personalization::NoteCommitment,
            &temp_bits,
            self.params,
        )?
        .get_x()
        .clone();
        let mut sig_msg_bits = sig_msg.into_bits_le(cs.namespace(|| "sig_msg_bits"))?;
        sig_msg_bits.resize(256, Boolean::constant(false));

        // signature.verify_sha256_musig(
        //     cs.namespace(|| "verify_sha"),
        //     self.params,
        //     &sig_msg_bits,
        //     generator,
        // )?;

        verify_pedersen(
            cs.namespace(|| "musig pedersen"),
            &sig_msg_bits,
            &signature,
            self.params,
            generator,
        )?;

        let diff_a_b =
            Expression::from(&op_data.a.get_number()) - Expression::from(&op_data.b.get_number());
        let mut diff_a_b_bits = diff_a_b.into_bits_le(cs.namespace(|| "balance-fee bits"))?;
        diff_a_b_bits.truncate(franklin_constants::BALANCE_BIT_WIDTH); //TODO: can be made inside helpers
        let diff_a_b_bits_repacked = Expression::le_bits::<CS>(&diff_a_b_bits);

        let is_a_geq_b = Boolean::from(Expression::equals(
            cs.namespace(|| "diff equal to repacked"),
            diff_a_b,
            diff_a_b_bits_repacked,
        )?);

        let mut op_flags = vec![];
        op_flags.push(self.deposit(
            cs.namespace(|| "deposit"),
            &mut cur,
            &chunk_data,
            &is_a_geq_b,
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
            &ext_pubdata_chunk,
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
            &ext_pubdata_chunk,
        )?);
        op_flags.push(self.partial_exit(
            cs.namespace(|| "partial_exit"),
            &mut cur,
            &chunk_data,
            &is_a_geq_b,
            &op_data,
            &ext_pubdata_chunk,
        )?);
        op_flags.push(self.close_account(
            cs.namespace(|| "close_account"),
            &mut cur,
            &chunk_data,
            &ext_pubdata_chunk,
            &op_data,
            &subtree_root,
        )?);
        op_flags.push(self.noop(cs.namespace(|| "noop"), &chunk_data, &ext_pubdata_chunk)?);

        let op_valid = multi_or(cs.namespace(|| "op_valid"), &op_flags)?;

        Boolean::enforce_equal(
            cs.namespace(|| "op_valid is true"),
            &op_valid,
            &Boolean::constant(true),
        )?;
        for (i, fee) in fees
            .iter_mut()
            .enumerate()
            .take(1 << franklin_constants::BALANCE_TREE_DEPTH)
        {
            let sum = Expression::from(&fee.clone()) + Expression::from(&op_data.fee.get_number());

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

    fn partial_exit<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        is_a_geq_b: &Boolean,
        op_data: &AllocatedOperationData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        let mut base_valid_flags = vec![];
        //construct pubdata
        let mut pubdata_bits = vec![];
        let mut pub_token_bits = cur.token.get_bits_le().clone();
        pub_token_bits.resize(
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        pub_token_bits.reverse();
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
        pubdata_bits.extend(cur.account_address.get_bits_be()); //ACCOUNT_TREE_DEPTH=24
        pubdata_bits.extend(pub_token_bits); //TOKEN_EXT_BIT_WIDTH=16
        pubdata_bits.extend(op_data.amount_packed.get_bits_be()); //AMOUNT_PACKED=24
        pubdata_bits.extend(op_data.fee_packed.get_bits_be()); //FEE_PACKED=8
        pubdata_bits.extend(op_data.ethereum_key.get_bits_be()); //ETHEREUM_KEY=160
                                                                 //        assert_eq!(pubdata_bits.len(), 30 * 8);
        pubdata_bits.resize(
            4 * franklin_constants::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            4,
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
        let is_partial_exit = Boolean::from(Expression::equals(
            cs.namespace(|| "is_partial_exit"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(3), //partial_exit tx code
        )?);
        base_valid_flags.push(is_partial_exit.clone());

        // construct signature message

        let mut sig_bits = vec![];

        sig_bits.extend(chunk_data.tx_type.get_bits_be());
        sig_bits.extend(cur.account.pub_key_hash.get_bits_be());
        sig_bits.extend(op_data.ethereum_key.get_bits_be());
        let mut token_bits = cur.token.get_bits_le().clone();
        token_bits.resize(
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        token_bits.reverse();
        sig_bits.extend(token_bits);
        sig_bits.extend(op_data.amount_packed.get_bits_be());
        sig_bits.extend(op_data.fee_packed.get_bits_be());
        sig_bits.extend(cur.account.nonce.get_bits_be());

        sig_bits.resize(E::Fr::CAPACITY as usize * 2, Boolean::constant(false));
        let (first_sig_part_bits, second_sig_part_bits) =
            sig_bits.split_at(E::Fr::CAPACITY as usize);
        let first_sig_part =
            pack_bits_to_element(cs.namespace(|| "first_sig_part"), &first_sig_part_bits)?;

        let second_sig_part =
            pack_bits_to_element(cs.namespace(|| "second_sig_part"), &second_sig_part_bits)?;

        let is_first_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_sig_part_correct"),
            Expression::from(&first_sig_part),
            Expression::from(&op_data.first_sig_msg.get_number()),
        )?);

        let is_second_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_second_sig_part_correct"),
            Expression::from(&second_sig_part),
            Expression::from(&op_data.second_sig_msg.get_number()),
        )?);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correect"),
            &op_data.signer_pubkey.get_hash(),
            &cur.account.pub_key_hash, //earlier we ensured that this new_pubkey_hash is equal to current if existed
        )?;

        let mut is_sig_valid_flags = vec![];

        is_sig_valid_flags.push(is_first_sig_part_correct);
        is_sig_valid_flags.push(is_second_sig_part_correct);
        is_sig_valid_flags.push(is_signer_valid);
        let is_sig_valid = multi_and(cs.namespace(|| "is_sig_valid"), &is_sig_valid_flags)?;
        let is_sig_correct = multi_or(
            cs.namespace(|| "sig is valid or not first chunk"),
            &[is_sig_valid, is_first_chunk.clone().not()],
        )?;
        base_valid_flags.push(is_sig_correct);
        let is_base_valid = multi_and(
            cs.namespace(|| "valid base partial_exit"),
            &base_valid_flags,
        )?;
        //here we verify whether exit should be full
        let is_full_exit = Boolean::from(Expression::equals(
            cs.namespace(|| "amount is zero"),
            &op_data.amount.get_number(),
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let lhs_partial_valid: Boolean;
        {
            let mut lhs_partial_valid_flags = vec![];

            let cs = &mut cs.namespace(|| "partial_exit");
            lhs_partial_valid_flags.push(is_full_exit.not().clone());
            lhs_partial_valid_flags.push(is_base_valid.clone());

            // check operation arguments
            let is_a_correct =
                CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;

            lhs_partial_valid_flags.push(is_a_correct);

            let sum_amount_fee = Expression::from(&op_data.amount.get_number())
                + Expression::from(&op_data.fee.get_number());

            let is_b_correct = Boolean::from(Expression::equals(
                cs.namespace(|| "is_b_correct"),
                &op_data.b.get_number(),
                sum_amount_fee.clone(),
            )?);
            lhs_partial_valid_flags.push(is_b_correct);
            lhs_partial_valid_flags.push(is_a_geq_b.clone());

            lhs_partial_valid_flags.push(no_nonce_overflow(
                cs.namespace(|| "no nonce overflow"),
                &cur.account.nonce.get_number(),
            )?);

            lhs_partial_valid_flags.push(is_first_chunk.clone());
            lhs_partial_valid =
                multi_and(cs.namespace(|| "is_lhs_valid"), &lhs_partial_valid_flags)?;

            let updated_balance = Expression::from(&cur.balance.get_number()) - sum_amount_fee;

            //mutate current branch if it is first chunk of valid partial_exit transaction
            cur.balance = CircuitElement::conditionally_select_with_number_strict(
                cs.namespace(|| "mutated balance"),
                updated_balance,
                &cur.balance,
                &lhs_partial_valid,
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
                &lhs_partial_valid,
            )?;
        }

        let lhs_full_valid: Boolean;
        {
            let mut lhs_full_valid_flags = vec![];

            let cs = &mut cs.namespace(|| "full_exit");
            lhs_full_valid_flags.push(is_full_exit.clone());
            lhs_full_valid_flags.push(is_base_valid.clone());
            lhs_full_valid_flags.push(is_first_chunk.clone());

            let diff_balance_fee = Expression::from(&cur.balance.get_number())
                - Expression::from(&op_data.fee.get_number());
            let mut diff_balance_fee_bits =
                diff_balance_fee.into_bits_le(cs.namespace(|| "balance-fee bits"))?;
            diff_balance_fee_bits.truncate(franklin_constants::BALANCE_BIT_WIDTH); //TODO: can be made inside helpers
            let diff_balance_fee_bits_repacked = Expression::le_bits::<CS>(&diff_balance_fee_bits);

            let is_balance_geq_fee = Boolean::from(Expression::equals(
                cs.namespace(|| "diff equal to repacked"),
                diff_balance_fee,
                diff_balance_fee_bits_repacked,
            )?);
            lhs_full_valid_flags.push(is_balance_geq_fee);

            lhs_full_valid_flags.push(no_nonce_overflow(
                cs.namespace(|| "no nonce overflow"),
                &cur.account.nonce.get_number(),
            )?);

            lhs_full_valid = multi_and(cs.namespace(|| "lhs_full_valid"), &lhs_full_valid_flags)?;

            //update cur data if we are processing first operation of valid partial_exit transaction
            let updated_balance_value = Expression::constant::<CS>(E::Fr::zero());

            //mutate current branch if it is first chunk of valid partial_exit transaction
            cur.balance = CircuitElement::conditionally_select_with_number_strict(
                cs.namespace(|| "mutated balance"),
                updated_balance_value,
                &cur.balance,
                &lhs_full_valid,
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
                &lhs_full_valid,
            )?;
        }

        let mut ohs_valid_flags = vec![];
        ohs_valid_flags.push(is_base_valid);
        ohs_valid_flags.push(is_first_chunk.not());
        let is_ohs_valid = multi_and(cs.namespace(|| "is_ohs_valid"), &ohs_valid_flags)?;

        let tx_valid = multi_or(
            cs.namespace(|| "tx_valid"),
            &[lhs_partial_valid, lhs_full_valid, is_ohs_valid],
        )?;
        Ok(tx_valid)
    }

    fn deposit<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        is_a_geq_b: &Boolean,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        //useful below
        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let mut is_valid_flags = vec![];
        //construct pubdata
        let mut pubdata_bits = vec![];
        let mut pub_token_bits = cur.token.get_bits_le().clone();
        pub_token_bits.resize(
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        pub_token_bits.reverse();
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
        pubdata_bits.extend(cur.account_address.get_bits_be()); //ACCOUNT_TREE_DEPTH=24
        pubdata_bits.extend(pub_token_bits); //TOKEN_EXT_BIT_WIDTH=16
        pubdata_bits.extend(op_data.amount_packed.get_bits_be()); //AMOUNT_PACKED=24
        pubdata_bits.extend(op_data.fee_packed.get_bits_be()); //FEE_PACKED=8
        pubdata_bits.extend(op_data.new_pubkey_hash.get_bits_be()); //NEW_PUBKEY_HASH_WIDTH=216
        pubdata_bits.resize(
            4 * franklin_constants::CHUNK_BIT_WIDTH, //TODO: move to constant
            Boolean::constant(false),
        );

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            4,
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
            Expression::u64::<CS>(1), //TODO: move to constants
        )?);
        is_valid_flags.push(is_deposit.clone());

        // verify if new pubkey is equal to previous one (if existed)
        let is_pub_equal_to_previous = CircuitElement::equals(
            cs.namespace(|| "is_pub_equal_to_previous"),
            &op_data.new_pubkey_hash,
            &cur.account.pub_key_hash,
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
        let is_a_correct =
            CircuitElement::equals(cs.namespace(|| "a == amount"), &op_data.amount, &op_data.a)?;

        is_valid_flags.push(is_a_correct);
        let is_b_correct =
            CircuitElement::equals(cs.namespace(|| "b == fee"), &op_data.fee, &op_data.b)?;

        is_valid_flags.push(is_b_correct);
        is_valid_flags.push(is_a_geq_b.clone());

        // construct signature message

        let mut sig_bits = vec![];

        sig_bits.extend(chunk_data.tx_type.get_bits_be());
        sig_bits.extend(op_data.new_pubkey_hash.get_bits_be());
        let mut token_bits = cur.token.get_bits_le().clone();
        token_bits.resize(
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        token_bits.reverse();
        sig_bits.extend(token_bits);
        sig_bits.extend(op_data.amount_packed.get_bits_be());
        sig_bits.extend(op_data.fee_packed.get_bits_be());
        sig_bits.extend(cur.account.nonce.get_bits_be());
        sig_bits.resize(E::Fr::CAPACITY as usize * 2, Boolean::constant(false));
        let (first_sig_part_bits, second_sig_part_bits) =
            sig_bits.split_at(E::Fr::CAPACITY as usize);
        let first_sig_part =
            pack_bits_to_element(cs.namespace(|| "first_sig_part"), &first_sig_part_bits)?;

        let second_sig_part =
            pack_bits_to_element(cs.namespace(|| "second_sig_part"), &second_sig_part_bits)?;

        let is_first_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_sig_part_correct"),
            Expression::from(&first_sig_part),
            Expression::from(&op_data.first_sig_msg.get_number()),
        )?);

        let is_second_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_second_sig_part_correct"),
            Expression::from(&second_sig_part),
            Expression::from(&op_data.second_sig_msg.get_number()),
        )?);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correect"),
            &op_data.signer_pubkey.get_hash(),
            &op_data.new_pubkey_hash, //earlier we ensured that this new_pubkey_hash is equal to current if existed
        )?;

        let mut is_sig_valid_flags = vec![];

        is_sig_valid_flags.push(is_first_sig_part_correct);
        is_sig_valid_flags.push(is_second_sig_part_correct);
        is_sig_valid_flags.push(is_signer_valid);
        let is_sig_valid = multi_and(cs.namespace(|| "is_sig_valid"), &is_sig_valid_flags)?;
        let is_sig_correct = multi_or(
            cs.namespace(|| "sig is valid or not first chunk"),
            &[is_sig_valid, is_first_chunk.clone().not()],
        )?;
        is_valid_flags.push(is_sig_correct);

        let tx_valid = multi_and(cs.namespace(|| "is_tx_valid"), &is_valid_flags)?;

        let is_valid_first = Boolean::and(
            cs.namespace(|| "is valid and first"),
            &tx_valid,
            &is_first_chunk,
        )?;

        let updated_balance = Expression::from(&cur.balance.get_number())
            + Expression::from(&op_data.amount.get_number());

        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "mutated balance"),
            updated_balance,
            &cur.balance,
            &is_valid_first,
        )?;

        // update pub_key
        cur.account.pub_key_hash = CircuitElement::conditionally_select(
            cs.namespace(|| "mutated_pubkey"),
            &op_data.new_pubkey_hash,
            &cur.account.pub_key_hash,
            &is_valid_first,
        )?;

        // update nonce
        let updated_nonce =
            Expression::from(&cur.account.nonce.get_number()) + Expression::u64::<CS>(1); //TODO: we can provide Expression++ syntax

        cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "update cur nonce"),
            updated_nonce,
            &cur.account.nonce,
            &is_valid_first,
        )?;
        Ok(tx_valid)
    }

    fn close_account<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        op_data: &AllocatedOperationData<E>,
        subtree_root: &CircuitElement<E>,
    ) -> Result<Boolean, SynthesisError> {
        let mut is_valid_flags = vec![];
        //construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
        pubdata_bits.extend(cur.account_address.get_bits_be()); //ACCOUNT_TREE_DEPTH=24
        pubdata_bits.resize(
            franklin_constants::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

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

        let is_close_account = Boolean::from(Expression::equals(
            cs.namespace(|| "is_deposit"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(4), //close_account tx_type
        )?);
        is_valid_flags.push(is_close_account.clone());

        let tmp = CircuitAccount::<E>::empty_balances_root_hash();
        let mut r_repr = E::Fr::zero().into_repr();
        r_repr.read_be(&tmp[..]).unwrap();
        let empty_root = E::Fr::from_repr(r_repr).unwrap();

        let are_balances_empty = Boolean::from(Expression::equals(
            cs.namespace(|| "are_balances_empty"),
            &subtree_root.get_number(),
            Expression::constant::<CS>(empty_root), //This is precalculated root_hash of subtree with empty balances
        )?);
        is_valid_flags.push(are_balances_empty);

        // construct signature message

        let mut sig_bits = vec![];

        sig_bits.extend(chunk_data.tx_type.get_bits_be());
        sig_bits.extend(cur.account.pub_key_hash.get_bits_be());
        sig_bits.extend(cur.account.nonce.get_bits_be());

        sig_bits.resize(E::Fr::CAPACITY as usize * 2, Boolean::constant(false));
        let (first_sig_part_bits, second_sig_part_bits) =
            sig_bits.split_at(E::Fr::CAPACITY as usize);
        let first_sig_part =
            pack_bits_to_element(cs.namespace(|| "first_sig_part"), &first_sig_part_bits)?;

        let second_sig_part =
            pack_bits_to_element(cs.namespace(|| "second_sig_part"), &second_sig_part_bits)?;

        let is_first_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_sig_part_correct"),
            Expression::from(&first_sig_part),
            Expression::from(&op_data.first_sig_msg.get_number()),
        )?);

        let is_second_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_second_sig_part_correct"),
            Expression::from(&second_sig_part),
            Expression::from(&op_data.second_sig_msg.get_number()),
        )?);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correect"),
            &op_data.signer_pubkey.get_hash(),
            &cur.account.pub_key_hash, //earlier we ensured that this new_pubkey_hash is equal to current if existed
        )?;

        let mut is_sig_valid_flags = vec![];

        is_sig_valid_flags.push(is_first_sig_part_correct);
        is_sig_valid_flags.push(is_second_sig_part_correct);
        is_sig_valid_flags.push(is_signer_valid);
        let is_sig_valid = multi_and(cs.namespace(|| "is_sig_valid"), &is_sig_valid_flags)?;

        is_valid_flags.push(is_sig_valid);
        let tx_valid = multi_and(cs.namespace(|| "is_tx_valid"), &is_valid_flags)?;

        // below we conditionally if it is valid operation

        // update pub_key
        cur.account.pub_key_hash = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "mutated_pubkey"),
            Expression::constant::<CS>(E::Fr::zero()),
            &cur.account.pub_key_hash,
            &tx_valid,
        )?;
        // update nonce
        cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "update cur nonce"),
            Expression::constant::<CS>(E::Fr::zero()),
            &cur.account.nonce,
            &tx_valid,
        )?;

        Ok(tx_valid)
    }

    fn noop<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        chunk_data: &AllocatedChunkData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        let mut is_valid_flags = vec![];
        //construct pubdata (it's all 0 for noop)
        let mut pubdata_bits = vec![];
        pubdata_bits.resize(
            franklin_constants::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

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
        is_valid_flags.push(is_noop.clone());

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
        ext_pubdata_chunk: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        let mut pubdata_bits = vec![];
        let mut pub_token_bits = lhs.token.get_bits_le().clone();
        pub_token_bits.resize(
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        pub_token_bits.reverse();
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //8
        pubdata_bits.extend(lhs.account_address.get_bits_be()); //24
        pubdata_bits.extend(pub_token_bits.clone()); //16
        pubdata_bits.extend(op_data.amount_packed.get_bits_be()); //24
        pubdata_bits.extend(op_data.new_pubkey_hash.get_bits_be()); //160
        pubdata_bits.extend(rhs.account_address.get_bits_be()); //24
        pubdata_bits.extend(op_data.fee_packed.get_bits_be()); //8
        pubdata_bits.resize(
            5 * franklin_constants::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );
        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            5,
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
            Expression::u64::<CS>(2),
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

        let sum_amount_fee = Expression::from(&op_data.amount.get_number())
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

        // construct signature message

        let mut sig_bits = vec![];
        let tx_code = CircuitElement::from_fe_strict(
            cs.namespace(|| "5_ce"),
            || Ok(E::Fr::from_str("5").unwrap()),
            8,
        )?; //we use here transfer tx_code=5 to allow user sign message without knowing whether it is transfer_to_new or transfer
        sig_bits.extend(tx_code.get_bits_be());
        sig_bits.extend(lhs.account.pub_key_hash.get_bits_be());
        sig_bits.extend(op_data.new_pubkey_hash.get_bits_be());
        let mut token_bits = cur.token.get_bits_le().clone(); //TODO: make util to get this token bits
        token_bits.resize(
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        token_bits.reverse();
        sig_bits.extend(token_bits);
        sig_bits.extend(op_data.amount_packed.get_bits_be());
        sig_bits.extend(op_data.fee_packed.get_bits_be());
        sig_bits.extend(cur.account.nonce.get_bits_be());
        sig_bits.resize(E::Fr::CAPACITY as usize * 2, Boolean::constant(false));
        let (first_sig_part_bits, second_sig_part_bits) =
            sig_bits.split_at(E::Fr::CAPACITY as usize);
        let first_sig_part =
            pack_bits_to_element(cs.namespace(|| "first_sig_part"), &first_sig_part_bits)?;
        let second_sig_part =
            pack_bits_to_element(cs.namespace(|| "second_sig_part"), &second_sig_part_bits)?;

        let is_first_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_sig_part_correct"),
            Expression::from(&first_sig_part),
            Expression::from(&op_data.first_sig_msg.get_number()),
        )?);

        let is_second_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_second_sig_part_correct"),
            Expression::from(&second_sig_part),
            Expression::from(&op_data.second_sig_msg.get_number()),
        )?);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correect"),
            &op_data.signer_pubkey.get_hash(),
            &lhs.account.pub_key_hash,
        )?;

        let mut is_sig_valid_flags = vec![];

        is_sig_valid_flags.push(is_first_sig_part_correct);
        is_sig_valid_flags.push(is_second_sig_part_correct);
        is_sig_valid_flags.push(is_signer_valid);
        let is_sig_valid = multi_and(cs.namespace(|| "is_sig_valid"), &is_sig_valid_flags)?;
        let is_sig_correct = multi_or(
            cs.namespace(|| "sig is valid or not first chunk"),
            &[is_sig_valid, is_first_chunk.clone().not()],
        )?; //actually redundant due to check only on lhs
        lhs_valid_flags.push(is_sig_correct);

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
            &op_data.amount,
            &cur.balance,
            &rhs_valid,
        )?;
        cur.balance
            .enforce_length(cs.namespace(|| "mutated balance is still correct length"))?; // TODO: this is actually redundant, cause they are both enforced to be of appropriate length

        cur.account.pub_key_hash = CircuitElement::conditionally_select(
            cs.namespace(|| "mutated_pubkey"),
            &op_data.new_pubkey_hash,
            &cur.account.pub_key_hash,
            &rhs_valid,
        )?;

        let mut ohs_valid_flags = vec![];
        ohs_valid_flags.push(is_pubdata_chunk_correct.clone());
        ohs_valid_flags.push(is_first_chunk.not().clone());
        ohs_valid_flags.push(is_second_chunk.not().clone());
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
        ext_pubdata_chunk: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        // construct pubdata
        let mut pubdata_bits = vec![];
        let mut pub_token_bits = lhs.token.get_bits_le().clone();
        pub_token_bits.resize(
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        pub_token_bits.reverse();
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be());
        pubdata_bits.extend(lhs.account_address.get_bits_be());
        pubdata_bits.extend(pub_token_bits.clone());
        pubdata_bits.extend(rhs.account_address.get_bits_be());
        pubdata_bits.extend(op_data.amount_packed.get_bits_be());
        pubdata_bits.extend(op_data.fee_packed.get_bits_be());

        pubdata_bits.resize(
            2 * franklin_constants::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            2,
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
            Expression::u64::<CS>(5), // transfer tx_type
        )?);

        let mut lhs_valid_flags = vec![];

        lhs_valid_flags.push(is_pubdata_chunk_correct.clone());
        lhs_valid_flags.push(is_transfer.clone());

        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);
        lhs_valid_flags.push(is_first_chunk.clone());

        // check operation arguments
        let is_a_correct =
            CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;

        lhs_valid_flags.push(is_a_correct);

        let sum_amount_fee = Expression::from(&op_data.amount.get_number())
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

        // construct signature message

        let mut sig_bits = vec![];

        sig_bits.extend(chunk_data.tx_type.get_bits_be());
        sig_bits.extend(lhs.account.pub_key_hash.get_bits_be());
        sig_bits.extend(rhs.account.pub_key_hash.get_bits_be());
        let mut token_bits = cur.token.get_bits_le().clone(); //TODO: make util to get this token bits
        token_bits.resize(
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        token_bits.reverse();
        sig_bits.extend(token_bits);
        sig_bits.extend(op_data.amount_packed.get_bits_be());
        sig_bits.extend(op_data.fee_packed.get_bits_be());
        sig_bits.extend(cur.account.nonce.get_bits_be());
        sig_bits.resize(E::Fr::CAPACITY as usize * 2, Boolean::constant(false));
        let (first_sig_part_bits, second_sig_part_bits) =
            sig_bits.split_at(E::Fr::CAPACITY as usize);
        let first_sig_part =
            pack_bits_to_element(cs.namespace(|| "first_sig_part"), &first_sig_part_bits)?;

        let second_sig_part =
            pack_bits_to_element(cs.namespace(|| "second_sig_part"), &second_sig_part_bits)?;

        let is_first_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_sig_part_correct"),
            Expression::from(&first_sig_part),
            Expression::from(&op_data.first_sig_msg.get_number()),
        )?);

        let is_second_sig_part_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_second_sig_part_correct"),
            Expression::from(&second_sig_part),
            Expression::from(&op_data.second_sig_msg.get_number()),
        )?);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correect"),
            &op_data.signer_pubkey.get_hash(),
            &lhs.account.pub_key_hash,
        )?;

        let mut is_sig_valid_flags = vec![];

        is_sig_valid_flags.push(is_first_sig_part_correct);
        is_sig_valid_flags.push(is_second_sig_part_correct);
        is_sig_valid_flags.push(is_signer_valid);
        let is_sig_valid = multi_and(cs.namespace(|| "is_sig_valid"), &is_sig_valid_flags)?;
        let is_sig_correct = multi_or(
            cs.namespace(|| "sig is valid or not first chunk"),
            &[is_sig_valid, is_first_chunk.clone().not()],
        )?; //actually redundant due to check only on lhs
        lhs_valid_flags.push(is_sig_correct);

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
        rhs_valid_flags.push(is_transfer.clone());

        let is_chunk_second = Boolean::from(Expression::equals(
            cs.namespace(|| "is_chunk_second"),
            &chunk_data.chunk_number,
            Expression::u64::<CS>(1),
        )?);
        rhs_valid_flags.push(is_chunk_second);
        rhs_valid_flags.push(is_account_empty.not());

        rhs_valid_flags.push(is_pubdata_chunk_correct.clone());
        let is_rhs_valid = multi_and(cs.namespace(|| "is_rhs_valid"), &rhs_valid_flags)?;

        // calculate new rhs balance value
        let updated_balance = Expression::from(&cur.balance.get_number())
            + Expression::from(&op_data.amount.get_number());

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

    fn allocate_account_leaf_bits<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        branch: &AllocatedOperationBranch<E>,
    ) -> Result<(Vec<Boolean>, Boolean, CircuitElement<E>), SynthesisError> {
        //first we prove calculate root of the subtree to obtain account_leaf_data:

        let balance_data = &branch.balance.get_bits_le();
        let balance_root = allocate_merkle_root(
            cs.namespace(|| "balance_subtree_root"),
            balance_data,
            &branch.token.get_bits_le(),
            &branch.balance_audit_path,
            self.params,
        )?;

        // println!("balance root: {}", balance_root.get_value().unwrap());
        let subtree_root =
            CircuitElement::from_number_padded(cs.namespace(|| "subtree_root_ce"), balance_root)?;

        let mut account_data = vec![];
        account_data.extend(branch.account.nonce.get_bits_le());
        account_data.extend(branch.account.pub_key_hash.get_bits_le());

        let account_data_packed =
            pack_bits_to_element(cs.namespace(|| "account_data_packed"), &account_data)?;

        let is_account_empty = Expression::equals(
            cs.namespace(|| "is_account_empty"),
            &account_data_packed,
            Expression::constant::<CS>(E::Fr::zero()),
        )?;
        account_data.extend(subtree_root.get_bits_le());
        Ok((account_data, Boolean::from(is_account_empty), subtree_root))
    }
}

fn allocate_merkle_root<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    leaf_bits: &[Boolean],
    index: &[Boolean],
    audit_path: &[AllocatedNum<E>],
    params: &E::Params,
) -> Result<AllocatedNum<E>, SynthesisError> {
    assert_eq!(index.len(), audit_path.len());

    let account_leaf_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "account leaf content hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &leaf_bits,
        params,
    )?;
    // This is an injective encoding, as cur is a
    // point in the prime order subgroup.
    let mut cur_hash = account_leaf_hash.get_x().clone();

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

        // We don't need to be strict, because the function is
        // collision-resistant. If the prover witnesses a congruency,
        // they will be unable to find an authentication path in the
        // tree with high probability.
        let mut preimage = vec![];
        preimage.extend(xl.into_bits_le(cs.namespace(|| "xl into bits"))?);
        preimage.extend(xr.into_bits_le(cs.namespace(|| "xr into bits"))?);

        // Compute the new subtree value
        cur_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "computation of pedersen hash"),
            pedersen_hash::Personalization::MerkleTree(i),
            &preimage,
            params,
        )?
        .get_x()
        .clone(); // Injective encoding
    }

    Ok(cur_hash.clone())
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

fn multi_and<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    x: &[Boolean],
) -> Result<Boolean, SynthesisError> {
    let mut result = Boolean::constant(true);

    for (i, bool_x) in x.iter().enumerate() {
        result = Boolean::and(
            cs.namespace(|| format!("multi and iteration number: {}", i)),
            &result,
            bool_x,
        )?;
        //        println!("and number i:{} value:{}", i, result.get_value().grab()?);
    }

    Ok(result)
}

fn verify_pedersen<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    sig_data_bits: &[Boolean],
    signature: &EddsaSignature<E>,
    params: &E::Params,
    generator: ecc::EdwardsPoint<E>,
) -> Result<(), SynthesisError> {
    let mut sig_data_bits = sig_data_bits.to_vec();
    sig_data_bits.resize(256, Boolean::constant(false));

    let mut first_round_bits: Vec<Boolean> = vec![];

    let mut pk_x_serialized = signature
        .pk
        .get_x()
        .clone()
        .into_bits_le(cs.namespace(|| "pk_x_bits"))?;
    pk_x_serialized.resize(256, Boolean::constant(false));

    let mut r_x_serialized = signature
        .r
        .get_x()
        .clone()
        .into_bits_le(cs.namespace(|| "r_x_bits"))?;
    r_x_serialized.resize(256, Boolean::constant(false));

    first_round_bits.extend(pk_x_serialized);
    first_round_bits.extend(r_x_serialized);

    let first_round_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "first_round_hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &first_round_bits,
        params,
    )?;
    let mut first_round_hash_bits = first_round_hash
        .get_x()
        .into_bits_le(cs.namespace(|| "first_round_hash_bits"))?;
    first_round_hash_bits.resize(256, Boolean::constant(false));

    let mut second_round_bits = vec![];
    second_round_bits.extend(first_round_hash_bits);
    second_round_bits.extend(sig_data_bits);
    let second_round_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "second_hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &second_round_bits,
        params,
    )?
    .get_x()
    .clone();

    let h_bits = second_round_hash.into_bits_le(cs.namespace(|| "h_bits"))?;

    let max_message_len = 32 as usize; //since it is the result of pedersen hash
    signature.verify_raw_message_signature(
        cs.namespace(|| "verify transaction signature"),
        params,
        &h_bits,
        generator,
        max_message_len,
    )?;
    Ok(())
}

fn select_pubdata_chunk<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    pubdata_bits: &[Boolean],
    chunk_number: &AllocatedNum<E>,
    total_chunks: usize,
) -> Result<AllocatedNum<E>, SynthesisError> {
    assert_eq!(
        pubdata_bits.len(),
        total_chunks * franklin_constants::CHUNK_BIT_WIDTH
    );
    let mut result =
        AllocatedNum::alloc(
            cs.namespace(|| "result pubdata chunk"),
            || Ok(E::Fr::zero()),
        )?;

    for i in 0..total_chunks {
        let cs = &mut cs.namespace(|| format!("chunk number {}", i));
        let pub_chunk_bits = pubdata_bits[i * franklin_constants::CHUNK_BIT_WIDTH
            ..(i + 1) * franklin_constants::CHUNK_BIT_WIDTH]
            .to_vec();
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
fn calculate_root_from_full_representation_fees<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    fees: &[AllocatedNum<E>],
    params: &E::Params,
) -> Result<AllocatedNum<E>, SynthesisError> {
    assert_eq!(fees.len(), 1 << franklin_constants::BALANCE_TREE_DEPTH);
    let mut fee_hashes = vec![];
    for (index, fee) in fees.iter().enumerate() {
        let cs = &mut cs.namespace(|| format!("fee hashing index number {}", index));
        let mut fee_bits = fee.into_bits_le(cs.namespace(|| "fee_bits"))?;
        fee_bits.truncate(franklin_constants::BALANCE_BIT_WIDTH);
        let temp = pedersen_hash::pedersen_hash(
            cs.namespace(|| "account leaf content hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &fee_bits,
            params,
        )?;
        fee_hashes.push(temp.get_x().clone());
    }
    let mut hash_vec = fee_hashes;

    for i in 0..franklin_constants::BALANCE_TREE_DEPTH {
        let cs = &mut cs.namespace(|| format!("merkle tree level index number {}", i));
        let chunks = hash_vec.chunks(2);
        let mut new_hashes = vec![];
        for (chunk_number, x) in chunks.enumerate() {
            let cs = &mut cs.namespace(|| format!("chunk number {}", chunk_number));
            let mut preimage = vec![];
            preimage.extend(x[0].into_bits_le(cs.namespace(|| "x[0] into bits"))?);
            preimage.extend(x[1].into_bits_le(cs.namespace(|| "x[1] into bits"))?);
            let hash = pedersen_hash::pedersen_hash(
                cs.namespace(|| "account leaf content hash"),
                pedersen_hash::Personalization::MerkleTree(i),
                &preimage,
                params,
            )?;
            new_hashes.push(hash.get_x().clone());
        }
        hash_vec = new_hashes;
    }
    assert_eq!(hash_vec.len(), 1);
    Ok(hash_vec[0].clone())
}

fn generate_maxchunk_polynomial<E: JubjubEngine>() -> Vec<E::Fr> {
    use franklin_crypto::interpolation::interpolate;

    let mut points: Vec<(E::Fr, E::Fr)> = vec![];
    for i in &[0, 4] {
        //noop, increment_nonce, partial_exit, close_account, escalation
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::zero();
        points.push((x, y));
    }

    for i in &[5] {
        //transfer, create_subaccount, close_subaccount, fill_orders
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::from_str("1").unwrap();
        points.push((x, y));
    }
    for i in &[1, 3] {
        //deposit, partial exit
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::from_str("3").unwrap();
        points.push((x, y));
    }

    for i in &[2] {
        //transfer_to_new
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::from_str("4").unwrap();
        points.push((x, y));
    }

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
        Expression::constant::<CS>(E::Fr::from_str(&(256 * 256 - 1).to_string()).unwrap()),
    )?)
    .not())
}
