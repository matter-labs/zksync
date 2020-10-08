// External deps
use zksync_crypto::franklin_crypto::{
    bellman::{
        pairing::ff::{Field, PrimeField},
        Circuit, ConstraintSystem, SynthesisError,
    },
    circuit::{
        boolean::Boolean,
        ecc,
        expression::Expression,
        multipack,
        num::AllocatedNum,
        polynomial_lookup::{do_the_lookup, generate_powers},
        rescue, sha256, Assignment,
    },
    jubjub::{FixedGenerators, JubjubEngine, JubjubParams},
    rescue::RescueEngine,
};
// Workspace deps
use zksync_crypto::params::{
    self, FR_BIT_WIDTH_PADDED, SIGNED_FORCED_EXIT_BIT_WIDTH, SIGNED_TRANSFER_BIT_WIDTH,
};
use zksync_types::{
    operations::{ChangePubKeyOp, NoopOp},
    CloseOp, DepositOp, ForcedExitOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp,
};
// Local deps
use crate::{
    account::{AccountContent, AccountWitness},
    allocated_structures::*,
    element::CircuitElement,
    operation::Operation,
    signature::{
        unpack_point_if_possible, verify_circuit_signature, verify_signature_message_construction,
        AllocatedSignerPubkey,
    },
    utils::{
        allocate_numbers_vec, allocate_sum, boolean_or, calculate_empty_account_tree_hashes,
        calculate_empty_balance_tree_hashes, multi_and, pack_bits_to_element_strict,
        resize_grow_only, vectorized_compare,
    },
};

const DIFFERENT_TRANSACTIONS_TYPE_NUMBER: usize = 9;
pub struct ZkSyncCircuit<'a, E: RescueEngine + JubjubEngine> {
    pub rescue_params: &'a <E as RescueEngine>::Params,
    pub jubjub_params: &'a <E as JubjubEngine>::Params,
    /// The old root of the tree
    pub old_root: Option<E::Fr>,
    pub initial_used_subtree_root: Option<E::Fr>,

    pub block_number: Option<E::Fr>,
    pub validator_address: Option<E::Fr>,

    pub pub_data_commitment: Option<E::Fr>,
    pub operations: Vec<Operation<E>>,

    pub validator_balances: Vec<Option<E::Fr>>,
    pub validator_audit_path: Vec<Option<E::Fr>>,
    pub validator_account: AccountWitness<E>,
}

