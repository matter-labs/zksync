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
    self, CONTENT_HASH_WIDTH, FR_BIT_WIDTH_PADDED, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID,
    SIGNED_FORCED_EXIT_BIT_WIDTH, SIGNED_MINT_NFT_BIT_WIDTH, SIGNED_TRANSFER_BIT_WIDTH,
    SIGNED_WITHDRAW_NFT_BIT_WIDTH,
};
use zksync_types::{
    operations::{ChangePubKeyOp, NoopOp},
    tx::Order,
    CloseOp, DepositOp, ForcedExitOp, FullExitOp, MintNFTOp, SwapOp, TransferOp, TransferToNewOp,
    WithdrawNFTOp, WithdrawOp,
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
        resize_grow_only, sequences_equal, u8_into_bits_be, vectorized_compare,
    },
};

const DIFFERENT_TRANSACTIONS_TYPE_NUMBER: usize = 12;
pub struct ZkSyncCircuit<'a, E: RescueEngine + JubjubEngine> {
    pub rescue_params: &'a <E as RescueEngine>::Params,
    pub jubjub_params: &'a <E as JubjubEngine>::Params,
    /// The old root of the tree
    pub old_root: Option<E::Fr>,
    pub initial_used_subtree_root: Option<E::Fr>,

    pub block_number: Option<E::Fr>,
    pub validator_address: Option<E::Fr>,
    pub block_timestamp: Option<E::Fr>,

    pub pub_data_commitment: Option<E::Fr>,
    pub operations: Vec<Operation<E>>,

    pub validator_balances: Vec<Option<E::Fr>>,
    pub validator_audit_path: Vec<Option<E::Fr>>,
    pub validator_account: AccountWitness<E>,

    pub validator_non_processable_tokens_audit_before_fees: Vec<Option<E::Fr>>,
    pub validator_non_processable_tokens_audit_after_fees: Vec<Option<E::Fr>>,
}

pub struct CircuitGlobalVariables<E: RescueEngine + JubjubEngine> {
    pub explicit_zero: CircuitElement<E>,
    pub block_timestamp: CircuitElement<E>,
    pub chunk_data: AllocatedChunkData<E>,
    pub min_nft_token_id: CircuitElement<E>,
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
            block_timestamp: self.block_timestamp,
            pub_data_commitment: self.pub_data_commitment,
            operations: self.operations.clone(),

            validator_balances: self.validator_balances.clone(),
            validator_audit_path: self.validator_audit_path.clone(),
            validator_account: self.validator_account.clone(),

            validator_non_processable_tokens_audit_before_fees: self
                .validator_non_processable_tokens_audit_before_fees
                .clone(),
            validator_non_processable_tokens_audit_after_fees: self
                .validator_non_processable_tokens_audit_after_fees
                .clone(),
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

        // vector of flags indicating first chunk of onchain op that will be aggregated during block processing
        let mut block_onchain_op_commitment_bits = vec![];