impl<'a, E: RescueEngine + JubjubEngine> std::clone::Clone for ZkSyncCircuit<'a, E> {
    fn clone(&self) -> Self {
        Self {
            rescue_params: self.rescue_params,
            jubjub_params: self.jubjub_params,
            old_root: self.old_root,
            initial_used_subtree_root: self.initial_used_subtree_root,
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
impl<'a, E: RescueEngine + JubjubEngine> Circuit<E> for ZkSyncCircuit<'a, E> {
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

        let old_root = AllocatedNum::alloc(cs.namespace(|| "old_root"), || self.old_root.grab())?;
        let mut rolling_root = {
            let initial_used_subtree_root =
                AllocatedNum::alloc(cs.namespace(|| "initial_used_subtree_root"), || {
                    self.initial_used_subtree_root.grab()
                })?;
            let old_root_from_subroot = continue_leftmost_subroot_to_root(
                cs.namespace(|| "continue initial_used_subtree root to old_root"),
                &initial_used_subtree_root,
                params::used_account_subtree_depth(),
                params::account_tree_depth(),
                self.rescue_params,
            )?;
            // ensure that old root contains initial_root
            cs.enforce(
                || "old_root contains initial_used_subtree_root",
                |lc| lc + old_root_from_subroot.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + old_root.get_variable(),
            );

            initial_used_subtree_root
        };

        // first chunk of block should always have number 0
        let mut next_chunk_number = zero.clone();

        // declare vector of fees, that will be collected during block processing
        let mut fees = vec![];
        let fees_len = params::number_of_processable_tokens();
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

        // we create a memory value for a token ID that is used to collect fees.
        // It is overwritten when we enter the first chunk of the op (that exposes sender
        // and defined a token in which transaction is valued). Later one (at the last chunk)
        // we use this value to give fee to the operator
        let mut last_token_id = zero.clone();

        // allocate some memory for every operation
        // when operation allocated pubdata from witness (that happens every chunk!)
        // we check that this pubdata is equal to pubdata if the previous chunk in the same
        // operation. So, we only allow these values to change if it's a first chunk in the operation
        let mut pubdata_holder = {
            let mut data = vec![vec![]; DIFFERENT_TRANSACTIONS_TYPE_NUMBER];

            data[NoopOp::OP_CODE as usize] = vec![]; // No-op allocated constant pubdata
            data[DepositOp::OP_CODE as usize] = vec![zero.clone(); 2];
            data[TransferOp::OP_CODE as usize] = vec![zero.clone(); 1];
            data[TransferToNewOp::OP_CODE as usize] = vec![zero.clone(); 2];
            data[WithdrawOp::OP_CODE as usize] = vec![zero.clone(); 2];
            data[FullExitOp::OP_CODE as usize] = vec![zero.clone(); 2];
            data[ChangePubKeyOp::OP_CODE as usize] = vec![zero.clone(); 2];
            data[ForcedExitOp::OP_CODE as usize] = vec![zero.clone(); 2];

            // this operation is disabled for now
            // data[CloseOp::OP_CODE as usize] = vec![];

            data
        };

        assert_eq!(pubdata_holder.len(), DIFFERENT_TRANSACTIONS_TYPE_NUMBER);

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
                params::used_account_subtree_depth(),
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
                &mut last_token_id,
                &mut fees,
                &mut prev,
                &mut pubdata_holder,
                &zero,
            )?;
            let (new_state_root, _, _) = check_account_data(
                cs.namespace(|| "calculate new account root"),
                &current_branch,
                params::used_account_subtree_depth(),
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

        let validator_address_padded = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "validator_address"),
            || self.validator_address.grab(),
            params::ACCOUNT_ID_BIT_WIDTH,
        )?;

        let validator_address_bits = validator_address_padded.get_bits_le();
        assert_eq!(validator_address_bits.len(), params::ACCOUNT_ID_BIT_WIDTH);

        let mut validator_balances_processable_tokens = {
            assert_eq!(self.validator_balances.len(), params::total_tokens());
            for balance in &self.validator_balances[params::number_of_processable_tokens()..] {
                if let Some(ingored_tokens_balance) = balance {
                    assert!(ingored_tokens_balance.is_zero());
                }
            }
            let allocated_validator_balances = allocate_numbers_vec(
                cs.namespace(|| "validator_balances"),
                &self.validator_balances[..params::number_of_processable_tokens()],
            )?;
            assert_eq!(
                allocated_validator_balances.len(),
                params::number_of_processable_tokens()
            );
            allocated_validator_balances
        };

        let validator_audit_path = allocate_numbers_vec(
            cs.namespace(|| "validator_audit_path"),
            &self.validator_audit_path[..params::used_account_subtree_depth()],
        )?;
        assert_eq!(
            validator_audit_path.len(),
            params::used_account_subtree_depth()
        );

        let validator_account = AccountContent::from_witness(
            cs.namespace(|| "validator account"),
            &self.validator_account,
        )?;

        // calculate operator's balance_tree root hash from sub tree representation
        let old_operator_balance_root = calculate_balances_root_from_left_tree_values(
            cs.namespace(|| "calculate_root_from_full_representation_fees before"),
            &validator_balances_processable_tokens,
            params::balance_tree_depth(),
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
            params::used_account_subtree_depth(),
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
            validator_balances_processable_tokens[i] = allocate_sum(
                cs.namespace(|| format!("validator balance number i {}", i)),
                &validator_balances_processable_tokens[i],
                &fees[i],
            )?;
        }

        // calculate operator's balance_tree root hash from whole tree representation
        let new_operator_balance_root = calculate_balances_root_from_left_tree_values(
            cs.namespace(|| "calculate_root_from_full_representation_fees after"),
            &validator_balances_processable_tokens,
            params::balance_tree_depth(),
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
            params::used_account_subtree_depth(),
            self.rescue_params,
        )?;

        let final_root = continue_leftmost_subroot_to_root(
            cs.namespace(|| "continue subroot to root"),
            &root_from_operator_after_fees,
            params::used_account_subtree_depth(),
            params::account_tree_depth(),
            self.rescue_params,
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

            // Perform bit decomposition with an explicit in-field check
            // and change to MSB first bit order
            let old_root_be_bits = {
                let mut old_root_le_bits = old_root
                    .into_bits_le_strict(cs.namespace(|| "old root hash into LE bits strict"))?;
                assert_eq!(old_root_le_bits.len(), E::Fr::NUM_BITS as usize);
                resize_grow_only(&mut old_root_le_bits, 256, Boolean::constant(false));
                let mut old_root_be_bits = old_root_le_bits;
                old_root_be_bits.reverse();
                assert_eq!(old_root_be_bits.len(), 256);
                old_root_be_bits
            };
            pack_bits.extend(old_root_be_bits);

            hash_block = sha256::sha256(cs.namespace(|| "hash old_root"), &pack_bits)?;

            let mut pack_bits = vec![];
            pack_bits.extend(hash_block);

            // Perform bit decomposition with an explicit in-field check
            // and change to MSB first bit order
            let final_root_be_bits = {
                let mut final_root_le_bits = final_root
                    .into_bits_le_strict(cs.namespace(|| "final root hash into LE bits strict"))?;
                assert_eq!(final_root_le_bits.len(), E::Fr::NUM_BITS as usize);
                resize_grow_only(&mut final_root_le_bits, 256, Boolean::constant(false));
                let mut final_root_be_bits = final_root_le_bits;
                final_root_be_bits.reverse();
                assert_eq!(final_root_be_bits.len(), 256);
                final_root_be_bits
            };
            pack_bits.extend(final_root_be_bits);

            hash_block = sha256::sha256(cs.namespace(|| "hash with new_root"), &pack_bits)?;

            let mut pack_bits = vec![];
            pack_bits.extend(hash_block);
            pack_bits.extend(block_pub_data_bits.into_iter());

            hash_block = sha256::sha256(cs.namespace(|| "final hash public"), &pack_bits)?;

            // // now pack and enforce equality to the input

            hash_block.reverse();
            hash_block.truncate(E::Fr::CAPACITY as usize);

            let final_hash =
                pack_bits_to_element_strict(cs.namespace(|| "final_hash"), &hash_block)?;
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
impl<'a, E: RescueEngine + JubjubEngine> ZkSyncCircuit<'a, E> {
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

    // select a branch.
    // If TX type == deposit then select first branch
    // else if chunk number == 0 select first, else - select second
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

    #[allow(clippy::too_many_arguments)]
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
        last_token_id: &mut AllocatedNum<E>,
        fees: &mut [AllocatedNum<E>],
        prev: &mut PreviousData<E>,
        previous_pubdatas: &mut [Vec<AllocatedNum<E>>],
        explicit_zero: &AllocatedNum<E>,
    ) -> Result<(), SynthesisError> {
        let max_token_id =
            Expression::<E>::u64::<CS>(params::number_of_processable_tokens() as u64);
        cs.enforce(
            || "left and right tokens are equal",
            |lc| lc + lhs.token.get_number().get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + rhs.token.get_number().get_variable(),
        );

        let diff_token_numbers = max_token_id - Expression::from(&lhs.token.get_number());

        let _ = diff_token_numbers.into_bits_le_fixed(
            cs.namespace(|| "token number is smaller than processable number"),
            params::balance_tree_depth(),
        )?;

        let public_generator = self
            .jubjub_params
            .generator(FixedGenerators::SpendingKeyGenerator)
            .clone();

        let generator = ecc::EdwardsPoint::witness(
            cs.namespace(|| "allocate public generator"),
            Some(public_generator.clone()),
            self.jubjub_params,
        )?;
        let (public_generator_x, public_generator_y) = public_generator.into_xy();
        generator.get_x().assert_number(
            cs.namespace(|| "assert generator x is constant"),
            &public_generator_x,
        )?;
        generator.get_y().assert_number(
            cs.namespace(|| "assert generator y is constant"),
            &public_generator_y,
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
            &mut previous_pubdatas[DepositOp::OP_CODE as usize],
            &explicit_zero,
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
            &mut previous_pubdatas[TransferOp::OP_CODE as usize],
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
            &mut previous_pubdatas[TransferToNewOp::OP_CODE as usize],
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
            &mut previous_pubdatas[WithdrawOp::OP_CODE as usize],
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
            &mut previous_pubdatas[FullExitOp::OP_CODE as usize],
            &explicit_zero,
        )?);
        op_flags.push(self.change_pubkey_offchain(
            cs.namespace(|| "change_pubkey_offchain"),
            &lhs,
            &mut cur,
            &chunk_data,
            &op_data,
            &ext_pubdata_chunk,
            &mut previous_pubdatas[ChangePubKeyOp::OP_CODE as usize],
            &is_a_geq_b,
            &signature_data.is_verified,
            &signer_key,
        )?);
        op_flags.push(self.noop(
            cs.namespace(|| "noop"),
            &chunk_data,
            &ext_pubdata_chunk,
            &op_data,
            &mut previous_pubdatas[NoopOp::OP_CODE as usize],
            &explicit_zero,
        )?);
        op_flags.push(self.forced_exit(
            cs.namespace(|| "forced_exit"),
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
            &mut previous_pubdatas[ForcedExitOp::OP_CODE as usize],
        )?);

        assert_eq!(DIFFERENT_TRANSACTIONS_TYPE_NUMBER - 1, op_flags.len());

        let op_valid = multi_or(cs.namespace(|| "op_valid"), &op_flags)?;

        Boolean::enforce_equal(
            cs.namespace(|| "op_valid is true"),
            &op_valid,
            &Boolean::constant(true),
        )?;

        assert_eq!(
            fees.len(),
            params::number_of_processable_tokens(),
            "fees length is invalid"
        );

        // ensure that fee token only changes if it's in a first chunk.
        // First chunk is also always an LHS by "select-branch" function
        // There is always a signature on "cur" in the corresponding operations on the first chunk

        // if chunk_data.is_chunk_first we take value from the current chunk, else - we keep an
        // old value
        let new_last_token_id = AllocatedNum::conditionally_select(
            cs.namespace(|| {
                "ensure that token_id for token is only taken on the first
            chunk"
            }),
            &cur.token.get_number(),
            &last_token_id,
            &chunk_data.is_chunk_first,
        )?;

        *last_token_id = new_last_token_id.clone();

        for (i, fee) in fees.iter_mut().enumerate() {
            let sum = Expression::from(&*fee) + Expression::from(&op_data.fee.get_number());

            let is_token_correct = Boolean::from(Expression::equals(
                cs.namespace(|| format!("is token equal to number {}", i)),
                &new_last_token_id,
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
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
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

        resize_grow_only(
            &mut pubdata_bits,
            WithdrawOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

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
        assert_eq!(serialized_tx_bits.len(), params::SIGNED_WITHDRAW_BIT_WIDTH);

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            WithdrawOp::CHUNKS,
        )?;

        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_first_chunk,
            &is_equal_pubdata,
        )?;

        base_valid_flags.push(pubdata_properly_copied);

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
            &[is_signed_correctly, is_first_chunk.not()],
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
        log::debug!("lhs_valid_withdraw_begin");
        let lhs_valid = multi_and(cs.namespace(|| "is_lhs_valid"), &lhs_valid_flags)?;
        log::debug!("lhs_valid_withdraw_end");

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
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        explicit_zero: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        // Execute first chunk

        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        // MUST be true for all chunks
        let (is_pubdata_chunk_correct, pubdata_is_properly_copied) = {
            //construct pubdata
            let pubdata_bits = {
                let mut pub_data = Vec::new();
                pub_data.extend(chunk_data.tx_type.get_bits_be()); //1
                pub_data.extend(cur.account_id.get_bits_be()); //3
                pub_data.extend(op_data.eth_address.get_bits_be()); //20
                pub_data.extend(cur.token.get_bits_be()); // 2
                pub_data.extend(op_data.full_amount.get_bits_be());

                resize_grow_only(
                    &mut pub_data,
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

            let pubdata_chunk_correct = Boolean::from(Expression::equals(
                cs.namespace(|| "is_pubdata_equal"),
                &pubdata_chunk,
                ext_pubdata_chunk,
            )?);

            let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
                cs.namespace(|| "compare pubdata"),
                &*pubdata_holder,
                &pubdata_bits,
            )?;

            *pubdata_holder = packed_pubdata;

            let pubdata_properly_copied = boolean_or(
                cs.namespace(|| "first chunk or pubdata is copied properly"),
                &is_first_chunk,
                &is_equal_pubdata,
            )?;

            (pubdata_chunk_correct, pubdata_properly_copied)
        };

        let fee_is_zero = AllocatedNum::equals(
            cs.namespace(|| "fee is zero for full exit"),
            &op_data.fee.get_number(),
            &explicit_zero,
        )?;

        let fee_is_zero = Boolean::from(fee_is_zero);

        let is_base_valid = {
            let mut base_valid_flags = Vec::new();

            log::debug!(
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
            base_valid_flags.push(pubdata_is_properly_copied);
            base_valid_flags.push(fee_is_zero);
            multi_and(cs.namespace(|| "valid base full_exit"), &base_valid_flags)?
        };

        // SHOULD be true for successful exit
        let is_address_correct = CircuitElement::equals(
            cs.namespace(|| "is_address_correct"),
            &cur.account.address,
            &op_data.eth_address,
        )?;

        // MUST be true for the validity of the first chunk
        let is_pubdata_amount_valid = {
            let circuit_pubdata_amount = CircuitElement::conditionally_select_with_number_strict(
                cs.namespace(|| "pubdata_amount"),
                Expression::constant::<CS>(E::Fr::zero()),
                &cur.balance,
                &is_address_correct.not(),
            )?;

            CircuitElement::equals(
                cs.namespace(|| "is_pubdata_amount_correct"),
                &circuit_pubdata_amount,
                &op_data.full_amount,
            )?
        };

        // MUST be true for correct op. First chunk is correct and tree update can be executed.
        let first_chunk_valid = {
            let mut flags = Vec::new();
            flags.push(is_first_chunk.clone());
            flags.push(is_base_valid.clone());
            flags.push(no_nonce_overflow(
                cs.namespace(|| "no nonce overflow"),
                &cur.account.nonce.get_number(),
            )?);
            flags.push(is_pubdata_amount_valid);
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
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        explicit_zero: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        //construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
        pubdata_bits.extend(cur.account_id.get_bits_be()); //ACCOUNT_TREE_DEPTH=24
        pubdata_bits.extend(cur.token.get_bits_be()); //TOKEN_BIT_WIDTH=16
        pubdata_bits.extend(op_data.full_amount.get_bits_be()); //AMOUNT_PACKED=24
        pubdata_bits.extend(op_data.eth_address.get_bits_be()); //ETH_ADDRESS_BIT_WIDTH=160
        resize_grow_only(
            &mut pubdata_bits,
            DepositOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

        //useful below
        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let mut is_valid_flags = vec![];

        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_first_chunk,
            &is_equal_pubdata,
        )?;

        is_valid_flags.push(pubdata_properly_copied);

        let fee_is_zero = AllocatedNum::equals(
            cs.namespace(|| "fee is zero for deposit"),
            &op_data.fee.get_number(),
            &explicit_zero,
        )?;

        is_valid_flags.push(Boolean::from(fee_is_zero));

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

        let is_pubkey_correct = Boolean::xor(
            cs.namespace(|| "keys are same or account is empty"),
            &is_pub_equal_to_previous,
            &is_account_empty,
        )?;

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
        lhs: &AllocatedOperationBranch<E>,
        cur: &mut AllocatedOperationBranch<E>,
        chunk_data: &AllocatedChunkData<E>,
        op_data: &AllocatedOperationData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        is_a_geq_b: &Boolean,
        is_sig_verified: &Boolean,
        signer_key: &AllocatedSignerPubkey<E>,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        //construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
        pubdata_bits.extend(cur.account_id.get_bits_be()); //ACCOUNT_TREE_DEPTH=24
        pubdata_bits.extend(op_data.new_pubkey_hash.get_bits_be()); //ETH_KEY_BIT_WIDTH=160
        pubdata_bits.extend(op_data.eth_address.get_bits_be()); //ETH_KEY_BIT_WIDTH=160
                                                                // NOTE: nonce if verified implicitly here. Current account nonce goes to pubdata and to contract.
        pubdata_bits.extend(op_data.pub_nonce.get_bits_be());
        pubdata_bits.extend(cur.token.get_bits_be());
        pubdata_bits.extend(op_data.fee_packed.get_bits_be());

        resize_grow_only(
            &mut pubdata_bits,
            ChangePubKeyOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        // Construct serialized tx
        let mut serialized_tx_bits = vec![];

        serialized_tx_bits.extend(chunk_data.tx_type.get_bits_be());
        serialized_tx_bits.extend(cur.account_id.get_bits_be());
        serialized_tx_bits.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits.extend(op_data.new_pubkey_hash.get_bits_be());
        serialized_tx_bits.extend(cur.token.get_bits_be());
        serialized_tx_bits.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits.extend(cur.account.nonce.get_bits_be());

        assert_eq!(
            serialized_tx_bits.len(),
            params::SIGNED_CHANGE_PUBKEY_BIT_WIDTH
        );

        let is_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_serialized_tx_correct"),
            serialized_tx_bits,
            &op_data,
        )?;

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

        //useful below
        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let mut is_valid_flags = vec![];

        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_first_chunk,
            &is_equal_pubdata,
        )?;

        is_valid_flags.push(pubdata_properly_copied);

        // check operation arguments
        let is_a_correct =
            CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &lhs.balance)?;

        let fee_expr = Expression::from(&op_data.fee.get_number());
        let is_b_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_b_correct"),
            &op_data.b.get_number(),
            fee_expr.clone(),
        )?);

        is_valid_flags.push(is_a_correct);
        is_valid_flags.push(is_b_correct);
        is_valid_flags.push(is_a_geq_b.clone());

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

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correect"),
            &signer_key.pubkey.get_hash(),
            &op_data.new_pubkey_hash,
        )?;

        // Verify that zkSync signature corresponds to the public key in pubdata.
        let is_signed_correctly = multi_and(
            cs.namespace(|| "is_signed_correctly"),
            &[
                is_serialized_tx_correct,
                is_sig_verified.clone(),
                is_signer_valid,
            ],
        )?;

        let is_sig_correct = multi_or(
            cs.namespace(|| "sig is valid or not first chunk"),
            &[is_signed_correctly, is_first_chunk.not()],
        )?;

        is_valid_flags.push(is_sig_correct);

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

        let updated_balance = Expression::from(&cur.balance.get_number()) - fee_expr;

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

        //update balance
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance"),
            updated_balance,
            &cur.balance,
            &is_valid_first,
        )?;

        Ok(tx_valid)
    }

    fn noop<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        chunk_data: &AllocatedChunkData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        op_data: &AllocatedOperationData<E>,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        explicit_zero: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        assert_eq!(
            pubdata_holder.len(),
            0,
            "pubdata holder should be empty for no-op"
        );

        let mut is_valid_flags = vec![];
        //construct pubdata (it's all 0 for noop)
        let mut pubdata_bits = vec![];
        pubdata_bits.resize(params::CHUNK_BIT_WIDTH, Boolean::constant(false));

        // here pubdata is constant, so there is no check for a copy

        assert_eq!(
            NoopOp::CHUNKS,
            1,
            "no-op always takes one chunk as a padding op"
        );

        // don't need to check for proper copy cause it's always a first chunk

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

        let fee_is_zero = AllocatedNum::equals(
            cs.namespace(|| "fee is zero for no-op"),
            &op_data.fee.get_number(),
            &explicit_zero,
        )?;

        is_valid_flags.push(Boolean::from(fee_is_zero));

        let is_noop = Boolean::from(Expression::equals(
            cs.namespace(|| "is_noop"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(0), //noop tx_type
        )?);
        is_valid_flags.push(is_noop);

        let tx_valid = multi_and(cs.namespace(|| "is_tx_valid"), &is_valid_flags)?;

        Ok(tx_valid)
    }