        let block_timestamp = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "allocated_block_timestamp"),
            || self.block_timestamp.grab(),
            params::TIMESTAMP_BIT_WIDTH,
        )?;

        let min_nft_token_id_number =
            AllocatedNum::alloc(cs.namespace(|| "min_nft_token_id number"), || {
                Ok(E::Fr::from_str(&params::MIN_NFT_TOKEN_ID.to_string()).unwrap())
            })?;
        min_nft_token_id_number.assert_number(
            cs.namespace(|| "assert min_nft_token_id is a constant"),
            &E::Fr::from_str(&params::MIN_NFT_TOKEN_ID.to_string()).unwrap(),
        )?;
        let min_nft_token_id = CircuitElement::from_number_with_known_length(
            cs.namespace(|| "min_nft_token_id circuit element"),
            min_nft_token_id_number,
            params::TOKEN_BIT_WIDTH,
        )?;

        let chunk_data: AllocatedChunkData<E> = AllocatedChunkData {
            is_chunk_last: Boolean::constant(false),
            is_chunk_first: Boolean::constant(false),
            chunk_number: zero_circuit_element.get_number(),
            tx_type: zero_circuit_element.clone(),
        };

        let mut global_variables = CircuitGlobalVariables {
            block_timestamp,
            chunk_data,
            explicit_zero: zero_circuit_element,
            min_nft_token_id,
        };

        // we create a memory value for a token ID that is used to collect fees.
        // It is overwritten when we enter the first chunk of the op (that exposes sender
        // and defines a token in which transaction is valued). Later one (at the last chunk)
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
            data[FullExitOp::OP_CODE as usize] = vec![zero.clone(); 4];
            data[ChangePubKeyOp::OP_CODE as usize] = vec![zero.clone(); 2];
            data[ForcedExitOp::OP_CODE as usize] = vec![zero.clone(); 2];
            data[MintNFTOp::OP_CODE as usize] = vec![zero.clone(); 2];
            data[WithdrawNFTOp::OP_CODE as usize] = vec![zero.clone(); 4];
            data[SwapOp::OP_CODE as usize] = vec![zero; 2];

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

            global_variables.chunk_data = chunk_data;
            next_chunk_number = next_chunk;
            let operation_pub_data_chunk = CircuitElement::from_fe_with_known_length(
                cs.namespace(|| "operation_pub_data_chunk"),
                || operation.clone().pubdata_chunk.grab(),
                params::CHUNK_BIT_WIDTH,
            )?;
            block_pub_data_bits.extend(operation_pub_data_chunk.get_bits_le());
            {
                let is_onchain_operation = {
                    let onchain_op_codes = vec![
                        DepositOp::OP_CODE,
                        WithdrawOp::OP_CODE,
                        ForcedExitOp::OP_CODE,
                        FullExitOp::OP_CODE,
                        ChangePubKeyOp::OP_CODE,
                        WithdrawNFTOp::OP_CODE,
                    ];

                    let mut onchain_op_flags = Vec::new();
                    for code in onchain_op_codes {
                        onchain_op_flags.push(Boolean::from(Expression::equals(
                            cs.namespace(|| format!("is_chunk_onchain_op_code_{}", code)),
                            &global_variables.chunk_data.tx_type.get_number(),
                            Expression::u64::<CS>(u64::from(code)),
                        )?));
                    }
                    multi_or(cs.namespace(|| "is_chunk_onchain_op"), &onchain_op_flags)?
                };
                let should_set_onchain_commitment_flag = Boolean::and(
                    cs.namespace(|| "is_first_chunk_oncahin_op"),
                    &is_onchain_operation,
                    &global_variables.chunk_data.is_chunk_first,
                )?;

                block_onchain_op_commitment_bits
                    .extend_from_slice(&vec![Boolean::constant(false); 7]);
                block_onchain_op_commitment_bits.push(should_set_onchain_commitment_flag);
            }

            let lhs =
                AllocatedOperationBranch::from_witness(cs.namespace(|| "lhs"), &operation.lhs)?;
            let rhs =
                AllocatedOperationBranch::from_witness(cs.namespace(|| "rhs"), &operation.rhs)?;
            let mut current_branch = self.select_branch(
                cs.namespace(|| "select appropriate branch"),
                &lhs,
                &rhs,
                operation,
                &global_variables.chunk_data,
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

            let is_special_nft_storage_account = Boolean::from(Expression::equals(
                cs.namespace(|| "is_special_nft_storage_account"),
                &current_branch.account_id.get_number(),
                Expression::u64::<CS>(NFT_STORAGE_ACCOUNT_ID.0.into()),
            )?);
            let is_special_nft_token = Boolean::from(Expression::equals(
                cs.namespace(|| "is_special_nft_token"),
                &current_branch.token.get_number(),
                Expression::u64::<CS>(NFT_TOKEN_ID.0.into()),
            )?);

            self.execute_op(
                cs.namespace(|| "execute_op"),
                &mut current_branch,
                &lhs,
                &rhs,
                &operation,
                &global_variables,
                &is_account_empty,
                &operation_pub_data_chunk.get_number(),
                // &subtree_root, // Close disable
                &mut last_token_id,
                &mut fees,
                &mut prev,
                &mut pubdata_holder,
                &is_special_nft_storage_account,
                &is_special_nft_token,
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
                global_variables
                    .chunk_data
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

        let mut validator_balances_processable_tokens = allocate_numbers_vec(
            cs.namespace(|| "validator_balances"),
            &self.validator_balances,
        )?;

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

        // calculate operator's balance_tree root hash from processable tokens balances full representation
        let validator_non_processable_tokens_audit_before_fees = allocate_numbers_vec(
            cs.namespace(|| "validator_non_processable_tokens_audit_before_fees"),
            &self.validator_non_processable_tokens_audit_before_fees,
        )?;
        let old_operator_balance_root = calculate_validator_root_from_processable_values(
            cs.namespace(|| "calculate_validator_root_from_processable_values before fees"),
            &validator_balances_processable_tokens,
            &validator_non_processable_tokens_audit_before_fees,
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

        // calculate operator's balance_tree root hash from processable tokens balances full representation
        let validator_non_processable_tokens_audit_after_fees = allocate_numbers_vec(
            cs.namespace(|| "validator_non_processable_tokens_audit_after_fees"),
            &self.validator_non_processable_tokens_audit_after_fees,
        )?;
        let new_operator_balance_root = calculate_validator_root_from_processable_values(
            cs.namespace(|| "calculate_validator_root_from_processable_values after fees"),
            &validator_balances_processable_tokens,
            &validator_non_processable_tokens_audit_after_fees,
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

            initial_hash_data.extend(block_number.into_padded_be_bits(FR_BIT_WIDTH_PADDED));

            initial_hash_data
                .extend(validator_address_padded.into_padded_be_bits(FR_BIT_WIDTH_PADDED));

            assert_eq!(initial_hash_data.len(), FR_BIT_WIDTH_PADDED * 2);

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
                resize_grow_only(
                    &mut old_root_le_bits,
                    FR_BIT_WIDTH_PADDED,
                    Boolean::constant(false),
                );
                let mut old_root_be_bits = old_root_le_bits;
                old_root_be_bits.reverse();
                assert_eq!(old_root_be_bits.len(), FR_BIT_WIDTH_PADDED);
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
                resize_grow_only(
                    &mut final_root_le_bits,
                    FR_BIT_WIDTH_PADDED,
                    Boolean::constant(false),
                );
                let mut final_root_be_bits = final_root_le_bits;
                final_root_be_bits.reverse();
                assert_eq!(final_root_be_bits.len(), FR_BIT_WIDTH_PADDED);
                final_root_be_bits
            };
            pack_bits.extend(final_root_be_bits);

            hash_block = sha256::sha256(cs.namespace(|| "hash with new_root"), &pack_bits)?;

            let mut pack_bits = vec![];
            pack_bits.extend(hash_block);
            pack_bits.extend(
                global_variables
                    .block_timestamp
                    .into_padded_be_bits(FR_BIT_WIDTH_PADDED),
            );
            assert_eq!(pack_bits.len(), FR_BIT_WIDTH_PADDED * 2);

            hash_block = sha256::sha256(cs.namespace(|| "hash with timestamp"), &pack_bits)?;

            let mut pack_bits = vec![];
            pack_bits.extend(hash_block);
            pack_bits.extend(block_pub_data_bits.into_iter());
            pack_bits.extend(block_onchain_op_commitment_bits.into_iter());

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
    // If TX type == swap then select first if chunk number is even, else second
    // else if chunk number == 0 select first, else - select second
    fn select_branch<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        first: &AllocatedOperationBranch<E>,
        second: &AllocatedOperationBranch<E>,
        _op: &Operation<E>,
        chunk_data: &AllocatedChunkData<E>,
    ) -> Result<AllocatedOperationBranch<E>, SynthesisError> {
        let deposit_tx_type = Expression::u64::<CS>(DepositOp::OP_CODE.into());
        let swap_tx_type = Expression::u64::<CS>(SwapOp::OP_CODE.into());
        let left_side = Expression::constant::<CS>(E::Fr::zero());

        let cur_side = Expression::select_ifeq(
            cs.namespace(|| "select corresponding branch - if deposit"),
            &chunk_data.tx_type.get_number(),
            deposit_tx_type,
            left_side.clone(),
            &chunk_data.chunk_number,
        )?;

        let chunk_number_bits = chunk_data
            .chunk_number
            .into_bits_le_fixed(cs.namespace(|| "chunk number into bits"), 8)?;
        let chunk_number_last_bit = Expression::boolean::<CS>(chunk_number_bits[0].clone());

        let cur_side = Expression::select_ifeq(
            cs.namespace(|| "select corresponding branch - if swap"),
            &chunk_data.tx_type.get_number(),
            swap_tx_type,
            chunk_number_last_bit,
            &cur_side,
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
        global_variables: &CircuitGlobalVariables<E>,
        is_account_empty: &Boolean,
        ext_pubdata_chunk: &AllocatedNum<E>,
        // subtree_root: &CircuitElement<E>, // Close disable
        last_token_id: &mut AllocatedNum<E>,
        fees: &mut [AllocatedNum<E>],
        prev: &mut PreviousData<E>,
        previous_pubdatas: &mut [Vec<AllocatedNum<E>>],
        is_special_nft_storage_account: &Boolean,
        is_special_nft_token: &Boolean,
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

        let is_swap = Boolean::from(Expression::equals(
            cs.namespace(|| "is_swap"),
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(SwapOp::OP_CODE)),
        )?);

        // ensure op_data is equal to previous
        {
            let a_and_b_same_as_previous_flags = vec![
                CircuitElement::equals(
                    cs.namespace(|| "is a equal to previous"),
                    &op_data.a,
                    &prev.op_data.a,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is b equal to previous"),
                    &op_data.b,
                    &prev.op_data.b,
                )?,
            ];

            let a_and_b_same_as_previous = multi_and(
                cs.namespace(|| "a and b are equal to previous"),
                &a_and_b_same_as_previous_flags,
            )?;

            let is_op_data_correct_flags = vec![
                boolean_or(
                    cs.namespace(|| "a and b are equal to previous or op == swap"),
                    &a_and_b_same_as_previous,
                    &is_swap,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is amount_packed equal to previous"),
                    &op_data.amount_packed,
                    &prev.op_data.amount_packed,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is second_amount_packed equal to previous"),
                    &op_data.second_amount_packed,
                    &prev.op_data.second_amount_packed,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is fee_packed equal to previous"),
                    &op_data.fee_packed,
                    &prev.op_data.fee_packed,
                )?,
                sequences_equal(
                    cs.namespace(|| "are special_amounts_packed equal to previous"),
                    &op_data.special_amounts_packed,
                    &prev.op_data.special_amounts_packed,
                )?,
                sequences_equal(
                    cs.namespace(|| "are special_eth_addresses equal to previous"),
                    &op_data.special_eth_addresses,
                    &prev.op_data.special_eth_addresses,
                )?,
                sequences_equal(
                    cs.namespace(|| "are special_nonces equal to previous"),
                    &op_data.special_nonces,
                    &prev.op_data.special_nonces,
                )?,
                sequences_equal(
                    cs.namespace(|| "are special_tokens equal to previous"),
                    &op_data.special_tokens,
                    &prev.op_data.special_tokens,
                )?,
                sequences_equal(
                    cs.namespace(|| "are special_accounts equal to previous"),
                    &op_data.special_accounts,
                    &prev.op_data.special_accounts,
                )?,
                sequences_equal(
                    cs.namespace(|| "are special_prices equal to previous"),
                    &op_data.special_prices,
                    &prev.op_data.special_prices,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is eth_address equal to previous"),
                    &op_data.eth_address,
                    &prev.op_data.eth_address,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is new_pubkey_hash equal to previous"),
                    &op_data.new_pubkey_hash,
                    &prev.op_data.new_pubkey_hash,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is full_amount equal to previous"),
                    &op_data.full_amount,
                    &prev.op_data.full_amount,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is valid_from equal to previous"),
                    &op_data.valid_from,
                    &prev.op_data.valid_from,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is valid_until equal to previous"),
                    &op_data.valid_until,
                    &prev.op_data.valid_until,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is second_valid_from equal to previous"),
                    &op_data.second_valid_from,
                    &prev.op_data.second_valid_from,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is second_valid_until equal to previous"),
                    &op_data.second_valid_until,
                    &prev.op_data.second_valid_until,
                )?,
                sequences_equal(
                    cs.namespace(|| "special_eth_addresses"),
                    &op_data.special_eth_addresses,
                    &prev.op_data.special_eth_addresses,
                )?,
                sequences_equal(
                    cs.namespace(|| "special_tokens"),
                    &op_data.special_tokens,
                    &prev.op_data.special_tokens,
                )?,
                sequences_equal(
                    cs.namespace(|| "special_content_hash"),
                    &op_data.special_content_hash,
                    &prev.op_data.special_content_hash,
                )?,
                CircuitElement::equals(
                    cs.namespace(|| "is special_serial_id equal to previous"),
                    &op_data.special_serial_id,
                    &prev.op_data.special_serial_id,
                )?,
            ];

            let is_op_data_equal_to_previous = multi_and(
                cs.namespace(|| "is_op_data_equal_to_previous"),
                &is_op_data_correct_flags,
            )?;

            let is_op_data_correct = multi_or(
                cs.namespace(|| "is_op_data_correct"),
                &[
                    is_op_data_equal_to_previous,
                    global_variables.chunk_data.is_chunk_first.clone(),
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

        let is_valid_timestamp = self.verify_operation_timestamp(
            cs.namespace(|| "verify operation timestamp"),
            &op_data,
            &global_variables,
        )?;

        let nft_content_as_balance = hash_nft_content_to_balance_type(
            cs.namespace(|| "nft_content_as_balance"),
            &op_data.special_accounts[0],  // creator_account_id
            &op_data.special_serial_id,    // serial_id
            &op_data.special_content_hash, // content_hash
            self.rescue_params,
        )?;

        let is_fungible_token = CircuitElement::less_than_fixed(
            cs.namespace(|| "is_fungible_token"),
            &cur.token,
            &global_variables.min_nft_token_id,
        )?;

        let op_flags = vec![
            self.deposit(
                cs.namespace(|| "deposit"),
                &mut cur,
                global_variables,
                &is_account_empty,
                &op_data,
                &ext_pubdata_chunk,
                &mut previous_pubdatas[DepositOp::OP_CODE as usize],
            )?,
            self.transfer(
                cs.namespace(|| "transfer"),
                &mut cur,
                &lhs,
                &rhs,
                global_variables,
                &is_a_geq_b,
                &is_account_empty,
                &op_data,
                &signer_key,
                &ext_pubdata_chunk,
                &is_valid_timestamp,
                &signature_data.is_verified,
                &mut previous_pubdatas[TransferOp::OP_CODE as usize],
                is_special_nft_storage_account,
                is_special_nft_token,
                &is_fungible_token,
            )?,
            self.transfer_to_new(
                cs.namespace(|| "transfer_to_new"),
                &mut cur,
                &lhs,
                &rhs,
                global_variables,
                &is_a_geq_b,
                &is_account_empty,
                &op_data,
                &signer_key,
                &ext_pubdata_chunk,
                &is_valid_timestamp,
                &signature_data.is_verified,
                &mut previous_pubdatas[TransferToNewOp::OP_CODE as usize],
                is_special_nft_storage_account,
                is_special_nft_token,
                &is_fungible_token,
            )?,
            self.withdraw(
                cs.namespace(|| "withdraw"),
                &mut cur,
                global_variables,
                &is_a_geq_b,
                &op_data,
                &signer_key,
                &ext_pubdata_chunk,
                &is_valid_timestamp,
                &signature_data.is_verified,
                &mut previous_pubdatas[WithdrawOp::OP_CODE as usize],
                is_special_nft_storage_account,
                is_special_nft_token,
                &is_fungible_token,
            )?,
            // Close disable.
            // self.close_account(
            //      cs.namespace(|| "close_account"),
            //      &mut cur,
            //      &chunk_data,
            //      &ext_pubdata_chunk,
            //      &op_data,
            //      &signer_key,
            //      &subtree_root,
            //      &is_valid_timestamp,
            //      &signature_data.is_verified,
            // )?,
            self.full_exit(
                cs.namespace(|| "full_exit"),
                &mut cur,
                global_variables,
                &op_data,
                &ext_pubdata_chunk,
                &mut previous_pubdatas[FullExitOp::OP_CODE as usize],
                &nft_content_as_balance,
                is_special_nft_storage_account,
                is_special_nft_token,
                &is_fungible_token,
            )?,
            self.change_pubkey_offchain(
                cs.namespace(|| "change_pubkey_offchain"),
                &lhs,
                &mut cur,
                global_variables,
                &op_data,
                &ext_pubdata_chunk,
                &is_valid_timestamp,
                &mut previous_pubdatas[ChangePubKeyOp::OP_CODE as usize],
                &is_a_geq_b,
                &signature_data.is_verified,
                &signer_key,
                is_special_nft_storage_account,
                is_special_nft_token,
                &is_fungible_token,
            )?,
            self.noop(
                cs.namespace(|| "noop"),
                global_variables,
                &ext_pubdata_chunk,
                &op_data,
                &mut previous_pubdatas[NoopOp::OP_CODE as usize],
            )?,
            self.forced_exit(
                cs.namespace(|| "forced_exit"),
                &mut cur,
                &lhs,
                &rhs,
                global_variables,
                &is_a_geq_b,
                &is_account_empty,
                &op_data,
                &signer_key,
                &ext_pubdata_chunk,
                &is_valid_timestamp,
                &signature_data.is_verified,
                &mut previous_pubdatas[ForcedExitOp::OP_CODE as usize],
                is_special_nft_storage_account,
                is_special_nft_token,
                &is_fungible_token,
            )?,
            self.mint_nft(
                cs.namespace(|| "mint_nft"),
                &mut cur,
                global_variables,
                &is_a_geq_b,
                &is_account_empty,
                &op_data,
                &signer_key,
                &ext_pubdata_chunk,
                &signature_data.is_verified,
                &mut previous_pubdatas[MintNFTOp::OP_CODE as usize],
                &nft_content_as_balance,
                is_special_nft_storage_account,
                is_special_nft_token,
            )?,
            self.withdraw_nft(
                cs.namespace(|| "withdraw_nft"),
                &mut cur,
                global_variables,
                &is_a_geq_b,
                &op_data,
                &signer_key,
                &ext_pubdata_chunk,
                &is_valid_timestamp,
                &signature_data.is_verified,
                &mut previous_pubdatas[WithdrawNFTOp::OP_CODE as usize],
                &nft_content_as_balance,
                is_special_nft_storage_account,
                is_special_nft_token,
            )?,
            self.swap(
                cs.namespace(|| "swap"),
                &mut cur,
                global_variables,
                &is_a_geq_b,
                &is_account_empty,
                &op_data,
                &signer_key,
                &ext_pubdata_chunk,
                &is_valid_timestamp,
                &signature_data.is_verified,
                &mut previous_pubdatas[SwapOp::OP_CODE as usize],
                is_special_nft_storage_account,
                is_special_nft_token,
            )?,
        ];

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
            &global_variables.chunk_data.is_chunk_first,
        )?;

        // if TX type == swap then update the token on the last chunk
        let swap_and_last_chunk = Boolean::and(
            cs.namespace(|| "last chunk of swap tx"),
            &is_swap,
            &global_variables.chunk_data.is_chunk_last,
        )?;
        let new_last_token_id = AllocatedNum::conditionally_select(
            cs.namespace(|| "change token_id on last chunk of swap tx"),
            &cur.token.get_number(),
            &new_last_token_id,
            &swap_and_last_chunk,
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
                &global_variables.chunk_data.is_chunk_last.clone(),
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

    #[allow(clippy::too_many_arguments)]
    fn withdraw<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        global_variables: &CircuitGlobalVariables<E>,
        is_a_geq_b: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_valid_timestamp: &Boolean,
        is_sig_verified: &Boolean,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        _is_special_nft_storage_account: &Boolean,
        is_special_nft_token: &Boolean,
        is_fungible_token: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        let mut base_valid_flags = vec![];
        // construct pubdata
        let mut pubdata_bits = vec![];

        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
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

        let mut serialized_tx_bits_version1 = vec![];
        serialized_tx_bits_version1.extend(reversed_tx_type_bits_be(WithdrawOp::OP_CODE));
        serialized_tx_bits_version1.extend(u8_into_bits_be(params::CURRENT_TX_VERSION));
        serialized_tx_bits_version1.extend(cur.account_id.get_bits_be());
        serialized_tx_bits_version1.extend(cur.account.address.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits_version1.extend(cur.token.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.full_amount.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_version1.extend(cur.account.nonce.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_until.get_bits_be());
        assert_eq!(
            serialized_tx_bits_version1.len(),
            params::SIGNED_WITHDRAW_BIT_WIDTH
        );

        let mut serialized_tx_bits_old1 = vec![];
        serialized_tx_bits_old1.extend(global_variables.chunk_data.tx_type.get_bits_be());
        serialized_tx_bits_old1.extend(cur.account_id.get_bits_be());
        serialized_tx_bits_old1.extend(cur.account.address.get_bits_be());
        serialized_tx_bits_old1.extend(op_data.eth_address.get_bits_be());
        // the old version contains token 2-byte representation
        serialized_tx_bits_old1.extend_from_slice(&cur.token.get_bits_be()[16..32]);
        serialized_tx_bits_old1.extend(op_data.full_amount.get_bits_be());
        serialized_tx_bits_old1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_old1.extend(cur.account.nonce.get_bits_be());
        assert_eq!(
            serialized_tx_bits_old1.len(),
            params::OLD1_SIGNED_WITHDRAW_BIT_WIDTH
        );

        let mut serialized_tx_bits_old2 = vec![];
        serialized_tx_bits_old2.extend(global_variables.chunk_data.tx_type.get_bits_be());
        serialized_tx_bits_old2.extend(cur.account_id.get_bits_be());
        serialized_tx_bits_old2.extend(cur.account.address.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.eth_address.get_bits_be());
        // the old version contains token 2-byte representation
        serialized_tx_bits_old2.extend_from_slice(&cur.token.get_bits_be()[16..32]);
        serialized_tx_bits_old2.extend(op_data.full_amount.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_old2.extend(cur.account.nonce.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.valid_until.get_bits_be());
        assert_eq!(
            serialized_tx_bits_old2.len(),
            params::OLD2_SIGNED_WITHDRAW_BIT_WIDTH
        );

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &global_variables.chunk_data.chunk_number,
            WithdrawOp::CHUNKS,
        )?;

        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &global_variables.chunk_data.chunk_number,
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
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(WithdrawOp::OP_CODE)),
        )?);
        base_valid_flags.push(is_withdraw);
        base_valid_flags.push(is_valid_timestamp.clone());

        let is_version1_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_version1_serialized_tx_correct"),
            serialized_tx_bits_version1,
            &op_data,
        )?;

        let mut is_old1_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_old1_serialized_tx_correct"),
            serialized_tx_bits_old1,
            &op_data,
        )?;
        is_old1_serialized_tx_correct = multi_and(
            cs.namespace(|| "is_old1_serialized_tx_correct and fungible"),
            &[is_old1_serialized_tx_correct, is_fungible_token.clone()],
        )?;

        let mut is_old2_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_old2_serialized_tx_correct"),
            serialized_tx_bits_old2,
            &op_data,
        )?;
        is_old2_serialized_tx_correct = multi_and(
            cs.namespace(|| "is_old2_serialized_tx_correct and fungible"),
            &[is_old2_serialized_tx_correct, is_fungible_token.clone()],
        )?;

        let is_serialized_tx_correct = multi_or(
            cs.namespace(|| "is_serialized_tx_correct"),
            &[
                is_version1_serialized_tx_correct,
                is_old1_serialized_tx_correct,
                is_old2_serialized_tx_correct,
            ],
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

        base_valid_flags.push(is_fungible_token.clone());

        let is_base_valid = multi_and(cs.namespace(|| "valid base withdraw"), &base_valid_flags)?;

        let mut lhs_valid_flags = vec![
            is_first_chunk.clone(),
            is_base_valid.clone(),
            is_special_nft_token.not(),
        ];

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
        vlog::debug!("lhs_valid_withdraw_begin");
        let lhs_valid = multi_and(cs.namespace(|| "is_lhs_valid"), &lhs_valid_flags)?;
        vlog::debug!("lhs_valid_withdraw_end");

        let ohs_valid_flags = vec![is_base_valid, is_first_chunk.not()];
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
        global_variables: &CircuitGlobalVariables<E>,
        op_data: &AllocatedOperationData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        nft_content_as_balance: &CircuitElement<E>,
        is_special_nft_storage_account: &Boolean,
        _is_special_nft_token: &Boolean,
        is_fungible_token: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );
        /*
        fields specification:

        special:
        special_eth_addresses = [creator_address]
        special_accounts = [creator_account_id, account_id]
        */

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be()); // tx_type = 1 byte
        pubdata_bits.extend(op_data.special_accounts[1].get_bits_be()); // account_id = 4 bytes
        pubdata_bits.extend(op_data.eth_address.get_bits_be()); // initiator_address = 20 bytes
        pubdata_bits.extend(cur.token.get_bits_be()); // token_id = 4 bytes
        pubdata_bits.extend(op_data.full_amount.get_bits_be()); // full_amount = 16 bytes
        pubdata_bits.extend(op_data.special_accounts[0].get_bits_be()); // creator_account_id = 4 bytes
        pubdata_bits.extend(op_data.special_eth_addresses[0].get_bits_be()); // creator_address = 20 bytes
        pubdata_bits.extend(op_data.special_serial_id.get_bits_be()); // serial_id = 4 bytes
        pubdata_bits.extend(
            op_data
                .special_content_hash
                .iter()
                .map(|bit| bit.get_bits_be())
                .flatten(),
        ); // content_hash = 32 bytes
        resize_grow_only(
            &mut pubdata_bits,
            FullExitOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

        let is_chunk_with_index: Vec<Boolean> = (0..3)
            .map(|chunk_index| {
                Expression::equals(
                    cs.namespace(|| format!("is_chunk_with_index {}", chunk_index)),
                    &global_variables.chunk_data.chunk_number,
                    Expression::u64::<CS>(chunk_index as u64),
                )
            })
            .collect::<Result<Vec<_>, SynthesisError>>()?
            .iter()
            .map(|bit| Boolean::from(bit.clone()))
            .collect();

        // common valid flags
        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_chunk_with_index[0].clone(),
            &is_equal_pubdata,
        )?;
        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &global_variables.chunk_data.chunk_number,
            FullExitOp::CHUNKS,
        )?;
        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_equal"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);
        let fee_is_zero = CircuitElement::equals(
            cs.namespace(|| "fee_is_zero"),
            &op_data.fee,
            &global_variables.explicit_zero,
        )?;
        let is_full_exit_operation = Boolean::from(Expression::equals(
            cs.namespace(|| "is_full_exit_operation"),
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(FullExitOp::OP_CODE)),
        )?);

        let common_valid = multi_and(
            cs.namespace(|| "is_common_valid"),
            &[
                pubdata_properly_copied,
                is_pubdata_chunk_correct,
                fee_is_zero,
                is_full_exit_operation,
            ],
        )?;

        let full_amount_equals_to_zero = CircuitElement::equals(
            cs.namespace(|| "full_amount_equals_to_zero"),
            &op_data.full_amount,
            &global_variables.explicit_zero,
        )?;

        let first_chunk_valid = {
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[0].clone()];

            let is_initiator_account = CircuitElement::equals(
                cs.namespace(|| "is_initiator_account"),
                &op_data.special_accounts[1],
                &cur.account_id,
            )?;
            flags.push(is_initiator_account);

            let is_full_exit_success = CircuitElement::equals(
                cs.namespace(|| "is_full_exit_success"),
                &op_data.eth_address,
                &cur.account.address,
            )?;
            let real_full_amount = CircuitElement::conditionally_select_with_number_strict(
                cs.namespace(|| "real_full_amount"),
                Expression::constant::<CS>(E::Fr::zero()),
                &cur.balance,
                &is_full_exit_success.not(),
            )?;
            flags.push(CircuitElement::equals(
                cs.namespace(|| "real_full_amount equals to declared in op_data"),
                &real_full_amount,
                &op_data.full_amount,
            )?);

            multi_and(cs.namespace(|| "first_chunk_valid"), &flags)?
        };
        let updated_balance = Expression::from(&cur.balance.get_number())
            - Expression::from(&op_data.full_amount.get_number());
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance"),
            updated_balance,
            &cur.balance,
            &first_chunk_valid,
        )?;

        let second_chunk_valid = {
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[1].clone()];

            flags.push(is_special_nft_storage_account.clone());

            let is_nft_stored_content_valid = CircuitElement::equals(
                cs.namespace(|| "is_nft_stored_content_valid"),
                &nft_content_as_balance,
                &cur.balance,
            )?;
            flags.push(multi_or(
                cs.namespace(|| "is_nft_content_correct"),
                &[
                    full_amount_equals_to_zero.clone(),
                    is_nft_stored_content_valid,
                    is_fungible_token.clone(),
                ],
            )?);

            multi_and(cs.namespace(|| "second_chunk_valid"), &flags)?
        };

        let third_chunk_valid = {
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[2].clone()];

            let is_creator_account = CircuitElement::equals(
                cs.namespace(|| "is_creator_account"),
                &op_data.special_accounts[0],
                &cur.account_id,
            )?;
            flags.push(is_creator_account);

            let creator_address_valid = CircuitElement::equals(
                cs.namespace(|| "creator_address_valid"),
                &op_data.special_eth_addresses[0],
                &cur.account.address,
            )?;
            flags.push(multi_or(
                cs.namespace(|| "is_creator_address_correct"),
                &[
                    full_amount_equals_to_zero,
                    creator_address_valid,
                    is_fungible_token.clone(),
                ],
            )?);

            multi_and(cs.namespace(|| "third_chunk_valid"), &flags)?
        };

        let ohs_valid = multi_and(
            cs.namespace(|| "ohs_valid"),
            &[
                common_valid,
                is_chunk_with_index[0].not(),
                is_chunk_with_index[1].not(),
                is_chunk_with_index[2].not(),
            ],
        )?;

        multi_or(
            cs.namespace(|| "is_full_exit_valid"),
            &[
                first_chunk_valid,
                second_chunk_valid,
                third_chunk_valid,
                ohs_valid,
            ],
        )
    }

    fn deposit<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        global_variables: &CircuitGlobalVariables<E>,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
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
            &global_variables.chunk_data.chunk_number,
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
            &global_variables.explicit_zero.get_number(),
        )?;

        is_valid_flags.push(Boolean::from(fee_is_zero));

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &global_variables.chunk_data.chunk_number,
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
            &global_variables.chunk_data.tx_type.get_number(),
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
            cs.namespace(|| "keys are same xor account is empty"),
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

    #[allow(clippy::too_many_arguments)]
    fn change_pubkey_offchain<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        lhs: &AllocatedOperationBranch<E>,
        cur: &mut AllocatedOperationBranch<E>,
        global_variables: &CircuitGlobalVariables<E>,
        op_data: &AllocatedOperationData<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_valid_timestamp: &Boolean,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        is_a_geq_b: &Boolean,
        is_sig_verified: &Boolean,
        signer_key: &AllocatedSignerPubkey<E>,
        _is_special_nft_storage_account: &Boolean,
        is_special_nft_token: &Boolean,
        is_fungible_token: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be()); //TX_TYPE_BIT_WIDTH=8
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
        let mut serialized_tx_bits_version1 = vec![];
        serialized_tx_bits_version1.extend(reversed_tx_type_bits_be(ChangePubKeyOp::OP_CODE));
        serialized_tx_bits_version1.extend(u8_into_bits_be(params::CURRENT_TX_VERSION));
        serialized_tx_bits_version1.extend(cur.account_id.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.new_pubkey_hash.get_bits_be());
        serialized_tx_bits_version1.extend(cur.token.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_version1.extend(cur.account.nonce.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_until.get_bits_be());
        assert_eq!(
            serialized_tx_bits_version1.len(),
            params::SIGNED_CHANGE_PUBKEY_BIT_WIDTH
        );

        // Construct serialized tx
        let mut serialized_tx_bits_old1 = vec![];
        serialized_tx_bits_old1.extend(global_variables.chunk_data.tx_type.get_bits_be());
        serialized_tx_bits_old1.extend(cur.account_id.get_bits_be());
        serialized_tx_bits_old1.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits_old1.extend(op_data.new_pubkey_hash.get_bits_be());
        // the old version contains token 2-byte representation
        serialized_tx_bits_old1.extend_from_slice(&cur.token.get_bits_be()[16..32]);
        serialized_tx_bits_old1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_old1.extend(cur.account.nonce.get_bits_be());
        assert_eq!(
            serialized_tx_bits_old1.len(),
            params::OLD1_SIGNED_CHANGE_PUBKEY_BIT_WIDTH
        );

        // Construct serialized tx
        let mut serialized_tx_bits_old2 = vec![];
        serialized_tx_bits_old2.extend(global_variables.chunk_data.tx_type.get_bits_be());
        serialized_tx_bits_old2.extend(cur.account_id.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.new_pubkey_hash.get_bits_be());
        // the old version contains token 2-byte representation
        serialized_tx_bits_old2.extend_from_slice(&cur.token.get_bits_be()[16..32]);
        serialized_tx_bits_old2.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_old2.extend(cur.account.nonce.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.valid_until.get_bits_be());
        assert_eq!(
            serialized_tx_bits_old2.len(),
            params::OLD2_SIGNED_CHANGE_PUBKEY_BIT_WIDTH
        );

        let is_version1_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_version1_serialized_tx_correct"),
            serialized_tx_bits_version1,
            &op_data,
        )?;

        let mut is_old1_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_old1_serialized_tx_correct"),
            serialized_tx_bits_old1,
            &op_data,
        )?;
        is_old1_serialized_tx_correct = multi_and(
            cs.namespace(|| "is_old1_serialized_tx_correct and fungible"),
            &[is_old1_serialized_tx_correct, is_fungible_token.clone()],
        )?;

        let mut is_old2_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_old2_serialized_tx_correct"),
            serialized_tx_bits_old2,
            &op_data,
        )?;
        is_old2_serialized_tx_correct = multi_and(
            cs.namespace(|| "is_old2_serialized_tx_correct and fungible"),
            &[is_old2_serialized_tx_correct, is_fungible_token.clone()],
        )?;

        let is_serialized_tx_correct = multi_or(
            cs.namespace(|| "is_serialized_tx_correct"),
            &[
                is_version1_serialized_tx_correct,
                is_old1_serialized_tx_correct,
                is_old2_serialized_tx_correct,
            ],
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
            &global_variables.chunk_data.chunk_number,
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
            &global_variables.chunk_data.chunk_number,
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
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(ChangePubKeyOp::OP_CODE)),
        )?);
        is_valid_flags.push(is_change_pubkey_offchain);

        is_valid_flags.push(is_valid_timestamp.clone());

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
                is_special_nft_token.not(),
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
        global_variables: &CircuitGlobalVariables<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        op_data: &AllocatedOperationData<E>,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
    ) -> Result<Boolean, SynthesisError> {
        assert_eq!(
            pubdata_holder.len(),
            0,
            "pubdata holder should be empty for no-op"
        );

        let mut is_valid_flags = vec![];
        // construct pubdata (it's all 0 for noop)
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
            &global_variables.chunk_data.chunk_number,
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
            &global_variables.explicit_zero.get_number(),
        )?;

        is_valid_flags.push(Boolean::from(fee_is_zero));

        let is_noop = Boolean::from(Expression::equals(
            cs.namespace(|| "is_noop"),
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(0), //noop tx_type
        )?);
        is_valid_flags.push(is_noop);

        let tx_valid = multi_and(cs.namespace(|| "is_tx_valid"), &is_valid_flags)?;

        Ok(tx_valid)
    }

    #[allow(clippy::too_many_arguments)]
    fn mint_nft<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        global_variables: &CircuitGlobalVariables<E>,
        is_a_geq_b: &Boolean,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_sig_verified: &Boolean,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        nft_content_as_balance: &CircuitElement<E>,
        is_special_nft_storage_account: &Boolean,
        is_special_nft_token: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );
        /*
        fields specification:

        special:
        special_eth_addresses = [recipient_address]
        special_tokens = [fee_token, new_token]
        special_accounts = [creator_account_id, recipient_account_id]
        special_content_hash = vector of bits of the content hash
        special_serial_id = serial_id of the NFT from this creator
        */

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be()); // tx_type = 1 byte
        pubdata_bits.extend(op_data.special_accounts[0].get_bits_be()); // creator_account_id = 4 bytes
        pubdata_bits.extend(op_data.special_accounts[1].get_bits_be()); // recipient_account_id = 4 bytes
        pubdata_bits.extend(
            op_data
                .special_content_hash
                .iter()
                .map(|bit| bit.get_bits_be())
                .flatten(),
        ); // content_hash = 32 bytes
        pubdata_bits.extend(op_data.special_tokens[0].get_bits_be()); // fee_token = 4 bytes
        pubdata_bits.extend(op_data.fee_packed.get_bits_be()); // fee = 2 bytes
        resize_grow_only(
            &mut pubdata_bits,
            MintNFTOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

        let is_chunk_with_index: Vec<Boolean> = (0u64..MintNFTOp::CHUNKS as u64)
            .map(|chunk_index| {
                Expression::equals(
                    cs.namespace(|| format!("is_chunk_with_index {}", chunk_index)),
                    &global_variables.chunk_data.chunk_number,
                    Expression::u64::<CS>(chunk_index),
                )
            })
            .collect::<Result<Vec<_>, SynthesisError>>()?
            .iter()
            .map(|bit| Boolean::from(bit.clone()))
            .collect();

        // common valid flags
        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_chunk_with_index[0].clone(),
            &is_equal_pubdata,
        )?;
        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &global_variables.chunk_data.chunk_number,
            MintNFTOp::CHUNKS,
        )?;
        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_equal"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);
        let is_mint_nft_operation = Boolean::from(Expression::equals(
            cs.namespace(|| "is_mint_nft_operation"),
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(MintNFTOp::OP_CODE)),
        )?);

        let common_valid = multi_and(
            cs.namespace(|| "is_common_valid"),
            &[
                pubdata_properly_copied,
                is_pubdata_chunk_correct,
                is_mint_nft_operation,
            ],
        )?;

        // used in first and second chunk
        let is_creator_account = CircuitElement::equals(
            cs.namespace(|| "is_creator_account"),
            &op_data.special_accounts[0],
            &cur.account_id,
        )?;
        // used in fourth and fifth chunk
        let is_new_token = CircuitElement::equals(
            cs.namespace(|| "is_new_token"),
            &op_data.special_tokens[1],
            &cur.token,
        )?;

        let first_chunk_valid = {
            // First chunk should take a fee from creator account and increment nonce.
            // Here will be checked signature of the creator.
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[0].clone()];

            let mut serialized_tx_bits_version1 = vec![];
            serialized_tx_bits_version1.extend(reversed_tx_type_bits_be(MintNFTOp::OP_CODE)); // reversed_tx_type
            serialized_tx_bits_version1.extend(u8_into_bits_be(params::CURRENT_TX_VERSION)); // signature scheme identificator
            serialized_tx_bits_version1.extend(op_data.special_accounts[0].get_bits_be()); // creator_id
            serialized_tx_bits_version1.extend(cur.account.address.get_bits_be()); // creator_address
            serialized_tx_bits_version1.extend(
                op_data
                    .special_content_hash
                    .iter()
                    .map(|bit| bit.get_bits_be())
                    .flatten(),
            ); // content_hash
            serialized_tx_bits_version1.extend(op_data.special_eth_addresses[0].get_bits_be()); // recipient_address
            serialized_tx_bits_version1.extend(cur.token.get_bits_be()); // fee token
            serialized_tx_bits_version1.extend(op_data.fee_packed.get_bits_be()); // fee
            serialized_tx_bits_version1.extend(cur.account.nonce.get_bits_be()); // nonce
            assert_eq!(serialized_tx_bits_version1.len(), SIGNED_MINT_NFT_BIT_WIDTH);

            let is_serialized_tx_correct = verify_signature_message_construction(
                cs.namespace(|| "is_serialized_tx_correct"),
                serialized_tx_bits_version1,
                &op_data,
            )?;
            flags.push(is_serialized_tx_correct);
            let is_signer_valid = CircuitElement::equals(
                cs.namespace(|| "signer_key_correct"),
                &signer_key.pubkey.get_hash(),
                &cur.account.pub_key_hash,
            )?;
            flags.push(is_signer_valid);
            flags.push(is_sig_verified.clone());

            flags.push(is_creator_account.clone());
            // We should enforce that fee_token value that is used in pubdata (op_data.special_tokens[0])
            // is equal to the token used in the first chunk and signed by the creator
            let is_fee_token = CircuitElement::equals(
                cs.namespace(|| "is_fee_token"),
                &op_data.special_tokens[0],
                &cur.token,
            )?;
            flags.push(is_fee_token);
            flags.push(is_special_nft_token.not());

            let is_a_correct =
                CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;
            let is_b_correct =
                CircuitElement::equals(cs.namespace(|| "is_b_correct"), &op_data.b, &op_data.fee)?;
            flags.push(is_a_correct);
            flags.push(is_b_correct);
            flags.push(is_a_geq_b.clone());
            flags.push(no_nonce_overflow(
                cs.namespace(|| "no nonce overflow"),
                &cur.account.nonce.get_number(),
            )?);

            multi_and(cs.namespace(|| "first_chunk_valid"), &flags)?
        };
        let updated_nonce_first_chunk =
            Expression::from(&cur.account.nonce.get_number()) + Expression::u64::<CS>(1);
        let updated_balance_first_chunk = Expression::from(&cur.balance.get_number())
            - Expression::from(&op_data.fee.get_number());
        cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "update cur nonce (first chunk)"),
            updated_nonce_first_chunk,
            &cur.account.nonce,
            &first_chunk_valid,
        )?;
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance (first chunk)"),
            updated_balance_first_chunk,
            &cur.balance,
            &first_chunk_valid,
        )?;

        let second_chunk_valid = {
            // Second chunk should enforce the validity of serial_id of creator account.
            // Also here serial_id counter of the creator account will be incremented.
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[1].clone()];

            flags.push(is_creator_account);
            flags.push(is_special_nft_token.clone());
            let valid_serial_id = CircuitElement::equals(
                cs.namespace(|| "valid_serial_id"),
                &op_data.special_serial_id,
                &cur.balance,
            )?;
            flags.push(valid_serial_id);

            multi_and(cs.namespace(|| "second_chunk_valid"), &flags)?
        };
        let updated_balance_second_chunk =
            Expression::from(&cur.balance.get_number()) + Expression::u64::<CS>(1);
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance (second chunk)"),
            updated_balance_second_chunk,
            &cur.balance,
            &second_chunk_valid,
        )?;

        let third_chunk_valid = {
            // Third chunk should enforce the validity of new_token_id value.
            // Also here nft counter of the special account will be incremented.
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[2].clone()];

            flags.push(is_special_nft_storage_account.clone());
            flags.push(is_special_nft_token.clone());
            let is_new_token_id_valid = CircuitElement::equals(
                cs.namespace(|| "is_new_token_id_valid"),
                &op_data.special_tokens[1],
                &cur.balance,
            )?;
            flags.push(is_new_token_id_valid);

            multi_and(cs.namespace(|| "third_chunk_valid"), &flags)?
        };
        let updated_balance_third_chunk =
            Expression::from(&cur.balance.get_number()) + Expression::u64::<CS>(1);
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance (third chunk)"),
            updated_balance_third_chunk,
            &cur.balance,
            &third_chunk_valid,
        )?;

        let fourth_chunk_valid = {
            // Fourth chunk should store nft content to the corresponding leaf of the special account.
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[3].clone()];

            flags.push(is_special_nft_storage_account.clone());
            flags.push(is_new_token.clone());
            flags.push(is_special_nft_token.not()); // all possible NFT slots are filled

            multi_and(cs.namespace(|| "fourth_chunk_valid"), &flags)?
        };
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance (fourth chunk)"),
            Expression::from(&nft_content_as_balance.get_number()),
            &cur.balance,
            &fourth_chunk_valid,
        )?;

        let fifth_chunk_valid = {
            // Fifth chunk should increment the balance of the recipient.
            let mut flags = vec![common_valid, is_chunk_with_index[4].clone()];

            let is_recipient_account = CircuitElement::equals(
                cs.namespace(|| "is_recipient_account"),
                &op_data.special_accounts[1],
                &cur.account_id,
            )?;
            flags.push(is_recipient_account);
            flags.push(is_special_nft_storage_account.not());
            let is_recipient_address = CircuitElement::equals(
                cs.namespace(|| "is_recipient_address"),
                &op_data.special_eth_addresses[0],
                &cur.account.address,
            )?;
            flags.push(is_recipient_address);
            flags.push(is_new_token);
            flags.push(is_account_empty.not());

            multi_and(cs.namespace(|| "fifth_chunk_valid"), &flags)?
        };
        let updated_balance_fifth_chunk =
            Expression::from(&cur.balance.get_number()) + Expression::u64::<CS>(1);
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance (fifth chunk)"),
            updated_balance_fifth_chunk,
            &cur.balance,
            &fifth_chunk_valid,
        )?;

        multi_or(
            cs.namespace(|| "is_mintNFT_valid"),
            &[
                first_chunk_valid,
                second_chunk_valid,
                third_chunk_valid,
                fourth_chunk_valid,
                fifth_chunk_valid,
            ],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn withdraw_nft<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        global_variables: &CircuitGlobalVariables<E>,
        is_a_geq_b: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_valid_timestamp: &Boolean,
        is_sig_verified: &Boolean,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        nft_content_as_balance: &CircuitElement<E>,
        is_special_nft_storage_account: &Boolean,
        is_special_nft_token: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );
        /*
        fields specification:
        eth_address = to_address

        special:
        special_eth_addresses = [creator_address]
        special_tokens = [fee_token, token]
        special_accounts = [creator_account_id, initiator_account_id]
        special_content_hash = vector of bits of the content hash
        special_serial_id = serial_id of the NFT from this creator
        */

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be()); // tx_type = 1 byte
        pubdata_bits.extend(op_data.special_accounts[1].get_bits_be()); // initiator_account_id = 4 bytes
        pubdata_bits.extend(op_data.special_accounts[0].get_bits_be()); // creator_account_id = 4 bytes
        pubdata_bits.extend(op_data.special_eth_addresses[0].get_bits_be()); // creator_address = 20 bytes
        pubdata_bits.extend(op_data.special_serial_id.get_bits_be()); // serial_id = 4 bytes
        pubdata_bits.extend(
            op_data
                .special_content_hash
                .iter()
                .map(|bit| bit.get_bits_be())
                .flatten(),
        ); // content_hash = 32 bytes
        pubdata_bits.extend(op_data.eth_address.get_bits_be()); // to_address = 20 bytes
        pubdata_bits.extend(op_data.special_tokens[1].get_bits_be()); // token = 4 bytes
        pubdata_bits.extend(op_data.special_tokens[0].get_bits_be()); // fee_token = 4 bytes
        pubdata_bits.extend(op_data.fee_packed.get_bits_be()); // fee = 2 bytes
        resize_grow_only(
            &mut pubdata_bits,
            WithdrawNFTOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

        let is_chunk_with_index: Vec<Boolean> = (0..4)
            .map(|chunk_index| {
                Expression::equals(
                    cs.namespace(|| format!("is_chunk_with_index {}", chunk_index)),
                    &global_variables.chunk_data.chunk_number,
                    Expression::u64::<CS>(chunk_index as u64),
                )
            })
            .collect::<Result<Vec<_>, SynthesisError>>()?
            .iter()
            .map(|bit| Boolean::from(bit.clone()))
            .collect();

        // common valid flags
        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_chunk_with_index[0].clone(),
            &is_equal_pubdata,
        )?;
        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &global_variables.chunk_data.chunk_number,
            WithdrawNFTOp::CHUNKS,
        )?;
        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_equal"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);
        let is_withdraw_nft_operation = Boolean::from(Expression::equals(
            cs.namespace(|| "is_withdraw_nft_operation"),
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(WithdrawNFTOp::OP_CODE)),
        )?);

        let common_valid = multi_and(
            cs.namespace(|| "is_common_valid"),
            &[
                pubdata_properly_copied,
                is_pubdata_chunk_correct,
                is_withdraw_nft_operation,
                is_valid_timestamp.clone(),
            ],
        )?;

        // used in first and second chunk
        let is_initiator_account = CircuitElement::equals(
            cs.namespace(|| "is_initiator_account"),
            &op_data.special_accounts[1],
            &cur.account_id,
        )?;
        // used in second and third chunk
        let is_token_to_withdraw = CircuitElement::equals(
            cs.namespace(|| "is_token_to_withdraw"),
            &op_data.special_tokens[1],
            &cur.token,
        )?;

        let first_chunk_valid = {
            // First chunk should take a fee from initiator account and increment nonce.
            // Here will be checked signature of the initiator.
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[0].clone()];

            let mut serialized_tx_bits_version1 = vec![];
            serialized_tx_bits_version1.extend(reversed_tx_type_bits_be(WithdrawNFTOp::OP_CODE)); // reversed_tx_type
            serialized_tx_bits_version1.extend(u8_into_bits_be(params::CURRENT_TX_VERSION)); // signature scheme identificator
            serialized_tx_bits_version1.extend(op_data.special_accounts[1].get_bits_be()); // initiator_id
            serialized_tx_bits_version1.extend(cur.account.address.get_bits_be()); // initiator_address
            serialized_tx_bits_version1.extend(op_data.eth_address.get_bits_be()); // to_address
            serialized_tx_bits_version1.extend(op_data.special_tokens[1].get_bits_be()); // token
            serialized_tx_bits_version1.extend(cur.token.get_bits_be()); // fee_token
            serialized_tx_bits_version1.extend(op_data.fee_packed.get_bits_be()); // fee
            serialized_tx_bits_version1.extend(cur.account.nonce.get_bits_be()); // nonce
            serialized_tx_bits_version1.extend(op_data.valid_from.get_bits_be()); // valid_from
            serialized_tx_bits_version1.extend(op_data.valid_until.get_bits_be()); // valid_until
            assert_eq!(
                serialized_tx_bits_version1.len(),
                SIGNED_WITHDRAW_NFT_BIT_WIDTH
            );

            let is_serialized_tx_correct = verify_signature_message_construction(
                cs.namespace(|| "is_serialized_tx_correct"),
                serialized_tx_bits_version1,
                &op_data,
            )?;
            flags.push(is_serialized_tx_correct);
            let is_signer_valid = CircuitElement::equals(
                cs.namespace(|| "signer_key_correct"),
                &signer_key.pubkey.get_hash(),
                &cur.account.pub_key_hash,
            )?;
            flags.push(is_signer_valid);
            flags.push(is_sig_verified.clone());

            flags.push(is_initiator_account.clone());
            // We should enforce that fee_token value that is used in pubdata (op_data.special_tokens[0])
            // is equal to the token used in the first chunk and signed by the creator
            let is_fee_token = CircuitElement::equals(
                cs.namespace(|| "is_fee_token"),
                &op_data.special_tokens[0],
                &cur.token,
            )?;
            flags.push(is_fee_token);
            flags.push(is_special_nft_token.not());

            let is_a_correct =
                CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;
            let is_b_correct =
                CircuitElement::equals(cs.namespace(|| "is_b_correct"), &op_data.b, &op_data.fee)?;
            flags.push(is_a_correct);
            flags.push(is_b_correct);
            flags.push(is_a_geq_b.clone());
            flags.push(no_nonce_overflow(
                cs.namespace(|| "no nonce overflow"),
                &cur.account.nonce.get_number(),
            )?);

            multi_and(cs.namespace(|| "first_chunk_valid"), &flags)?
        };
        let updated_nonce_first_chunk =
            Expression::from(&cur.account.nonce.get_number()) + Expression::u64::<CS>(1);
        let updated_balance_first_chunk = Expression::from(&cur.balance.get_number())
            - Expression::from(&op_data.fee.get_number());
        cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "update cur nonce (first chunk)"),
            updated_nonce_first_chunk,
            &cur.account.nonce,
            &first_chunk_valid,
        )?;
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance (first chunk)"),
            updated_balance_first_chunk,
            &cur.balance,
            &first_chunk_valid,
        )?;

        let second_chunk_valid = {
            // Second chunk should nullify the balance of the initiator.
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[1].clone()];

            flags.push(is_initiator_account);
            flags.push(is_token_to_withdraw.clone());
            let is_balance_valid = Boolean::from(Expression::equals(
                cs.namespace(|| "is_balance_valid"),
                &cur.balance.get_number(),
                Expression::u64::<CS>(1),
            )?);
            flags.push(is_balance_valid);

            multi_and(cs.namespace(|| "second_chunk_valid"), &flags)?
        };
        let updated_balance_second_chunk = Expression::u64::<CS>(0);
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance (second chunk)"),
            updated_balance_second_chunk,
            &cur.balance,
            &second_chunk_valid,
        )?;

        let third_chunk_valid = {
            // Third chunk should enforce the validity of creator account id and content hash values.
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[2].clone()];

            flags.push(is_special_nft_storage_account.clone());
            flags.push(is_token_to_withdraw);
            let stored_content_valid = CircuitElement::equals(
                cs.namespace(|| "stored_content_valid"),
                &nft_content_as_balance,
                &cur.balance,
            )?;
            flags.push(stored_content_valid);

            multi_and(cs.namespace(|| "third_chunk_valid"), &flags)?
        };

        let fourth_chunk_valid = {
            // Fourth chunk should enforce the validity of creator account address.
            let mut flags = vec![common_valid.clone(), is_chunk_with_index[3].clone()];

            let is_creator_account = CircuitElement::equals(
                cs.namespace(|| "is_creator_account"),
                &op_data.special_accounts[0],
                &cur.account_id,
            )?;
            flags.push(is_creator_account);
            let creator_address_valid = CircuitElement::equals(
                cs.namespace(|| "creator_address_valid"),
                &op_data.special_eth_addresses[0],
                &cur.account.address,
            )?;
            flags.push(creator_address_valid);

            multi_and(cs.namespace(|| "fourth_chunk_valid"), &flags)?
        };

        let ohs_valid = multi_and(
            cs.namespace(|| "ohs_valid"),
            &[
                common_valid,
                is_chunk_with_index[0].not(),
                is_chunk_with_index[1].not(),
                is_chunk_with_index[2].not(),
                is_chunk_with_index[3].not(),
            ],
        )?;

        multi_or(
            cs.namespace(|| "is_withdrawNFT_valid"),
            &[
                first_chunk_valid,
                second_chunk_valid,
                third_chunk_valid,
                fourth_chunk_valid,
                ohs_valid,
            ],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn transfer_to_new<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        lhs: &AllocatedOperationBranch<E>,
        rhs: &AllocatedOperationBranch<E>,
        global_variables: &CircuitGlobalVariables<E>,
        is_a_geq_b: &Boolean,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_valid_timestamp: &Boolean,
        is_sig_verified: &Boolean,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        is_special_nft_storage_account: &Boolean,
        is_special_nft_token: &Boolean,
        is_fungible_token: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        let mut pubdata_bits = vec![];
        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be()); //8
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
        // we use here transfer tx_code to allow user sign message without knowing whether it is transfer_to_new or transfer

        let mut serialized_tx_bits_version1 = vec![];
        serialized_tx_bits_version1.extend(reversed_tx_type_bits_be(TransferOp::OP_CODE));
        serialized_tx_bits_version1.extend(u8_into_bits_be(params::CURRENT_TX_VERSION));
        serialized_tx_bits_version1.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits_version1.extend(lhs.account.address.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits_version1.extend(cur.token.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_version1.extend(cur.account.nonce.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_until.get_bits_be());
        assert_eq!(
            serialized_tx_bits_version1.len(),
            params::SIGNED_TRANSFER_BIT_WIDTH
        );

        let mut serialized_tx_bits_old1 = vec![];
        serialized_tx_bits_old1.extend(u8_into_bits_be(TransferOp::OP_CODE));
        serialized_tx_bits_old1.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits_old1.extend(lhs.account.address.get_bits_be());
        serialized_tx_bits_old1.extend(op_data.eth_address.get_bits_be());
        // the old version contains token 2-byte representation
        serialized_tx_bits_old1.extend_from_slice(&cur.token.get_bits_be()[16..32]);
        serialized_tx_bits_old1.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits_old1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_old1.extend(cur.account.nonce.get_bits_be());
        assert_eq!(
            serialized_tx_bits_old1.len(),
            params::OLD1_SIGNED_TRANSFER_BIT_WIDTH
        );

        let mut serialized_tx_bits_old2 = vec![];
        serialized_tx_bits_old2.extend(u8_into_bits_be(TransferOp::OP_CODE));
        serialized_tx_bits_old2.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits_old2.extend(lhs.account.address.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.eth_address.get_bits_be());
        // the old version contains token 2-byte representation
        serialized_tx_bits_old2.extend_from_slice(&cur.token.get_bits_be()[16..32]);
        serialized_tx_bits_old2.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_old2.extend(cur.account.nonce.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.valid_until.get_bits_be());
        assert_eq!(
            serialized_tx_bits_old2.len(),
            params::OLD2_SIGNED_TRANSFER_BIT_WIDTH
        );

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &global_variables.chunk_data.chunk_number,
            TransferToNewOp::CHUNKS,
        )?;
        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_correct"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);

        let mut lhs_valid_flags = vec![is_pubdata_chunk_correct.clone()];

        let is_transfer = Boolean::from(Expression::equals(
            cs.namespace(|| "is_transfer"),
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(TransferToNewOp::OP_CODE)),
        )?);
        lhs_valid_flags.push(is_transfer.clone());
        lhs_valid_flags.push(is_valid_timestamp.clone());

        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &global_variables.chunk_data.chunk_number,
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

        let is_version1_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_version1_serialized_tx_correct"),
            serialized_tx_bits_version1,
            &op_data,
        )?;

        let mut is_old1_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_old1_serialized_tx_correct"),
            serialized_tx_bits_old1,
            &op_data,
        )?;
        is_old1_serialized_tx_correct = multi_and(
            cs.namespace(|| "is_old1_serialized_tx_correct and fungible"),
            &[is_old1_serialized_tx_correct, is_fungible_token.clone()],
        )?;

        let mut is_old2_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_old2_serialized_tx_correct"),
            serialized_tx_bits_old2,
            &op_data,
        )?;
        is_old2_serialized_tx_correct = multi_and(
            cs.namespace(|| "is_old2_serialized_tx_correct and fungible"),
            &[is_old2_serialized_tx_correct, is_fungible_token.clone()],
        )?;

        let is_serialized_tx_correct = multi_or(
            cs.namespace(|| "is_serialized_tx_correct"),
            &[
                is_version1_serialized_tx_correct,
                is_old1_serialized_tx_correct,
                is_old2_serialized_tx_correct,
            ],
        )?;

        vlog::debug!(
            "is_serialized_tx_correct: {:?}",
            is_serialized_tx_correct.get_value()
        );
        let is_signed_correctly = multi_and(
            cs.namespace(|| "is_signed_correctly"),
            &[is_serialized_tx_correct, is_sig_verified.clone()],
        )?;

        vlog::debug!("is_sig_verified: {:?}", is_sig_verified.get_value());

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
        vlog::debug!(
            "signer_key.pubkey.get_hash(): {:?}",
            signer_key.pubkey.get_hash().get_number().get_value()
        );
        vlog::debug!(
            "signer_key.pubkey.get_x(): {:?}",
            signer_key.pubkey.get_x().get_number().get_value()
        );

        vlog::debug!(
            "signer_key.pubkey.get_y(): {:?}",
            signer_key.pubkey.get_y().get_number().get_value()
        );

        vlog::debug!(
            "lhs.account.pub_key_hash: {:?}",
            lhs.account.pub_key_hash.get_number().get_value()
        );
        vlog::debug!("is_signer_valid: {:?}", is_signer_valid.get_value());

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

        let is_second_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_second_chunk"),
            &global_variables.chunk_data.chunk_number,
            Expression::u64::<CS>(1),
        )?);
        let rhs_valid_flags = vec![
            is_pubdata_chunk_correct.clone(),
            is_second_chunk.clone(),
            is_transfer.clone(),
            is_valid_timestamp.clone(),
            is_account_empty.clone(),
            pubdata_properly_copied.clone(),
            is_special_nft_storage_account.not(),
            is_special_nft_token.not(),
        ];
        let rhs_valid = multi_and(cs.namespace(|| "rhs_valid"), &rhs_valid_flags)?;

        cur.balance = CircuitElement::conditionally_select(
            cs.namespace(|| "mutated balance"),
            &op_data.amount_unpacked,
            &cur.balance,
            &rhs_valid,
        )?;
        cur.balance
            .enforce_length(cs.namespace(|| "mutated balance is still correct length"))?; // TODO: this is actually redundant, cause they are both enforced to be of appropriate length (ZKS-106).

        cur.account.address = CircuitElement::conditionally_select(
            cs.namespace(|| "mutated_pubkey"),
            &op_data.eth_address,
            &cur.account.address,
            &rhs_valid,
        )?;

        let ohs_valid_flags = vec![
            is_pubdata_chunk_correct,
            is_first_chunk.not(),
            is_second_chunk.not(),
            is_transfer,
            pubdata_properly_copied,
            is_valid_timestamp.clone(),
        ];

        let is_ohs_valid = multi_and(cs.namespace(|| "is_ohs_valid"), &ohs_valid_flags)?;

        let is_op_valid = multi_or(
            cs.namespace(|| "is_op_valid"),
            &[is_ohs_valid, lhs_valid, rhs_valid],
        )?;
        Ok(is_op_valid)
    }

    #[allow(clippy::too_many_arguments)]
    fn swap<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        global_variables: &CircuitGlobalVariables<E>,
        is_a_geq_b: &Boolean,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_valid_timestamp: &Boolean,
        is_sig_verified: &Boolean,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        is_special_nft_storage_account: &Boolean,
        is_special_nft_token: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );
        /*
        fields specification:

        special_eth_addresses = [recipient_0_address, recipient_1_address]
        special_tokens = [order_0_sell_token, order_1_sell_token, fee_token]
        special_accounts = [order_0_sell_amount, order_1_sell_amount]
        special_prices = [order_0_sell_price, order_0_buy_price, order_1_sell_price, order_1_buy_price]
        special_nonces = [account_0_nonce, account_1_nonce, submitter_nonce]
        */

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be());
        pubdata_bits.extend(
            op_data
                .special_accounts
                .iter()
                .map(|acc| acc.get_bits_be())
                .flatten(),
        );
        pubdata_bits.extend(
            op_data
                .special_tokens
                .iter()
                .map(|tok| tok.get_bits_be())
                .flatten(),
        );
        pubdata_bits.extend(op_data.amount_packed.get_bits_be());
        pubdata_bits.extend(op_data.second_amount_packed.get_bits_be());
        pubdata_bits.extend(op_data.fee_packed.get_bits_be());

        let zero = Expression::constant::<CS>(E::Fr::zero());
        let one = Expression::constant::<CS>(E::Fr::one());

        let nonce_inc_0 = Expression::select_ifeq(
            cs.namespace(|| "nonce increment 0"),
            &op_data.special_amounts_unpacked[0].get_number(),
            Expression::u64::<CS>(0u64),
            zero.clone(),
            one.clone(),
        )?;

        let nonce_inc_1 = Expression::select_ifeq(
            cs.namespace(|| "nonce increment 1"),
            &op_data.special_amounts_unpacked[1].get_number(),
            Expression::u64::<CS>(0u64),
            zero.clone(),
            one.clone(),
        )?;

        let nonce_mask = {
            let double_nonce_inc_1 =
                nonce_inc_1.add(cs.namespace(|| "double nonce_inc_1"), &nonce_inc_1)?;
            let nonce_mask = nonce_inc_0.add(cs.namespace(|| "nonce mask"), &double_nonce_inc_1)?;
            CircuitElement::from_fe_with_known_length(
                cs.namespace(|| "nonce mask construction"),
                || nonce_mask.get_value().grab(),
                8,
            )?
        };

        pubdata_bits.extend(nonce_mask.get_bits_be());

        resize_grow_only(
            &mut pubdata_bits,
            SwapOp::CHUNKS * params::CHUNK_BIT_WIDTH,
            Boolean::constant(false),
        );

        let (is_equal_pubdata, packed_pubdata) = vectorized_compare(
            cs.namespace(|| "compare pubdata"),
            &*pubdata_holder,
            &pubdata_bits,
        )?;

        *pubdata_holder = packed_pubdata;

        // construct signature message preimage (serialized_tx)

        let mut serialized_order_bits_0 = vec![];
        let mut serialized_order_bits_1 = vec![];
        let mut serialized_tx_bits_version1 = vec![];

        let order_type = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "order message type"),
            || Ok(E::Fr::from_str(&Order::MSG_TYPE.to_string()).unwrap()),
            8,
        )?;

        serialized_order_bits_0.extend(order_type.get_bits_be());
        serialized_order_bits_0.extend(u8_into_bits_be(params::CURRENT_TX_VERSION));
        serialized_order_bits_0.extend(op_data.special_accounts[0].get_bits_be());
        serialized_order_bits_0.extend(op_data.special_eth_addresses[0].get_bits_be());
        serialized_order_bits_0.extend(op_data.special_nonces[0].get_bits_be());
        serialized_order_bits_0.extend(op_data.special_tokens[0].get_bits_be());
        serialized_order_bits_0.extend(op_data.special_tokens[1].get_bits_be());
        serialized_order_bits_0.extend(op_data.special_prices[0].get_bits_be());
        serialized_order_bits_0.extend(op_data.special_prices[1].get_bits_be());
        serialized_order_bits_0.extend(op_data.special_amounts_packed[0].get_bits_be());
        serialized_order_bits_0.extend(op_data.valid_from.get_bits_be());
        serialized_order_bits_0.extend(op_data.valid_until.get_bits_be());

        serialized_order_bits_1.extend(order_type.get_bits_be());
        serialized_order_bits_1.extend(u8_into_bits_be(params::CURRENT_TX_VERSION));
        serialized_order_bits_1.extend(op_data.special_accounts[2].get_bits_be());
        serialized_order_bits_1.extend(op_data.special_eth_addresses[1].get_bits_be());
        serialized_order_bits_1.extend(op_data.special_nonces[1].get_bits_be());
        serialized_order_bits_1.extend(op_data.special_tokens[1].get_bits_be());
        serialized_order_bits_1.extend(op_data.special_tokens[0].get_bits_be());
        serialized_order_bits_1.extend(op_data.special_prices[2].get_bits_be());
        serialized_order_bits_1.extend(op_data.special_prices[3].get_bits_be());
        serialized_order_bits_1.extend(op_data.special_amounts_packed[1].get_bits_be());
        serialized_order_bits_1.extend(op_data.second_valid_from.get_bits_be());
        serialized_order_bits_1.extend(op_data.second_valid_until.get_bits_be());

        let mut orders_bits = Vec::with_capacity(serialized_order_bits_0.len() * 2);
        orders_bits.extend_from_slice(&serialized_order_bits_0);
        orders_bits.extend_from_slice(&serialized_order_bits_1);

        let result_orders_hash = rescue_hash_allocated_bits(
            cs.namespace(|| "hash orders"),
            self.rescue_params,
            &orders_bits,
        )?;

        serialized_tx_bits_version1.extend(reversed_tx_type_bits_be(SwapOp::OP_CODE));
        serialized_tx_bits_version1.extend(u8_into_bits_be(params::CURRENT_TX_VERSION));
        serialized_tx_bits_version1.extend(op_data.special_accounts[4].get_bits_be());
        serialized_tx_bits_version1.extend(op_data.eth_address.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.special_nonces[2].get_bits_be());
        serialized_tx_bits_version1.extend(result_orders_hash);
        serialized_tx_bits_version1.extend(op_data.special_tokens[2].get_bits_be());
        serialized_tx_bits_version1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.second_amount_packed.get_bits_be());

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &global_variables.chunk_data.chunk_number,
            SwapOp::CHUNKS,
        )?;
        let is_pubdata_chunk_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_pubdata_correct"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);

        let is_swap = Boolean::from(Expression::equals(
            cs.namespace(|| "is_swap"),
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(SwapOp::OP_CODE)), // swap tx_type
        )?);

        let is_chunk_number = (0..SwapOp::CHUNKS as u64)
            .map(|num| {
                Ok(Boolean::from(Expression::equals(
                    cs.namespace(|| format!("is chunk number {}", num)),
                    &global_variables.chunk_data.chunk_number,
                    Expression::u64::<CS>(num),
                )?))
            })
            .collect::<Result<Vec<_>, SynthesisError>>()?;

        let is_first_part = boolean_or(
            cs.namespace(|| "is first part"),
            &is_chunk_number[0],
            &is_chunk_number[1],
        )?;

        let is_second_part = boolean_or(
            cs.namespace(|| "is second part"),
            &is_chunk_number[2],
            &is_chunk_number[3],
        )?;

        let pubdata_properly_copied = boolean_or(
            cs.namespace(|| "first chunk or pubdata is copied properly"),
            &is_chunk_number[0],
            &is_equal_pubdata,
        )?;

        // nonce enforcement
        // order in special_nonces: account0, account1, submitter
        // order in chunks:         account0, recipient1, account1, recipient0, submitter
        let is_nonce_correct_in_slot = (0..3)
            .map(|num| {
                let nonce_correct = CircuitElement::equals(
                    cs.namespace(|| format!("is_nonce_correct_in_slot {}", num)),
                    &cur.account.nonce,
                    &op_data.special_nonces[num],
                )?;
                Boolean::and(
                    cs.namespace(|| format!("is nonce is correct in chunk {}", num * 2)),
                    &nonce_correct,
                    &is_chunk_number[num * 2],
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let is_nonce_correct = multi_or(
            cs.namespace(|| "is_nonce_correct"),
            &is_nonce_correct_in_slot,
        )?;

        // account id enforcement
        // order in special accounts: account0, recipient0, account1, recipient1, submitter
        // order in chunks:           account0, recipient1, account1, recipient0, submitter
        let is_account_id_correct_in_slot = (0..5)
            .map(|num| {
                let permutation = [0, 3, 2, 1, 4];
                let account_id_correct = CircuitElement::equals(
                    cs.namespace(|| format!("is_account_id_correct_in_slot {}", num)),
                    &cur.account_id,
                    &op_data.special_accounts[num],
                )?;
                Boolean::and(
                    cs.namespace(|| format!("is account id correct in chunk {}", permutation[num])),
                    &account_id_correct,
                    &is_chunk_number[permutation[num]],
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let is_account_id_correct = multi_or(
            cs.namespace(|| "is_account_id_correct"),
            &is_account_id_correct_in_slot,
        )?;

        // token enforcement
        // order in special_tokens: token_sell, token_buy, fee_token
        // order in chunks:         token_sell, token_sell, token_buy, token_buy, fee_token
        let is_token_correct_in_chunk = (0..5)
            .map(|num| {
                let token_correct = CircuitElement::equals(
                    cs.namespace(|| format!("is_token_correct_in_slot {}", num)),
                    &cur.token,
                    &op_data.special_tokens[num / 2],
                )?;
                Boolean::and(
                    cs.namespace(|| format!("is token correct in chunk {}", num)),
                    &token_correct,
                    &is_chunk_number[num],
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let is_token_correct = multi_or(
            cs.namespace(|| "is_token_correct"),
            &is_token_correct_in_chunk,
        )?;

        let is_submitter_address_correct = {
            let is_correct = CircuitElement::equals(
                cs.namespace(|| "is_submitter_address_correct"),
                &cur.account.address,
                &op_data.eth_address,
            )?;

            boolean_or(
                cs.namespace(|| "is_submitter_address_correct_in_last_chunk"),
                &is_correct,
                &is_chunk_number[4].not(),
            )?
        };

        let is_recipient_0_address_correct = {
            let is_correct = CircuitElement::equals(
                cs.namespace(|| "is_recipient_0_address_correct"),
                &cur.account.address,
                &op_data.special_eth_addresses[0],
            )?;

            boolean_or(
                cs.namespace(|| "is_recipient_0_address_correct_in_fourth_chunk"),
                &is_correct,
                &is_chunk_number[3].not(),
            )?
        };

        let is_recipient_1_address_correct = {
            let is_correct = CircuitElement::equals(
                cs.namespace(|| "is_recipient_1_address_correct"),
                &cur.account.address,
                &op_data.special_eth_addresses[1],
            )?;

            boolean_or(
                cs.namespace(|| "is_recipient_1_address_correct_in_second_chunk"),
                &is_correct,
                &is_chunk_number[1].not(),
            )?
        };

        let is_a_correct =
            CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;

        let amount_unpacked = CircuitElement::conditionally_select(
            cs.namespace(|| "swapped amount"),
            &op_data.amount_unpacked,
            &op_data.second_amount_unpacked,
            &is_first_part,
        )?;

        let actual_b = CircuitElement::conditionally_select(
            cs.namespace(|| "b"),
            &op_data.fee,
            &amount_unpacked,
            &is_chunk_number[4],
        )?;

        let is_b_correct = Boolean::from(Expression::equals(
            cs.namespace(|| "is_b_correct"),
            &op_data.b.get_number(),
            &actual_b.get_number(),
        )?);

        let are_swapped_tokens_different = CircuitElement::equals(
            cs.namespace(|| "swapped tokens equal"),
            &op_data.special_tokens[0],
            &op_data.special_tokens[1],
        )?
        .not();

        let are_swapping_accounts_different = CircuitElement::equals(
            cs.namespace(|| "swapping accounts equal"),
            &op_data.special_accounts[0],
            &op_data.special_accounts[2],
        )?
        .not();

        let is_amount_valid = {
            let is_amount_explicit = CircuitElement::equals(
                cs.namespace(|| "is first amount explicit"),
                &op_data.special_amounts_unpacked[0],
                &op_data.amount_unpacked,
            )?;
            let is_amount_implicit = CircuitElement::equals(
                cs.namespace(|| "is first amount implicit"),
                &op_data.special_amounts_unpacked[0],
                &global_variables.explicit_zero,
            )?;
            boolean_or(
                cs.namespace(|| "is first amount valid"),
                &is_amount_explicit,
                &is_amount_implicit,
            )?
        };

        let is_second_amount_valid = {
            let is_amount_explicit = CircuitElement::equals(
                cs.namespace(|| "is second amount explicit"),
                &op_data.special_amounts_unpacked[1],
                &op_data.second_amount_unpacked,
            )?;
            let is_amount_implicit = CircuitElement::equals(
                cs.namespace(|| "is second amount implicit"),
                &op_data.special_amounts_unpacked[1],
                &global_variables.explicit_zero,
            )?;
            boolean_or(
                cs.namespace(|| "is second amount valid"),
                &is_amount_explicit,
                &is_amount_implicit,
            )?
        };

        // check that both prices are valid
        // Swap.amountA * Swap.orderA.price.buy <= Swap.amountB * Swap.orderA.price.sell
        // Swap.amountB * Swap.orderB.price.buy <= Swap.amountA * Swap.orderB.price.sell
        let is_first_price_ok = {
            let amount_bought = {
                let amount = op_data.amount_unpacked.get_number().mul(
                    cs.namespace(|| "amountA * orderA.price_buy"),
                    &op_data.special_prices[1].get_number(),
                )?;
                CircuitElement::from_number_with_known_length(
                    cs.namespace(|| "amount bought - first order"),
                    amount,
                    params::BALANCE_BIT_WIDTH + params::PRICE_BIT_WIDTH,
                )?
            };

            let amount_sold = {
                let amount = op_data.second_amount_unpacked.get_number().mul(
                    cs.namespace(|| "amountB * orderA.price_sell"),
                    &op_data.special_prices[0].get_number(),
                )?;
                CircuitElement::from_number_with_known_length(
                    cs.namespace(|| "amount sold - first order"),
                    amount,
                    params::BALANCE_BIT_WIDTH + params::PRICE_BIT_WIDTH,
                )?
            };

            CircuitElement::less_than_fixed(
                cs.namespace(|| "sold < bought (first order)"),
                &amount_sold,
                &amount_bought,
            )?
            .not()
        };

        let is_second_price_ok = {
            let amount_bought = {
                let amount = op_data.second_amount_unpacked.get_number().mul(
                    cs.namespace(|| "amountB * orderB.price_buy"),
                    &op_data.special_prices[3].get_number(),
                )?;
                CircuitElement::from_number_with_known_length(
                    cs.namespace(|| "amount bought - second order"),
                    amount,
                    params::BALANCE_BIT_WIDTH + params::PRICE_BIT_WIDTH,
                )?
            };

            let amount_sold = {
                let amount = op_data.amount_unpacked.get_number().mul(
                    cs.namespace(|| "amountA * orderB.price_sell"),
                    &op_data.special_prices[2].get_number(),
                )?;
                CircuitElement::from_number_with_known_length(
                    cs.namespace(|| "amount sold - second order"),
                    amount,
                    params::BALANCE_BIT_WIDTH + params::PRICE_BIT_WIDTH,
                )?
            };

            CircuitElement::less_than_fixed(
                cs.namespace(|| "sold < bought (second order)"),
                &amount_sold,
                &amount_bought,
            )?
            .not()
        };

        let common_valid_flag = multi_and(
            cs.namespace(|| "common_valid_flags"),
            &[
                is_pubdata_chunk_correct,
                is_swap,
                is_valid_timestamp.clone(),
                pubdata_properly_copied,
                is_account_id_correct,
                is_token_correct,
                is_submitter_address_correct,
                is_recipient_0_address_correct,
                is_recipient_1_address_correct,
                are_swapped_tokens_different,
                are_swapping_accounts_different,
                is_amount_valid,
                is_second_amount_valid,
                is_first_price_ok,
                is_second_price_ok,
                is_special_nft_token.not(),
            ],
        )?;

        let is_lhs_chunk = multi_or(
            cs.namespace(|| "is lhs chunk"),
            &[
                is_chunk_number[0].clone(),
                is_chunk_number[2].clone(),
                is_chunk_number[4].clone(),
            ],
        )?;

        let is_rhs_chunk = boolean_or(
            cs.namespace(|| "is rhs chunk"),
            &is_chunk_number[1],
            &is_chunk_number[3],
        )?;

        let is_serialized_swap_correct = verify_signature_message_construction(
            cs.namespace(|| "is_serialized_swap_correct"),
            serialized_tx_bits_version1,
            &op_data,
        )?;

        let is_serialized_order_0_correct = verify_signature_message_construction(
            cs.namespace(|| "is_serialized_order_0_correct"),
            serialized_order_bits_0,
            &op_data,
        )?;

        let is_serialized_order_1_correct = verify_signature_message_construction(
            cs.namespace(|| "is_serialized_order_1_correct"),
            serialized_order_bits_1,
            &op_data,
        )?;

        let correct_messages_in_corresponding_chunks = &[
            Boolean::and(
                cs.namespace(|| "serialized order 0 in first part of the swap"),
                &is_first_part,
                &is_serialized_order_0_correct,
            )?,
            Boolean::and(
                cs.namespace(|| "serialized order 1 in second part of the swap"),
                &is_second_part,
                &is_serialized_order_1_correct,
            )?,
            Boolean::and(
                cs.namespace(|| "whole swap serialized in last part of the swap"),
                &is_chunk_number[4],
                &is_serialized_swap_correct,
            )?,
        ];

        let is_serialized_tx_correct = multi_or(
            cs.namespace(|| "is_serialized_tx_correct"),
            correct_messages_in_corresponding_chunks,
        )?;

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correct"),
            &signer_key.pubkey.get_hash(),
            &cur.account.pub_key_hash,
        )?;

        let lhs_valid_flags = vec![
            common_valid_flag.clone(),
            is_a_correct,
            is_b_correct,
            is_a_geq_b.clone(),
            is_sig_verified.clone(),
            is_nonce_correct,
            is_lhs_chunk,
            is_serialized_tx_correct,
            is_signer_valid,
            no_nonce_overflow(
                cs.namespace(|| "no nonce overflow"),
                &cur.account.nonce.get_number(),
            )?,
        ];

        let lhs_valid = multi_and(cs.namespace(|| "lhs_valid"), &lhs_valid_flags)?;

        let updated_balance =
            Expression::from(&cur.balance.get_number()) - Expression::from(&actual_b.get_number());

        let nonce_inc = Expression::conditionally_select(
            cs.namespace(|| "nonce increment"),
            &nonce_inc_0,
            &nonce_inc_1,
            &is_first_part,
        )?;

        let nonce_inc = Expression::conditionally_select(
            cs.namespace(|| "nonce increment for submitter always 1"),
            one,
            Expression::from(&nonce_inc),
            &is_chunk_number[4],
        )?;

        let sender_is_submitter = CircuitElement::equals(
            cs.namespace(|| "is account sender == submitter"),
            &cur.account_id,
            &op_data.special_accounts[4],
        )?;

        // if submitter == account_0 or account_1 then
        // don't increment nonce for this account
        let nonce_inc = {
            let sender_is_submitter_and_not_last_chunk = Boolean::and(
                cs.namespace(|| "sender == submitter and we are not in the last chunk"),
                &sender_is_submitter,
                &is_chunk_number[4].not(),
            )?;
            Expression::conditionally_select(
                cs.namespace(|| "nonce increment is 0 if account == submitter (for account)"),
                zero,
                Expression::from(&nonce_inc),
                &sender_is_submitter_and_not_last_chunk,
            )?
        };

        let updated_nonce =
            Expression::from(&cur.account.nonce.get_number()) + Expression::from(&nonce_inc);

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
        let rhs_valid = multi_and(
            cs.namespace(|| "is_rhs_valid"),
            &[
                common_valid_flag,
                is_account_empty.not(),
                is_rhs_chunk,
                is_special_nft_storage_account.not(),
            ],
        )?;

        // calculate new rhs balance value
        let updated_balance = Expression::from(&cur.balance.get_number())
            + Expression::from(&amount_unpacked.get_number());

        //update balance
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated_balance rhs"),
            updated_balance,
            &cur.balance,
            &rhs_valid,
        )?;

        // Either LHS xor RHS are correct (due to chunking at least)
        let correct = Boolean::xor(
            cs.namespace(|| "lhs_valid XOR rhs_valid"),
            &lhs_valid,
            &rhs_valid,
        )?;

        Ok(correct)
    }

    #[allow(clippy::too_many_arguments)]
    fn transfer<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &mut AllocatedOperationBranch<E>,
        lhs: &AllocatedOperationBranch<E>,
        rhs: &AllocatedOperationBranch<E>,
        global_variables: &CircuitGlobalVariables<E>,
        is_a_geq_b: &Boolean,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_valid_timestamp: &Boolean,
        is_sig_verified: &Boolean,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        is_special_nft_storage_account: &Boolean,
        is_special_nft_token: &Boolean,
        is_fungible_token: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be());
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
        let mut serialized_tx_bits_version1 = vec![];
        serialized_tx_bits_version1.extend(reversed_tx_type_bits_be(TransferOp::OP_CODE));
        serialized_tx_bits_version1.extend(u8_into_bits_be(params::CURRENT_TX_VERSION));
        serialized_tx_bits_version1.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits_version1.extend(lhs.account.address.get_bits_be());
        serialized_tx_bits_version1.extend(rhs.account.address.get_bits_be());
        serialized_tx_bits_version1.extend(cur.token.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_version1.extend(cur.account.nonce.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_until.get_bits_be());
        assert_eq!(serialized_tx_bits_version1.len(), SIGNED_TRANSFER_BIT_WIDTH);

        let mut serialized_tx_bits_old1 = vec![];
        serialized_tx_bits_old1.extend(global_variables.chunk_data.tx_type.get_bits_be());
        serialized_tx_bits_old1.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits_old1.extend(lhs.account.address.get_bits_be());
        serialized_tx_bits_old1.extend(rhs.account.address.get_bits_be());
        // the old version contains token 2-byte representation
        serialized_tx_bits_old1.extend_from_slice(&cur.token.get_bits_be()[16..32]);
        serialized_tx_bits_old1.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits_old1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_old1.extend(cur.account.nonce.get_bits_be());
        assert_eq!(
            serialized_tx_bits_old1.len(),
            params::OLD1_SIGNED_TRANSFER_BIT_WIDTH
        );

        let mut serialized_tx_bits_old2 = vec![];
        serialized_tx_bits_old2.extend(global_variables.chunk_data.tx_type.get_bits_be());
        serialized_tx_bits_old2.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits_old2.extend(lhs.account.address.get_bits_be());
        serialized_tx_bits_old2.extend(rhs.account.address.get_bits_be());
        // the old version contains token 2-byte representation
        serialized_tx_bits_old2.extend_from_slice(&cur.token.get_bits_be()[16..32]);
        serialized_tx_bits_old2.extend(op_data.amount_packed.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_old2.extend(cur.account.nonce.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_old2.extend(op_data.valid_until.get_bits_be());
        assert_eq!(
            serialized_tx_bits_old2.len(),
            params::OLD2_SIGNED_TRANSFER_BIT_WIDTH
        );

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &global_variables.chunk_data.chunk_number,
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
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(TransferOp::OP_CODE)), // transfer tx_type
        )?);

        let mut lhs_valid_flags = vec![
            is_pubdata_chunk_correct.clone(),
            is_transfer.clone(),
            is_valid_timestamp.clone(),
        ];
        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &global_variables.chunk_data.chunk_number,
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

        let is_version1_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_version1_serialized_tx_correct"),
            serialized_tx_bits_version1,
            &op_data,
        )?;

        let mut is_old1_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_old1_serialized_tx_correct"),
            serialized_tx_bits_old1,
            &op_data,
        )?;
        is_old1_serialized_tx_correct = multi_and(
            cs.namespace(|| "is_old1_serialized_tx_correct and fungible"),
            &[is_old1_serialized_tx_correct, is_fungible_token.clone()],
        )?;

        let mut is_old2_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_old2_serialized_tx_correct"),
            serialized_tx_bits_old2,
            &op_data,
        )?;
        is_old2_serialized_tx_correct = multi_and(
            cs.namespace(|| "is_old2_serialized_tx_correct and fungible"),
            &[is_old2_serialized_tx_correct, is_fungible_token.clone()],
        )?;

        let is_serialized_tx_correct = multi_or(
            cs.namespace(|| "is_serialized_tx_correct"),
            &[
                is_version1_serialized_tx_correct,
                is_old1_serialized_tx_correct,
                is_old2_serialized_tx_correct,
            ],
        )?;
        lhs_valid_flags.push(is_serialized_tx_correct);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correct"),
            &signer_key.pubkey.get_hash(),
            &lhs.account.pub_key_hash,
        )?;
        lhs_valid_flags.push(is_signer_valid);

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
        let mut rhs_valid_flags = vec![
            pubdata_properly_copied,
            is_transfer,
            is_valid_timestamp.clone(),
        ];
        let is_chunk_second = Boolean::from(Expression::equals(
            cs.namespace(|| "is_chunk_second"),
            &global_variables.chunk_data.chunk_number,
            Expression::u64::<CS>(1),
        )?);
        rhs_valid_flags.push(is_chunk_second);
        rhs_valid_flags.push(is_account_empty.not());
        rhs_valid_flags.push(is_special_nft_storage_account.not());
        rhs_valid_flags.push(is_special_nft_token.not());

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

        // Either LHS xor RHS are correct (due to chunking at least)
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
        global_variables: &CircuitGlobalVariables<E>,
        is_a_geq_b: &Boolean,
        is_account_empty: &Boolean,
        op_data: &AllocatedOperationData<E>,
        signer_key: &AllocatedSignerPubkey<E>,
        ext_pubdata_chunk: &AllocatedNum<E>,
        is_valid_timestamp: &Boolean,
        is_sig_verified: &Boolean,
        pubdata_holder: &mut Vec<AllocatedNum<E>>,
        is_special_nft_storage_account: &Boolean,
        is_special_nft_token: &Boolean,
        is_fungible_token: &Boolean,
    ) -> Result<Boolean, SynthesisError> {
        assert!(
            !pubdata_holder.is_empty(),
            "pubdata holder has to be preallocated"
        );

        // construct pubdata
        let mut pubdata_bits = vec![];
        pubdata_bits.extend(global_variables.chunk_data.tx_type.get_bits_be());
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

        let mut serialized_tx_bits_version1 = vec![];
        serialized_tx_bits_version1.extend(reversed_tx_type_bits_be(ForcedExitOp::OP_CODE));
        serialized_tx_bits_version1.extend(u8_into_bits_be(params::CURRENT_TX_VERSION));
        serialized_tx_bits_version1.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits_version1.extend(rhs.account.address.get_bits_be());
        serialized_tx_bits_version1.extend(cur.token.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_version1.extend(lhs.account.nonce.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_version1.extend(op_data.valid_until.get_bits_be());
        assert_eq!(
            serialized_tx_bits_version1.len(),
            SIGNED_FORCED_EXIT_BIT_WIDTH
        );

        // Construct serialized tx
        let mut serialized_tx_bits_old = vec![];
        serialized_tx_bits_old.extend(global_variables.chunk_data.tx_type.get_bits_be());
        serialized_tx_bits_old.extend(lhs.account_id.get_bits_be());
        serialized_tx_bits_old.extend(rhs.account.address.get_bits_be());
        // the old version contains token 2-byte representation
        serialized_tx_bits_old.extend_from_slice(&cur.token.get_bits_be()[16..32]);
        serialized_tx_bits_old.extend(op_data.fee_packed.get_bits_be());
        serialized_tx_bits_old.extend(lhs.account.nonce.get_bits_be());
        serialized_tx_bits_old.extend(op_data.valid_from.get_bits_be());
        serialized_tx_bits_old.extend(op_data.valid_until.get_bits_be());
        assert_eq!(
            serialized_tx_bits_old.len(),
            params::OLD_SIGNED_FORCED_EXIT_BIT_WIDTH
        );

        let pubdata_chunk = select_pubdata_chunk(
            cs.namespace(|| "select_pubdata_chunk"),
            &pubdata_bits,
            &global_variables.chunk_data.chunk_number,
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
            &global_variables.chunk_data.tx_type.get_number(),
            Expression::u64::<CS>(u64::from(ForcedExitOp::OP_CODE)),
        )?);

        let mut lhs_valid_flags = vec![
            is_pubdata_chunk_correct.clone(),
            is_forced_exit.clone(),
            is_valid_timestamp.clone(),
            is_special_nft_token.not(),
        ];
        let is_first_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_first_chunk"),
            &global_variables.chunk_data.chunk_number,
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

        let is_version1_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_version1_serialized_tx_correct"),
            serialized_tx_bits_version1,
            &op_data,
        )?;

        let mut is_old_serialized_tx_correct = verify_signature_message_construction(
            cs.namespace(|| "is_old_serialized_tx_correct"),
            serialized_tx_bits_old,
            &op_data,
        )?;
        is_old_serialized_tx_correct = multi_and(
            cs.namespace(|| "is_old_serialized_tx_correct and fungible"),
            &[is_old_serialized_tx_correct, is_fungible_token.clone()],
        )?;

        let is_serialized_tx_correct = multi_or(
            cs.namespace(|| "is_serialized_tx_correct"),
            &[
                is_version1_serialized_tx_correct,
                is_old_serialized_tx_correct,
            ],
        )?;

        lhs_valid_flags.push(is_serialized_tx_correct);

        let is_signer_valid = CircuitElement::equals(
            cs.namespace(|| "signer_key_correct"),
            &signer_key.pubkey.get_hash(),
            &lhs.account.pub_key_hash,
        )?;
        lhs_valid_flags.push(is_signer_valid);

        lhs_valid_flags.push(is_fungible_token.clone());

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
        let mut rhs_valid_flags = vec![
            pubdata_properly_copied.clone(),
            is_forced_exit.clone(),
            is_valid_timestamp.clone(),
            is_special_nft_storage_account.not(),
            is_special_nft_token.not(),
        ];
        let is_second_chunk = Boolean::from(Expression::equals(
            cs.namespace(|| "is_chunk_second"),
            &global_variables.chunk_data.chunk_number,
            Expression::u64::<CS>(1),
        )?);
        rhs_valid_flags.push(is_second_chunk.clone());
        rhs_valid_flags.push(is_account_empty.not());

        rhs_valid_flags.push(is_pubdata_chunk_correct.clone());

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

        // Check that `eth_address` corresponds to the rhs account Ethereum address.
        let is_pubkey_empty = CircuitElement::equals(
            cs.namespace(|| "is_pubkey_empty"),
            &rhs.account.pub_key_hash,
            &global_variables.explicit_zero,
        )?;
        rhs_valid_flags.push(is_pubkey_empty);

        rhs_valid_flags.push(is_fungible_token.clone());

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
        let ohs_valid_flags = vec![
            is_pubdata_chunk_correct,
            is_first_chunk.not(),
            is_second_chunk.not(),
            is_forced_exit,
            is_valid_timestamp.clone(),
            pubdata_properly_copied,
            is_fungible_token.clone(),
        ];

        let is_ohs_valid = multi_and(cs.namespace(|| "is_ohs_valid"), &ohs_valid_flags)?;

        let is_op_valid = multi_or(
            cs.namespace(|| "is_op_valid"),
            &[is_ohs_valid, lhs_valid, rhs_valid],
        )?;
        Ok(is_op_valid)
    }

    fn verify_operation_timestamp<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        op_data: &AllocatedOperationData<E>,
        global_variables: &CircuitGlobalVariables<E>,
    ) -> Result<Boolean, SynthesisError> {
        let is_valid_from_ok = CircuitElement::less_than_fixed(
            cs.namespace(|| "valid_from leq block_timestamp"),
            &global_variables.block_timestamp,
            &op_data.valid_from,
        )?
        .not();

        let is_valid_until_ok = CircuitElement::less_than_fixed(
            cs.namespace(|| "block_timestamp leq valid_until"),
            &op_data.valid_until,
            &global_variables.block_timestamp,
        )?
        .not();

        let is_second_valid_from_ok = CircuitElement::less_than_fixed(
            cs.namespace(|| "second_valid_from leq block_timestamp"),
            &global_variables.block_timestamp,
            &op_data.second_valid_from,
        )?
        .not();

        let is_second_valid_until_ok = CircuitElement::less_than_fixed(
            cs.namespace(|| "block_timestamp leq second_valid_until"),
            &op_data.second_valid_until,
            &global_variables.block_timestamp,
        )?
        .not();

        let is_valid_timestamp = multi_and(
            cs.namespace(|| "is_valid_from_ok AND is_valid_until_ok"),
            &[
                is_valid_from_ok,
                is_valid_until_ok,
                is_second_valid_from_ok,
                is_second_valid_until_ok,
            ],
        )?;

        Ok(is_valid_timestamp)
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

    // this is safe and just allows the convention.
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

    let remaining_index_bits = AllocatedNum::pack_bits_to_element(
        cs.namespace(|| "index_bits_after_length_root_packed"),
        &index[length_to_root..],
    )?;
    remaining_index_bits.assert_zero(cs.namespace(|| "index_bits_after_length_are_zero"))?;

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

#[allow(dead_code)]
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

fn calculate_validator_root_from_processable_values<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    processable_fees: &[AllocatedNum<E>],
    non_processable_audit: &[AllocatedNum<E>],
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
    assert_eq!(
        non_processable_audit.len(),
        params::balance_tree_depth() - processable_fees_tree_depth,
    );

    // will hash processable part of the tree
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

    let mut node_hash = hash_vec[0].clone();
    for (i, audit_value) in (processable_fees_tree_depth..params::balance_tree_depth())
        .zip(non_processable_audit.iter())
    {
        let cs = &mut cs.namespace(|| format!("merkle tree level index number {}", i));

        let mut sponge_output = rescue::rescue_hash(
            cs.namespace(|| "perform smt hashing"),
            &[node_hash, audit_value.clone()],
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

pub fn hash_nft_content_to_balance_type<E: RescueEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    creator_account_id: &CircuitElement<E>,
    serial_id: &CircuitElement<E>,
    content_hash: &[CircuitElement<E>],
    params: &E::Params,
) -> Result<CircuitElement<E>, SynthesisError> {
    let mut content_hash_as_booleans_le = content_hash
        .iter()
        .map(|bit| bit.get_bits_le())
        .flatten()
        .collect::<Vec<_>>();
    content_hash_as_booleans_le.reverse();
    assert_eq!(content_hash_as_booleans_le.len(), CONTENT_HASH_WIDTH);

    let mut lhs_le_bits = vec![];
    lhs_le_bits.extend_from_slice(&content_hash_as_booleans_le[128..]);
    lhs_le_bits.extend(serial_id.get_bits_le());
    lhs_le_bits.extend(creator_account_id.get_bits_le());
    let lhs = CircuitElement::from_le_bits(cs.namespace(|| "lhs"), lhs_le_bits)?;

    let mut rhs_le_bits = vec![];
    rhs_le_bits.extend_from_slice(&content_hash_as_booleans_le[..128]);
    let rhs = CircuitElement::from_le_bits(cs.namespace(|| "rhs"), rhs_le_bits)?;

    let mut sponge_output = rescue::rescue_hash(
        cs.namespace(|| "hash lhs and rhs"),
        &[lhs.get_number(), rhs.get_number()],
        params,
    )?;
    assert_eq!(sponge_output.len(), 1);
    let content_as_bits_le = sponge_output
        .pop()
        .expect("must get a single element")
        .into_bits_le_strict(cs.namespace(|| "content into_bits_le_strict"))?;

    CircuitElement::from_le_bits(
        cs.namespace(|| "NFT_content_as_balance from lower BALANCE_BIT_WIDTH bits"),
        content_as_bits_le[..params::BALANCE_BIT_WIDTH].to_vec(),
    )
}

fn generate_maxchunk_polynomial<E: JubjubEngine>() -> Vec<E::Fr> {
    use zksync_crypto::franklin_crypto::interpolation::interpolate;

    let get_xy = |op_type: u8, op_chunks: usize| {
        let x = E::Fr::from_str(&op_type.to_string()).unwrap();
        let y = E::Fr::from_str(&(op_chunks - 1).to_string()).unwrap();
        (x, y)
    };

    let points: Vec<(E::Fr, E::Fr)> = vec![
        get_xy(NoopOp::OP_CODE, NoopOp::CHUNKS),
        get_xy(CloseOp::OP_CODE, CloseOp::CHUNKS),
        get_xy(TransferOp::OP_CODE, TransferOp::CHUNKS),
        get_xy(DepositOp::OP_CODE, DepositOp::CHUNKS),
        get_xy(WithdrawOp::OP_CODE, WithdrawOp::CHUNKS),
        get_xy(TransferToNewOp::OP_CODE, TransferToNewOp::CHUNKS),
        get_xy(FullExitOp::OP_CODE, FullExitOp::CHUNKS),
        get_xy(ChangePubKeyOp::OP_CODE, ChangePubKeyOp::CHUNKS),
        get_xy(ForcedExitOp::OP_CODE, ForcedExitOp::CHUNKS),
        get_xy(MintNFTOp::OP_CODE, MintNFTOp::CHUNKS),
        get_xy(WithdrawNFTOp::OP_CODE, WithdrawNFTOp::CHUNKS),
        get_xy(SwapOp::OP_CODE, SwapOp::CHUNKS),
    ];
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

fn rescue_hash_allocated_bits<E: RescueEngine + JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    rescue_params: &<E as RescueEngine>::Params,
    bits: &[Boolean],
) -> Result<Vec<Boolean>, SynthesisError> {
    let input = multipack::pack_into_witness(
        cs.namespace(|| "pack transaction bits into field elements for rescue"),
        &bits,
    )?;

    let sponge_output = rescue::rescue_hash(cs.namespace(|| "rescue hash"), &input, rescue_params)?;
    assert_eq!(sponge_output.len(), 1);

    let output_bits_le = sponge_output[0].into_bits_le(cs.namespace(|| "rescue hash bits"))?;

    // Max whole number of bytes that fit into Fr (248 bits)
    let len_bits = (E::Fr::CAPACITY / 8 * 8) as usize;

    Ok(output_bits_le[..len_bits].to_vec())
}

fn reversed_tx_type_bits_be(tx_type: u8) -> Vec<Boolean> {
    let reversed_tx_type = 255 - tx_type;
    assert!(reversed_tx_type > tx_type);

    u8_into_bits_be(reversed_tx_type)
}