    #[allow(clippy::too_many_arguments)]
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
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be()); //8
        pubdata_bits.extend(lhs.account_id.get_bits_be()); //24
        pubdata_bits.extend(cur.token.get_bits_be()); //16
        pubdata_bits.extend(op_data.amount_packed.get_bits_be()); //24
        pubdata_bits.extend(op_data.eth_address.get_bits_be()); //160
        pubdata_bits.extend(rhs.account_id.get_bits_be()); //24
        pubdata_bits.extend(op_data.fee_packed.get_bits_be()); //8
        resize_grow_only(
            &mut pubdata_bits,
            TransferToNewOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

        // construct signature message preimage (serialized_tx)
        let mut serialized_tx_bits = vec![];
        let tx_code = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "transfer_to_new_code_ce"),
            || Ok(E::Fr::from_str(&TransferOp::OP_CODE.to_string()).unwrap()),
            8,
        )?; //we use here transfer tx_code to allow user sign message without knowing whether it is transfer_to_new or transfer
        tx_code.get_number().assert_number(
            cs.namespace(|| "tx code is constant TransferOp"),
            &E::Fr::from_str(&TransferOp::OP_CODE.to_string()).unwrap(),
        )?;

        serialized_tx_bits.extend(tx_code.get_bits_be());
        serialized_tx_bits.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits.extend(lhs.account.address.get_bits_be());
        serialized_tx_bits.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits.extend(cur.token.get_bits_be());
        serialized_tx_bits.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits.extend(cur.account.nonce.get_bits_be());
        assert_eq!(serialized_tx_bits.len(), SIGNED_TRANSFER_BIT_WIDTH);

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

        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_first_chunk,
            &is_equal_pubdata,
        )?;

        lhs_valid_flags.push(pubdata_properly_copied.clone());

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
        log::debug!(
            "is_serialized_tx_correct: {:?}",
            is_serialized_tx_correct.get_value()
        );
        let is_signed_correctly = multi_and(
            cs.namespace(|| "is_signed_correctly"),
            &[is_serialized_tx_correct, is_sig_verified.clone()],
        )?;

        log::debug!("is_sig_verified: {:?}", is_sig_verified.get_value());

        let is_sig_correct = multi_or(
            cs.namespace(|| "sig is valid or not first chunk"),
            &[is_signed_correctly, is_first_chunk.not()],
        )?;
        lhs_valid_flags.push(is_sig_correct);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correect"),
            &signer_key.pubkey.get_hash(),
            &lhs.account.pub_key_hash,
        )?;
        log::debug!(
            "signer_key.pubkey.get_hash(): {:?}",
            signer_key.pubkey.get_hash().get_number().get_value()
        );
        log::debug!(
            "signer_key.pubkey.get_x(): {:?}",
            signer_key.pubkey.get_x().get_number().get_value()
        );

        log::debug!(
            "signer_key.pubkey.get_y(): {:?}",
            signer_key.pubkey.get_y().get_number().get_value()
        );

        log::debug!(
            "lhs.account.pub_key_hash: {:?}",
            lhs.account.pub_key_hash.get_number().get_value()
        );
        log::debug!("is_signer_valid: {:?}", is_signer_valid.get_value());

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
        rhs_valid_flags.push(pubdata_properly_copied.clone());

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
        ohs_valid_flags.push(pubdata_properly_copied);

        let is_ohs_valid = multi_and(cs.namespace(|| "is_ohs_valid"), &ohs_valid_flags)?;

        let is_op_valid = multi_or(
            cs.namespace(|| "is_op_valid"),
            &[is_ohs_valid, lhs_valid, rhs_valid],
        )?;
        Ok(is_op_valid)
    }

    #[allow(clippy::too_many_arguments)]
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
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be());
        pubdata_bits.extend(lhs.account_id.get_bits_be());
        pubdata_bits.extend(cur.token.get_bits_be());
        pubdata_bits.extend(rhs.account_id.get_bits_be());
        pubdata_bits.extend(op_data.amount_packed.get_bits_be());
        pubdata_bits.extend(op_data.fee_packed.get_bits_be());

        resize_grow_only(
            &mut pubdata_bits,
            TransferOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

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
        assert_eq!(serialized_tx_bits.len(), SIGNED_TRANSFER_BIT_WIDTH);

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

        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_first_chunk,
            &is_equal_pubdata,
        )?;

        lhs_valid_flags.push(pubdata_properly_copied.clone());

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
        rhs_valid_flags.push(pubdata_properly_copied);
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

        // Either LHS or RHS are correct (due to chunking at least)
        let correct = Boolean::xor(
            cs.namespace(|| "lhs_valid XOR rhs_valid"),
            &lhs_valid,
            &is_rhs_valid,
        )?;

        Ok(correct)
    }

    #[allow(clippy::too_many_arguments)]
    fn forced_exit<CS: ConstraintSystem<E>>(
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
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be());
        pubdata_bits.extend(lhs.account_id.get_bits_be());
        pubdata_bits.extend(rhs.account_id.get_bits_be());
        pubdata_bits.extend(cur.token.get_bits_be());
        pubdata_bits.extend(op_data.full_amount.get_bits_be());
        pubdata_bits.extend(op_data.fee_packed.get_bits_be());
        pubdata_bits.extend(op_data.eth_address.get_bits_be());

        resize_grow_only(
            &mut pubdata_bits,
            ForcedExitOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

        // construct signature message preimage (serialized_tx)

        let mut serialized_tx_bits = vec![];

        serialized_tx_bits.extend(chunk_data.tx_type.get_bits_be());
        serialized_tx_bits.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits.extend(rhs.account.address.get_bits_be());
        serialized_tx_bits.extend(cur.token.get_bits_be());
        serialized_tx_bits.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits.extend(lhs.account.nonce.get_bits_be());
        assert_eq!(serialized_tx_bits.len(), SIGNED_FORCED_EXIT_BIT_WIDTH);

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &chunk_data.chunk_number,
            ForcedExitOp::CHUNKS,
        )?;
        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_correct"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);

        // verify correct tx_code

        let is_forced_exit = Boolean::from(Expression::equals(
            cs.namespace(|| "is_forced_exit"),
            &chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(ForcedExitOp::OP_CODE)),
        )?);

        let mut lhs_valid_flags = vec![];

        lhs_valid_flags.push(is_pubdata_chunk_correct.clone());
        lhs_valid_flags.push(is_forced_exit.clone());

        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            Expression::constant::<CS>(E::Fr::zero()),
        )?);

        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_first_chunk,
            &is_equal_pubdata,
        )?;

        lhs_valid_flags.push(pubdata_properly_copied.clone());

        lhs_valid_flags.push(is_first_chunk.clone());

        // check operation arguments
        let is_a_correct =
            CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;

        lhs_valid_flags.push(is_a_correct);

        let fee_expr = Expression::from(&op_data.fee.get_number());

        let is_b_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_b_correct"),
            &op_data.b.get_number(),
            fee_expr.clone(),
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

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correct"),
            &signer_key.pubkey.get_hash(),
            &lhs.account.pub_key_hash,
        )?;
        lhs_valid_flags.push(is_signer_valid);

        let lhs_valid = multi_and(cs.namespace(|| "lhs_valid"), &lhs_valid_flags)?;

        let updated_balance = Expression::from(&cur.balance.get_number()) - fee_expr;

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
        rhs_valid_flags.push(pubdata_properly_copied.clone());
        rhs_valid_flags.push(is_forced_exit.clone());

        let is_second_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_chunk_second"),
            &chunk_data.chunk_number,
            Expression::u64::<CS>(1),
        )?);
        rhs_valid_flags.push(is_second_chunk.clone());
        rhs_valid_flags.push(is_account_empty.not());

        rhs_valid_flags.push(is_pubdata_chunk_correct.clone());

        let empty_pubkey_hash = Expression::constant::<CS>(E::Fr::zero());
        let allocated_pubkey_hash = rhs.account.pub_key_hash.clone().into_number();

        let is_rhs_signing_key_unset = Expression::equals(
            cs.namespace(|| "rhs_signing_key_unset"),
            &allocated_pubkey_hash,
            empty_pubkey_hash,
        )?;
        rhs_valid_flags.push(Boolean::from(is_rhs_signing_key_unset));

        // Check that the withdraw amount is equal to the rhs account balance.
        let is_rhs_balance_eq_amount = CircuitElement::equals(
            cs.namespace(|| "is_rhs_balance_eq_amount_correct"),
            &op_data.full_amount,
            &cur.balance,
        )?;
        rhs_valid_flags.push(is_rhs_balance_eq_amount);

        // Check that `eth_address` corresponds to the rhs account Ethereum address.
        let is_address_correct = CircuitElement::equals(
            cs.namespace(|| "is_address_correct"),
            &rhs.account.address,
            &op_data.eth_address,
        )?;
        rhs_valid_flags.push(is_address_correct);

        let rhs_valid = multi_and(cs.namespace(|| "is_rhs_valid"), &rhs_valid_flags)?;

        // calculate new rhs balance value
        let updated_balance = Expression::from(&cur.balance.get_number())
            - Expression::from(&op_data.full_amount.get_number());

        // update balance
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated_balance rhs"),
            updated_balance,
            &cur.balance,
            &rhs_valid,
        )?;

        // Remaining chunks
        let mut ohs_valid_flags = vec![];
        ohs_valid_flags.push(is_pubdata_chunk_correct);
        ohs_valid_flags.push(is_first_chunk.not());
        ohs_valid_flags.push(is_second_chunk.not());
        ohs_valid_flags.push(is_forced_exit);
        ohs_valid_flags.push(pubdata_properly_copied);

        let is_ohs_valid = multi_and(cs.namespace(|| "is_ohs_valid"), &ohs_valid_flags)?;

        let is_op_valid = multi_or(
            cs.namespace(|| "is_op_valid"),
            &[is_ohs_valid, lhs_valid, rhs_valid],
        )?;
        Ok(is_op_valid)
    }
}

pub fn check_account_data<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    cur: &AllocatedOperationBranch<E>,
    length_to_root: usize,
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
            length_to_root,
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
        params::balance_tree_depth(),
        params,
    )?;

    let mut account_data = vec![];
    account_data.extend(branch.account.nonce.get_bits_le());
    account_data.extend(branch.account.pub_key_hash.get_bits_le());
    account_data.extend(branch.account.address.get_bits_le());

    let account_data_packed_as_field_elements = multipack::pack_into_witness(
        cs.namespace(|| "pack account data to check if empty"),
        &account_data,
    )?;

    assert_eq!(account_data_packed_as_field_elements.len(), 2);

    let mut account_words_are_empty =
        Vec::with_capacity(account_data_packed_as_field_elements.len());

    for (i, el) in account_data_packed_as_field_elements
        .into_iter()
        .enumerate()
    {
        let is_word_empty = Expression::equals(
            cs.namespace(|| format!("is account word {} empty", i)),
            &el,
            Expression::constant::<CS>(E::Fr::zero()),
        )?;

        account_words_are_empty.push(Boolean::from(is_word_empty));
    }

    let is_account_empty = multi_and(
        cs.namespace(|| "check if all account words are empty"),
        &account_words_are_empty,
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

    Ok((account_data, is_account_empty, balance_subtree_root))
}

pub fn allocate_merkle_root<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    leaf_bits: &[Boolean],
    index: &[Boolean],
    audit_path: &[AllocatedNum<E>],
    length_to_root: usize,
    params: &E::Params,
) -> Result<AllocatedNum<E>, SynthesisError> {
    // only first bits of index are considered valuable
    assert!(length_to_root <= index.len());
    assert!(index.len() >= audit_path.len());

    let index = &index[0..length_to_root];
    let audit_path = &audit_path[0..length_to_root];

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
        let current_chunk = pack_bits_to_element_strict(
            cs.namespace(|| "chunk as field element"),
            &pub_chunk_bits,
        )?;

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

fn calculate_balances_root_from_left_tree_values<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    processable_fees: &[AllocatedNum<E>],
    tree_depth: usize,
    params: &E::Params,
) -> Result<AllocatedNum<E>, SynthesisError> {
    assert_eq!(
        processable_fees.len(),
        params::number_of_processable_tokens()
    );

    let processable_fee_hashes = processable_fees
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, fee)| {
            let cs = &mut cs.namespace(|| format!("fee hashing index number {}", index));

            fee.limit_number_of_bits(
                cs.namespace(|| "ensure that fees are short enough"),
                params::BALANCE_BIT_WIDTH,
            )?;

            let fee_hash = {
                let mut sponge_output = rescue::rescue_hash(
                    cs.namespace(|| "hash the fee leaf content"),
                    &[fee],
                    params,
                )?;
                assert_eq!(sponge_output.len(), 1);
                sponge_output.pop().expect("must get a single element")
            };

            Ok(fee_hash)
        })
        .collect::<Result<Vec<_>, SynthesisError>>()?;

    let processable_fees_tree_depth = processable_fees.len().trailing_zeros() as usize;
    assert_eq!(
        1 << processable_fees_tree_depth,
        params::number_of_processable_tokens()
    );

    // will hash non-empty part of the tree
    let mut hash_vec = processable_fee_hashes;
    for i in 0..processable_fees_tree_depth {
        let cs = &mut cs.namespace(|| format!("merkle tree level index number {}", i));
        assert!(hash_vec.len().is_power_of_two());
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

    let last_level_node_hash = hash_vec[0].clone();

    let empty_node_hashes = calculate_empty_balance_tree_hashes::<E>(params, tree_depth);
    let mut node_hash = last_level_node_hash;
    // will hash top of the tree where RHS is always an empty tree
    for i in processable_fees_tree_depth..params::balance_tree_depth() {
        let cs = &mut cs.namespace(|| format!("merkle tree level index number {}", i));
        let pair_value = empty_node_hashes[i - 1]; // we need value from previous level
        let pair = AllocatedNum::alloc(
            cs.namespace(|| format!("allocate empty node as num for level {}", i)),
            || Ok(pair_value),
        )?;

        pair.assert_number(
            cs.namespace(|| format!("assert pair at level {} is constant", i)),
            &pair_value,
        )?;

        let mut sponge_output = rescue::rescue_hash(
            cs.namespace(|| "perform smt hashing"),
            &[node_hash, pair],
            params,
        )?;
        assert_eq!(sponge_output.len(), 1, "must get a single element");
        node_hash = sponge_output.pop().unwrap();
    }

    Ok(node_hash)
}

fn continue_leftmost_subroot_to_root<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    subroot: &AllocatedNum<E>,
    subroot_is_at_level: usize,
    tree_depth: usize,
    params: &E::Params,
) -> Result<AllocatedNum<E>, SynthesisError> {
    let empty_node_hashes = calculate_empty_account_tree_hashes::<E>(params, tree_depth);
    let mut node_hash = subroot.clone();

    // will hash top of the tree where RHS is always an empty tree
    for i in subroot_is_at_level..tree_depth {
        let cs = &mut cs.namespace(|| format!("merkle tree level index number {}", i));
        let pair_value = empty_node_hashes[i - 1]; // we need value from previous level
        let pair = AllocatedNum::alloc(
            cs.namespace(|| format!("allocate empty node as num for level {}", i)),
            || Ok(pair_value),
        )?;

        pair.assert_number(
            cs.namespace(|| format!("assert pair at level {} is constant", i)),
            &pair_value,
        )?;

        let mut sponge_output = rescue::rescue_hash(
            cs.namespace(|| "perform smt hashing"),
            &[node_hash, pair],
            params,
        )?;

        assert_eq!(sponge_output.len(), 1);

        let tmp = sponge_output.pop().expect("must get a single element");
        node_hash = tmp;
    }

    Ok(node_hash)
}

fn generate_maxchunk_polynomial<E: JubjubEngine>() -> Vec<E::Fr> {
    use zksync_crypto::franklin_crypto::interpolation::interpolate;

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
    points.push(get_xy(ForcedExitOp::OP_CODE, ForcedExitOp::CHUNKS));

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
