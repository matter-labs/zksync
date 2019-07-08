use crate::account::AccountContent;
use crate::allocated_structures::*;
use crate::element::{CircuitElement, CircuitPubkey};
use crate::operation::Operation;
use crate::utils::pack_bits_to_element;
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField};
use franklin_crypto::circuit::baby_eddsa::EddsaSignature;
use franklin_crypto::circuit::boolean::Boolean;
use franklin_crypto::circuit::ecc;
use franklin_crypto::circuit::sha256;

use franklin_crypto::circuit::num::AllocatedNum;
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::circuit::polynomial_lookup::{do_the_lookup, generate_powers};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};
use franklinmodels::params as franklin_constants;

const DIFFERENT_TRANSACTIONS_TYPE_NUMBER: usize = 6;

struct FranklinCircuit<'a, E: JubjubEngine> {
    pub params: &'a E::Params,
    /// The old root of the tree
    pub old_root: Option<E::Fr>,

    /// The new root of the tree
    pub new_root: Option<E::Fr>,
    pub block_number: Option<E::Fr>,
    pub validator_address: Option<E::Fr>,

    pub pub_data_commitment: Option<E::Fr>,

    pub operations: Vec<Operation<E>>,
}

#[derive(Clone)]
struct PreviousData<E: JubjubEngine> {
    //lhs, rhs //TODO: #merkle
    new_root: AllocatedNum<E>,
}

// Implementation of our circuit:
//
impl<'a, E: JubjubEngine> Circuit<E> for FranklinCircuit<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let public_data_commitment =
            AllocatedNum::alloc(cs.namespace(|| "public_data_commitment"), || {
                self.pub_data_commitment.grab()
            })?;
        public_data_commitment.inputize(cs.namespace(|| "inputize pub_data"))?;

            let validator_address = CircuitElement::from_fe_padded(cs.namespace(||"validator_address"), ||self.validator_address.grab())?;
        

        let mut rolling_root =
            AllocatedNum::alloc(cs.namespace(|| "rolling_root"), || self.old_root.grab())?;

        let old_root = CircuitElement::from_number_padded(cs.namespace(|| "old_root"), rolling_root.clone())?;

        let mut next_chunk_number =
            AllocatedNum::alloc(cs.namespace(|| "next_chunk_number"), || Ok(E::Fr::zero()))?;
        next_chunk_number.assert_zero(cs.namespace(|| "initial next_chunk_number"))?;

        let mut allocated_chunk_data: AllocatedChunkData<E>;
        let mut block_pub_data_bits = vec![];
        for (i, operation) in self.operations.iter().enumerate() {
            println!("\n operation number {} started \n", i);
            let cs = &mut cs.namespace(|| format!("chunk number {}", i));

            let (next_chunk, chunk_data) = self.verify_correct_chunking(
                &operation,
                &mut next_chunk_number,
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
            let (state_root, is_account_empty) = self
                .check_account_data(cs.namespace(|| "calculate account root"), &current_branch)?;
            println!("old_state_root: {}", state_root.get_value().unwrap());
            println!(
                "is_account_empty: {}",
                is_account_empty.get_value().unwrap()
            );
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
            )?;
            let (new_state_root, _is_account_empty) = self.check_account_data(
                cs.namespace(|| "calculate new account root"),
                &current_branch,
            )?;
            let operation_new_root =
                AllocatedNum::alloc(cs.namespace(|| "op_new_root"), || operation.new_root.grab())?;
            println!("new state_root: {}", new_state_root.get_value().unwrap());
            println!(
                "op new state_root: {}",
                operation_new_root.get_value().unwrap()
            );
            cs.enforce(
                || "new root is correct",
                |lc| lc + new_state_root.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + operation_new_root.get_variable(),
            );
            rolling_root = new_state_root;
        }

        //TODO: apply fees
        let final_root = CircuitElement::from_number_padded(cs.namespace(|| "final_root"), rolling_root.clone())?;
        {
            // Now it's time to pack the initial SHA256 hash due to Ethereum BE encoding
            // and start rolling the hash

            let mut initial_hash_data: Vec<Boolean> = vec![];

            // make initial hash as sha256(uint256(block_number)||uint256(validator_address))
            let mut block_number = CircuitElement::from_fe_padded(cs.namespace(||"block_number"), ||self.block_number.grab())?;
            
            initial_hash_data.extend(block_number.get_bits_be());

            //TODO: probably enforce validator_address
            initial_hash_data.extend(validator_address.get_bits_be());

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

            let final_hash = pack_bits_to_element(cs.namespace(||"final_hash"), &hash_block)?;

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
            *franklin_constants::TX_TYPE_BIT_WIDTH,
        )?;
        enforce_lies_between(
            cs.namespace(|| "tx_type is valid"),
            &tx_type.get_number(),
            0 as i32,
            DIFFERENT_TRANSACTIONS_TYPE_NUMBER as i32,
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
        let is_chunk_last = AllocatedNum::equals(
            cs.namespace(|| "is_chunk_last"),
            &operation_chunk_number,
            &max_chunk,
        )?;

        let subseq_chunk_value = match operation_chunk_number.get_value() {
            Some(a) => {
                let mut a = a;
                a.add_assign(&E::Fr::one());
                Some(a)
            }
            None => None,
        };

        let subseq_chunk = AllocatedNum::alloc(cs.namespace(|| "subseq_chunk_number"), || {
            Ok(subseq_chunk_value.grab()?)
        })?;

        cs.enforce(
            || "enforce subsequence",
            |lc| lc + operation_chunk_number.get_variable() + CS::one(),
            |lc| lc + CS::one(),
            |lc| lc + subseq_chunk.get_variable(),
        );

        let zero_chunk_number =
            AllocatedNum::alloc(cs.namespace(|| "zero_chunk_number"), || Ok(E::Fr::zero()))?;

        zero_chunk_number.assert_zero(cs.namespace(|| "initial next_chunk_number"))?; //TODO: we can use the same zero every time if it makes sense

        let next_chunk_number = AllocatedNum::conditionally_select(
            cs.namespace(|| "determine next_chunk_number"),
            &zero_chunk_number,
            &subseq_chunk,
            &Boolean::from(is_chunk_last.clone()),
        )?;

        Ok((
            next_chunk_number,
            AllocatedChunkData {
                chunk_number: operation_chunk_number,
                is_chunk_last: is_chunk_last,
                tx_type: tx_type,
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
        let deposit_allocated =
            AllocatedNum::alloc(cs.namespace(|| "deposit_tx_type"), || Ok(E::Fr::one()))?;
        deposit_allocated.assert_number(cs.namespace(|| "deposit_type is one"), &E::Fr::one())?;

        let left_side = AllocatedNum::alloc(cs.namespace(|| "left_side"), || Ok(E::Fr::zero()))?;
        left_side.assert_zero(cs.namespace(|| "left_side is zero"))?;

        let cur_side = AllocatedNum::select_ifeq(
            cs.namespace(|| "select corresponding branch"),
            &chunk_data.tx_type.get_number(),
            &deposit_allocated,
            &left_side,
            &chunk_data.chunk_number,
        )?;

        let is_left = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_left"),
            &left_side,
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
                pub_key: CircuitPubkey::conditionally_select(
                    cs.namespace(|| "chosen pubkey"),
                    &first.account.pub_key,
                    &second.account.pub_key,
                    &is_left,
                )?,
            },
            account_audit_path: select_vec_ifeq(
                cs.namespace(|| "account_audit_path"),
                &left_side,
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
                &left_side,
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
    ) -> Result<(AllocatedNum<E>, Boolean), SynthesisError> {
        //first we prove calculate root of the subtree to obtain account_leaf_data:
        let (cur_account_leaf_bits, is_account_empty) = self.allocate_account_leaf_bits(
            cs.namespace(|| "allocate current_account_leaf_hash"),
            cur,
        )?;
        println!("cur_account_leaf_bits.len {}", cur_account_leaf_bits.len());
        let temp = pedersen_hash::pedersen_hash(
            cs.namespace(|| "account leaf content hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &cur_account_leaf_bits,
            self.params,
        )?
        .clone()
        .get_x()
        .clone();
        println!("acc_leaf_hash: {}", temp.get_value().unwrap());
        Ok((
            allocate_merkle_root(
                cs.namespace(|| "account_merkle_root"),
                &cur_account_leaf_bits,
                &cur.account_address.get_bits_le(),
                &cur.account_audit_path,
                self.params,
            )?,
            is_account_empty,
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
    ) -> Result<(), SynthesisError> {
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

        let sender_pk = ecc::EdwardsPoint::interpret(
            cs.namespace(|| "sender public key"),
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

        let max_message_len = 32 as usize; //TODO fix when clear
                                           //TOdO: we should always use the same length
        signature.verify_raw_message_signature(
            cs.namespace(|| "verify transaction signature"),
            self.params,
            &op_data.sig_msg.get_bits_le(),
            generator,
            max_message_len,
        )?;

        let diff_a_b = AllocatedNum::alloc(cs.namespace(|| "a-b"), || {
            let mut a_val = op_data.a.grab()?;
            a_val.sub_assign(&op_data.b.grab()?);
            Ok(a_val)
        })?;
        cs.enforce(
            || "a-b is correct",
            |lc| lc + op_data.a.get_number().get_variable() - op_data.b.get_number().get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + diff_a_b.get_variable(),
        );
        let mut diff_a_b_bits = diff_a_b.into_bits_le(cs.namespace(|| "a - b bits"))?;
        diff_a_b_bits.truncate(franklin_constants::BALANCE_BIT_WIDTH);
        let diff_a_b_repacked =
            pack_bits_to_element(cs.namespace(|| "pack a-b bits"), &diff_a_b_bits)?;
        let is_a_geq_b = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "diff equal to repacked"),
            &diff_a_b,
            &diff_a_b_repacked,
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
        let op_valid = multi_or(cs.namespace(|| "op_valid"), &op_flags)?;
        println!("op_valid {}", op_valid.get_value().unwrap());
        cs.enforce(
            || "op is valid",
            |_| op_valid.lc(CS::one(), E::Fr::one()),
            |lc| lc + CS::one(),
            |lc| lc + CS::one(),
        );
        Ok(())
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
        let mut is_valid_flags = vec![];
        //construct pubdata
        let mut pubdata_bits = vec![];
        let mut pub_token_bits = cur.token.get_bits_le().clone();
        pub_token_bits.resize(
            *franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        pubdata_bits.extend(chunk_data.tx_type.get_bits_le().clone()); //TX_TYPE_BIT_WIDTH=8
        pubdata_bits.extend(cur.account_address.get_bits_le().clone()); //ACCOUNT_TREE_DEPTH=24
        pubdata_bits.extend(pub_token_bits); //TOKEN_EXT_BIT_WIDTH=16
        pubdata_bits.extend(op_data.amount_packed.get_bits_le().clone()); //AMOUNT_PACKED=24
        pubdata_bits.extend(op_data.fee_packed.get_bits_le().clone()); //FEE_PACKED=8
        pubdata_bits.extend(op_data.new_pubkey_hash.get_bits_le().clone()); //NEW_PUBKEY_HASH_WIDTH=224
        assert_eq!(pubdata_bits.len(), 304);
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
        let is_pubdata_chunk_correct = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_pubdata_equal"),
            &pubdata_chunk,
            ext_pubdata_chunk,
        )?);
        is_valid_flags.push(is_pubdata_chunk_correct);

        // verify correct tx_code
        let allocated_deposit_tx_type =
            AllocatedNum::alloc(cs.namespace(|| "deposit_tx_type"), || Ok(E::Fr::one()))?;
        allocated_deposit_tx_type
            .assert_number(cs.namespace(|| "deposit_tx_type equals one"), &E::Fr::one())?;
        let is_deposit = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_deposit"),
            &chunk_data.tx_type.get_number(),
            &allocated_deposit_tx_type,
        )?);
        is_valid_flags.push(is_deposit.clone());

        // verify if new pubkey is equal to previous one (if existed)
        let is_pub_equal_to_previous = CircuitPubkey::equals(
            cs.namespace(|| "is_pub_equal_to_previous"),
            &op_data.new_pubkey,
            &cur.account.pub_key,
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
        let tx_valid = multi_and(cs.namespace(|| "is_tx_valid"), &is_valid_flags)?;

        println!("tx_valid {}", tx_valid.get_value().unwrap());

        //TODO precompute
        let zero = AllocatedNum::alloc(cs.namespace(|| "zero"), || Ok(E::Fr::zero()))?;
        zero.assert_zero(cs.namespace(|| "zero is zero"))?;
        let is_first_chunk = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            &zero,
        )?);
        println!("is_first  chunk {}", is_first_chunk.get_value().unwrap());
        let is_valid_first = Boolean::and(
            cs.namespace(|| "is valid and first"),
            &tx_valid,
            &is_first_chunk,
        )?;

        //calculate updated value on deposit
        let updated_balance_value =
            AllocatedNum::alloc(cs.namespace(|| "updated_balance_value"), || {
                let mut new_balance = cur.balance.clone().grab()?;
                new_balance.add_assign(&op_data.amount.grab()?);
                new_balance.sub_assign(&op_data.fee.grab()?);
                Ok(new_balance)
            })?;
        cs.enforce(
            || "correct_updated_balance",
            |lc| lc + updated_balance_value.get_variable(),
            |lc| lc + CS::one(),
            |lc| {
                lc + cur.balance.get_number().get_variable() + op_data.amount.get_number().get_variable()//TODO: get_number().get_variable() is kinda ugly
                    - op_data.fee.get_number().get_variable()
            },
        );
        //
        let updated_balance_ce = CircuitElement::from_number(
            cs.namespace(|| "updated_balance_ce"),
            updated_balance_value,
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;
        //mutate current branch if it is first chunk of valid deposit transaction
        cur.balance = CircuitElement::conditionally_select(
            cs.namespace(|| "mutated balance"),
            &updated_balance_ce,
            &cur.balance,
            &is_valid_first,
        )?;
        cur.balance
            .enforce_length(cs.namespace(|| "mutated balance is still correct length"))?;

        println!(
            "changed bal data: {}",
            cur.balance.get_number().get_value().unwrap()
        );
        cur.account.pub_key = CircuitPubkey::conditionally_select(
            cs.namespace(|| "mutated_pubkey"),
            &op_data.new_pubkey,
            &cur.account.pub_key,
            &is_valid_first,
        )?;

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
        let mut lhs_valid_flags = vec![];
        let transfer_to_new_tx_type = AllocatedNum::alloc(cs.namespace(|| "transfer_to_new_tx_type"), || {
                Ok(E::Fr::from_str("2").unwrap())
            })?;
        transfer_to_new_tx_type.assert_number(
            cs.namespace(|| "transfer_to_new_tx_type equals two"),
            &E::Fr::from_str("2").unwrap(),
        )?;
        let is_transfer = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_transfer"),
            &chunk_data.tx_type.get_number(),
            &transfer_to_new_tx_type,
        )?);
        lhs_valid_flags.push(is_transfer);
        let zero = AllocatedNum::alloc(cs.namespace(|| "zero"), || Ok(E::Fr::zero()))?;
        zero.assert_zero(cs.namespace(|| "zero is zero"))?;
        let is_first_chunk = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            &zero,
        )?);
        lhs_valid_flags.push(is_first_chunk);

    }

    //TODO: verify token equality
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
            *franklin_constants::TOKEN_EXT_BIT_WIDTH,
            Boolean::constant(false),
        );
        pub_token_bits.reverse();
        pubdata_bits.extend(chunk_data.tx_type.get_bits_be());
        pubdata_bits.extend(lhs.account_address.get_bits_be());
        pubdata_bits.extend(pub_token_bits.clone());
        pubdata_bits.extend(rhs.account_address.get_bits_be());
        pubdata_bits.extend(op_data.amount_packed.get_bits_be());
        pubdata_bits.extend(op_data.fee_packed.get_bits_be());
        assert_eq!(pubdata_bits.len(), 13 * 8);

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
        let is_pubdata_chunk_correct = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_pubdata_correct"),
            &pubdata_chunk,
            &ext_pubdata_chunk,
        )?);
        println!(
            "ext_pubdata_chunk {}",
            ext_pubdata_chunk.get_value().grab()?
        );
        println!("pubdata_chunk {}", pubdata_chunk.get_value().grab()?);
        // verify correct tx_code
        let allocated_transfer_tx_type =
            AllocatedNum::alloc(cs.namespace(|| "transfer_tx_type"), || {
                Ok(E::Fr::from_str("5").unwrap())
            })?;
        allocated_transfer_tx_type.assert_number(
            cs.namespace(|| "transfer_tx_type equals five"),
            &E::Fr::from_str("5").unwrap(),
        )?;
        let is_transfer = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_transfer"),
            &chunk_data.tx_type.get_number(),
            &allocated_transfer_tx_type,
        )?);

        let mut lhs_valid_flags = vec![];

        lhs_valid_flags.push(is_pubdata_chunk_correct.clone());
        lhs_valid_flags.push(is_transfer.clone());
        let zero = AllocatedNum::alloc(cs.namespace(|| "zero"), || Ok(E::Fr::zero()))?;
        zero.assert_zero(cs.namespace(|| "zero is zero"))?;
        let is_first_chunk = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            &zero,
        )?);
        lhs_valid_flags.push(is_first_chunk);

        // construct signature message
        let mut sig_bits = vec![];
        let mut transfer_tx_type_bits =
            allocated_transfer_tx_type.into_bits_le(cs.namespace(|| "transfer_tx_type_bits"))?;
        transfer_tx_type_bits.truncate(*franklin_constants::TX_TYPE_BIT_WIDTH);
        sig_bits.extend(transfer_tx_type_bits);
        sig_bits.extend(lhs.account_address.get_bits_le());
        sig_bits.extend(lhs.token.get_bits_le());
        sig_bits.extend(lhs.account.nonce.get_bits_le());
        sig_bits.extend(op_data.amount_packed.get_bits_le().clone());
        sig_bits.extend(op_data.fee_packed.get_bits_le().clone());
        let sig_bit_len = sig_bits.len();
        let sig_msg = CircuitElement::from_le_bits(
            cs.namespace(|| "sig_msg from bits"),
            sig_bits,
            sig_bit_len,
        )?; //TODO; think of ommiting 3rd argument
        let sig_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "hash_sig_bits"),
            pedersen_hash::Personalization::NoteCommitment,
            &sig_msg.get_bits_le(),
            self.params,
        )?
        .get_x()
        .clone();
        //TODO: rhs_pubkey

        println!(
            "sig_hash={} sig_bits.len={}",
            sig_hash.get_value().grab()?,
            sig_bit_len,
        );

        let is_sig_msg_correct = CircuitElement::equals(
            cs.namespace(|| "is_sig_msg_correct"),
            &op_data.sig_msg,
            &sig_msg,
        )?;

        println!(
            "is_sig_msg_correct={} ",
            is_sig_msg_correct.get_value().grab()?
        );

        lhs_valid_flags.push(is_sig_msg_correct);

        // check signer pubkey
        let is_signer_key_correct = CircuitPubkey::equals(
            cs.namespace(|| "is_signer_key_correct"),
            &op_data.signer_pubkey,
            &lhs.account.pub_key,
        )?;
        lhs_valid_flags.push(is_signer_key_correct);

        // check operation arguments
        let is_a_correct =
            CircuitElement::equals(cs.namespace(|| "is_a_correct"), &op_data.a, &cur.balance)?;

        lhs_valid_flags.push(is_a_correct);

        let sum_amount_fee = AllocatedNum::alloc(cs.namespace(|| "amount plus fee"), || {
            let mut bal = op_data.amount.grab()?;
            bal.add_assign(&op_data.fee.grab()?);
            Ok(bal)
        })?;
        cs.enforce(
            || "sum_amount_fee is correct",
            |lc| {
                lc + op_data.amount.get_number().get_variable()
                    + op_data.fee.get_number().get_variable()
            },
            |lc| lc + CS::one(),
            |lc| lc + sum_amount_fee.get_variable(),
        );
        let is_b_correct = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_b_correct"),
            &op_data.b.get_number(),
            &sum_amount_fee,
        )?);
        lhs_valid_flags.push(is_b_correct);
        lhs_valid_flags.push(is_a_geq_b.clone());

        lhs_valid_flags.push(no_nonce_overflow(
            cs.namespace(|| "no nonce overflow"),
            &cur.account.nonce.get_number(),
        )?);

        println!("lhs valid");
        let lhs_valid = multi_and(cs.namespace(|| "lhs_valid"), &lhs_valid_flags)?;
        println!("is lhs valid {}", lhs_valid.get_value().grab()?);

        let updated_balance_value =
            AllocatedNum::alloc(cs.namespace(|| "lhs updated_balance_value"), || {
                let mut bal = cur.balance.clone().grab()?;
                bal.sub_assign(sum_amount_fee.get_value().get()?);
                Ok(bal)
            })?;
        cs.enforce(
            || "lhs updated_balance_value is correct",
            |lc| lc + cur.balance.get_number().get_variable() - sum_amount_fee.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + updated_balance_value.get_variable(),
        );

        let updated_nonce = AllocatedNum::alloc(cs.namespace(|| "updated_nonce"), || {
            let mut nonce = cur.account.nonce.clone().grab()?;
            nonce.add_assign(&E::Fr::from_str("1").unwrap());
            Ok(nonce)
        })?;
        cs.enforce(
            || "updated_nonce is correct",
            |lc| lc + updated_nonce.get_variable() - CS::one(),
            |lc| lc + CS::one(),
            |lc| lc + cur.account.nonce.get_number().get_variable(),
        );
        //update cur values if lhs is valid
        //update nonce
        cur.account.nonce = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "update cur nonce"),
            &updated_nonce,
            &cur.account.nonce,
            &lhs_valid,
        )?;

        //update balance
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated cur balance"),
            &updated_balance_value,
            &cur.balance,
            &lhs_valid,
        )?;

        // rhs
        let mut rhs_valid_flags = vec![];
        rhs_valid_flags.push(is_transfer);

        let one =
            AllocatedNum::alloc(cs.namespace(|| "one"), || Ok(E::Fr::from_str("1").unwrap()))?;
        one.assert_number(
            cs.namespace(|| "one is correct"),
            &E::Fr::from_str("1").unwrap(),
        )?;
        let is_chunk_second = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_chunk_second"),
            &chunk_data.chunk_number,
            &one,
        )?);
        rhs_valid_flags.push(is_chunk_second);
        rhs_valid_flags.push(is_account_empty.not());

        rhs_valid_flags.push(is_pubdata_chunk_correct.clone());
        let is_rhs_valid = multi_and(cs.namespace(|| "is_rhs_valid"), &rhs_valid_flags)?;

        // calculate new rhs balance value
        let updated_balance_value =
            AllocatedNum::alloc(cs.namespace(|| "updated_balance_value"), || {
                let mut bal = cur.balance.clone().grab()?;
                bal.add_assign(&op_data.amount.grab()?);
                Ok(bal)
            })?;
        cs.enforce(
            || "rhs updated_balance_value is correct",
            |lc| {
                lc + cur.balance.get_number().get_variable()
                    + op_data.amount.get_number().get_variable()
            },
            |lc| lc + CS::one(),
            |lc| lc + updated_balance_value.get_variable(),
        );

        //update balance
        cur.balance = CircuitElement::conditionally_select_with_number_strict(
            cs.namespace(|| "updated_balance rhs"),
            &updated_balance_value,
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
    ) -> Result<(Vec<Boolean>, Boolean), SynthesisError> {
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
        let subtree_root = CircuitElement::from_number(
            cs.namespace(|| "subtree_root_ce"),
            balance_root,
            franklin_constants::FR_BIT_WIDTH,
        )?;
        let mut account_data = vec![];
        account_data.extend(branch.account.nonce.get_bits_le().clone());
        account_data.extend(branch.account.pub_key.get_packed_key());
        println!(
            "get_packed_key {}",
            branch.account.pub_key.get_packed_key().len()
        );
        let account_data_packed =
            pack_bits_to_element(cs.namespace(|| "account_data_packed"), &account_data)?;

        let zero = AllocatedNum::alloc(cs.namespace(|| "zero"), || Ok(E::Fr::zero()))?;
        zero.assert_zero(cs.namespace(|| "zero is zero"))?;

        let is_account_empty = AllocatedNum::equals(
            cs.namespace(|| "is_account_empty"),
            &account_data_packed,
            &zero,
        )?;
        account_data.extend(subtree_root.get_bits_le());
        Ok((account_data, Boolean::from(is_account_empty)))
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
    for (i, direction_bit) in index.clone().into_iter().enumerate() {
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

fn select_vec_ifeq<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    a: &AllocatedNum<E>,
    b: &AllocatedNum<E>,
    x: &[AllocatedNum<E>],
    y: &[AllocatedNum<E>],
) -> Result<Vec<AllocatedNum<E>>, SynthesisError> {
    assert_eq!(x.len(), y.len());
    let mut resulting_vector = vec![];
    for (i, (t_x, t_y)) in x.iter().zip(y.iter()).enumerate() {
        let temp =
            AllocatedNum::select_ifeq(cs.namespace(|| format!("iteration {}", i)), a, b, t_x, t_y)?;
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
        println!("and number i:{} value:{}", i, result.get_value().grab()?);
    }

    Ok(result)
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
        let current_chunk_number_allocated =
            AllocatedNum::alloc(cs.namespace(|| "chunk number"), || {
                Ok(E::Fr::from_str(&i.to_string()).unwrap())
            })?;
        current_chunk_number_allocated.assert_number(
            cs.namespace(|| "number is correct"),
            &E::Fr::from_str(&i.to_string()).unwrap(),
        )?;
        result = AllocatedNum::select_ifeq(
            cs.namespace(|| "select if correct chunk number"),
            &current_chunk_number_allocated,
            &chunk_number,
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
// TODO: actually it is redundant due to circuit design
fn enforce_lies_between<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    number: &AllocatedNum<E>,
    min: i32,
    max: i32,
) -> Result<(), SynthesisError> {
    let mut initial_mult_value = number.get_value().grab()?;
    initial_mult_value.sub_assign(&E::Fr::from_str(&min.to_string()).unwrap());
    let mut current_mult =
        AllocatedNum::alloc(cs.namespace(|| "initial_mult"), || Ok(initial_mult_value))?;
    cs.enforce(
        || "initial_mult is number - min",
        |lc| {
            lc + current_mult.get_variable() - number.get_variable()
                + (E::Fr::from_str(&min.to_string()).unwrap(), CS::one())
        },
        |lc| lc + CS::one(),
        |lc| lc,
    );
    for i in min..max {
        let mut x_value = E::Fr::from_str(&(i + 1).to_string()).unwrap();
        x_value.sub_assign(&number.get_value().grab()?);
        x_value.mul_assign(&current_mult.get_value().grab()?);

        let new_mult = AllocatedNum::alloc(
            cs.namespace(|| format!("current mult on iteration {}", i + 1)),
            || Ok(x_value),
        )?;
        cs.enforce(
            || format!("equals i {}", i),
            |lc| lc + current_mult.get_variable(),
            |lc| {
                lc + (E::Fr::from_str(&(i + 1).to_string()).unwrap(), CS::one())
                    - number.get_variable()
            },
            |lc| lc + new_mult.get_variable(),
        );
        current_mult = new_mult;
    }
    current_mult.assert_zero(cs.namespace(|| "lies between"))?;
    Ok(())
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

    for i in &[3, 5] {
        //transfer, create_subaccount, close_subaccount, fill_orders
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::from_str("1").unwrap();
        points.push((x, y));
    }
    for i in &[1, 2] {
        //deposit, transfer_to_new
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
    let max_nonce = AllocatedNum::alloc(cs.namespace(|| "max_nonce"), || {
        Ok(E::Fr::from_str(&(256 * 256 - 1).to_string()).unwrap())
    })?;
    max_nonce.assert_number(
        cs.namespace(|| "max_nonce is correct"),
        &E::Fr::from_str(&(256 * 256 - 1).to_string()).unwrap(),
    )?;
    Ok(Boolean::from(AllocatedNum::equals(
        cs.namespace(|| "is nonce at max"),
        nonce,
        &max_nonce,
    )?)
    .not())
}
#[cfg(test)]
mod test {

    use super::*;

    use franklin_crypto::jubjub::FixedGenerators;

    use franklin_crypto::eddsa::{PrivateKey, PublicKey};

    #[test]
    fn test_deposit_franklin_in_empty_leaf() {
        use crate::account::*;
        use crate::operation::*;
        use crate::utils::*;
        use ff::{BitIterator, Field};
        use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
        use franklin_crypto::circuit::float_point::convert_to_float;
        use franklin_crypto::circuit::test::*;
        use franklinmodels::circuit::account::{Balance, CircuitAccount};
        use franklinmodels::{CircuitAccountTree, CircuitBalanceTree};
        use merkle_tree::hasher::Hasher;
        use merkle_tree::PedersenHasher;
        use pairing::bn256::*;
        use rand::{Rng, SeedableRng, XorShiftRng};

        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;

        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let mut balance_tree =
            CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32);
        let balance_root = balance_tree.root_hash();
        // println!("test balance root: {}", balance_root);
        // println!("test subaccount root: {}", subaccount_root);
        let phasher = PedersenHasher::<Bn256>::default();
        let default_subtree_hash = balance_root;
        // println!("test subtree root: {}", default_subtree_hash);
        let zero_account = CircuitAccount {
            nonce: Fr::zero(),
            pub_x: Fr::zero(),
            pub_y: Fr::zero(),
            subtree_root_hash: default_subtree_hash,
        };
        let mut tree = CircuitAccountTree::new_with_leaf(
            franklin_constants::ACCOUNT_TREE_DEPTH as u32,
            zero_account,
        );
        let initial_root = tree.root_hash();
        println!("Initial root = {}", initial_root);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << franklin_constants::ACCOUNT_TREE_DEPTH);

        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
        println!("x = {}, y = {}", sender_x, sender_y);

        // give some funds to sender and make zero balance for recipient

        // let sender_leaf_number = 1;

        let mut sender_leaf_number: u32 = rng.gen();
        sender_leaf_number %= capacity;
        let sender_leaf_number_fe = Fr::from_str(&sender_leaf_number.to_string()).unwrap();
        println!(
            "old leaf hash is {}",
            tree.get_hash((
                franklin_constants::ACCOUNT_TREE_DEPTH as u32,
                sender_leaf_number
            ))
        );
        let transfer_amount: u128 = 500;

        let transfer_amount_as_field_element = Fr::from_str(&transfer_amount.to_string()).unwrap();

        let transfer_amount_bits = convert_to_float(
            transfer_amount,
            *franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
            *franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let transfer_amount_encoded: Fr = le_bit_vector_into_field_element(&transfer_amount_bits);

        let fee: u128 = 0;

        let fee_as_field_element = Fr::from_str(&fee.to_string()).unwrap();

        let fee_bits = convert_to_float(
            fee,
            *franklin_constants::FEE_EXPONENT_BIT_WIDTH,
            *franklin_constants::FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

        let token: u32 = 2;
        let token_fe = Fr::from_str(&token.to_string()).unwrap();

        balance_tree.insert(
            token,
            Balance {
                value: transfer_amount_as_field_element,
            },
        );
        let after_deposit_balance_root = balance_tree.root_hash();

        let after_deposit_subtree_hash = after_deposit_balance_root;

        let sender_leaf = CircuitAccount::<Bn256> {
            subtree_root_hash: after_deposit_subtree_hash.clone(),
            nonce: Fr::zero(),
            pub_x: sender_x.clone(),
            pub_y: sender_y.clone(),
        };

        tree.insert(sender_leaf_number, sender_leaf.clone());
        let new_root = tree.root_hash();

        println!("New root = {}", new_root);

        assert!(initial_root != new_root);
        println!(
            "updated leaf hash is {}",
            tree.get_hash((
                franklin_constants::ACCOUNT_TREE_DEPTH as u32,
                sender_leaf_number
            ))
        );

        let sig_msg = Fr::from_str("2").unwrap(); //dummy sig msg cause skipped on deposit proof
        let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
        sig_bits.reverse();
        sig_bits.truncate(80);

        // println!(" capacity {}",<Bn256 as JubjubEngine>::Fs::Capacity);
        let signature = sign(&sig_bits, &sender_sk, p_g, params, rng);
        //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));

        let deposit_tx_type = Fr::from_str("1").unwrap();
        let mut pubdata_bits = vec![];
        append_be_fixed_width(
            &mut pubdata_bits,
            &deposit_tx_type,
            *franklin_constants::TX_TYPE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &sender_leaf_number_fe,
            franklin_constants::ACCOUNT_TREE_DEPTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &token_fe,
            *franklin_constants::TOKEN_EXT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &transfer_amount_encoded,
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &fee_encoded,
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );

        let mut new_pubkey_bits = vec![];
        append_be_fixed_width(
            &mut new_pubkey_bits,
            &sender_y,
            franklin_constants::FR_BIT_WIDTH - 1,
        );
        append_be_fixed_width(&mut new_pubkey_bits, &sender_x, 1);
        let new_pubkey_hash = phasher.hash_bits(new_pubkey_bits);

        append_be_fixed_width(
            &mut pubdata_bits,
            &new_pubkey_hash,
            *franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        assert_eq!(pubdata_bits.len(), 38 * 8);
        pubdata_bits.resize(40 * 8, false);

        let pubdata_chunks: Vec<Fr> = pubdata_bits
            .chunks(64)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .collect::<Vec<_>>();

        let audit_path: Vec<Option<Fr>> = tree
            .merkle_path(sender_leaf_number)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let audit_balance_path: Vec<Option<Fr>> = balance_tree
            .merkle_path(token)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let op_args = OperationArguments {
            a: Some(transfer_amount_as_field_element.clone()),
            b: Some(fee_as_field_element.clone()),
            amount: Some(transfer_amount_encoded.clone()),
            fee: Some(fee_encoded.clone()),
            new_pub_x: Some(sender_x.clone()),
            new_pub_y: Some(sender_y.clone()),
        };
        let operation_branch_before = OperationBranch {
            address: Some(sender_leaf_number_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(Fr::zero()),
                    pub_x: Some(Fr::zero()),
                    pub_y: Some(Fr::zero()),
                },
                account_path: audit_path.clone(),
                balance_value: Some(Fr::zero()),
                balance_subtree_path: audit_balance_path.clone(),
            },
        };
        let operation_branch_after = OperationBranch::<Bn256> {
            address: Some(sender_leaf_number_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(Fr::zero()),
                    pub_x: Some(sender_x.clone()),
                    pub_y: Some(sender_y.clone()),
                },
                account_path: audit_path.clone(),
                balance_value: Some(transfer_amount_as_field_element.clone()),
                balance_subtree_path: audit_balance_path.clone(),
            },
        };
        let operation_zero = Operation {
            new_root: Some(new_root.clone()),
            tx_type: Some(Fr::from_str("1").unwrap()),
            chunk: Some(Fr::from_str("0").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[0]),
            sig_msg: Some(sig_msg.clone()),
            signature: signature.clone(),
            signer_pub_key_x: Some(sender_x.clone()),
            signer_pub_key_y: Some(sender_y.clone()),
            args: op_args.clone(),
            lhs: operation_branch_before.clone(),
            rhs: operation_branch_before.clone(),
        };

        let operation_one = Operation {
            new_root: Some(new_root.clone()),
            tx_type: Some(Fr::from_str("1").unwrap()),
            chunk: Some(Fr::from_str("1").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[1]),
            sig_msg: Some(sig_msg.clone()),
            signature: signature.clone(),
            signer_pub_key_x: Some(sender_x.clone()),
            signer_pub_key_y: Some(sender_y.clone()),
            args: op_args.clone(),
            lhs: operation_branch_after.clone(),
            rhs: operation_branch_after.clone(),
        };

        let operation_two = Operation {
            new_root: Some(new_root.clone()),
            tx_type: Some(Fr::from_str("1").unwrap()),
            chunk: Some(Fr::from_str("2").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[2]),
            sig_msg: Some(sig_msg.clone()),
            signature: signature.clone(),
            signer_pub_key_x: Some(sender_x.clone()),
            signer_pub_key_y: Some(sender_y.clone()),
            args: op_args.clone(),
            lhs: operation_branch_after.clone(),
            rhs: operation_branch_after.clone(),
        };

        let operation_three = Operation {
            new_root: Some(new_root.clone()),
            tx_type: Some(Fr::from_str("1").unwrap()),
            chunk: Some(Fr::from_str("3").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[3]),
            sig_msg: Some(sig_msg.clone()),
            signature: signature.clone(),
            signer_pub_key_x: Some(sender_x.clone()),
            signer_pub_key_y: Some(sender_y.clone()),
            args: op_args.clone(),
            lhs: operation_branch_after.clone(),
            rhs: operation_branch_after.clone(),
        };
        let operation_four = Operation {
            new_root: Some(new_root.clone()),
            tx_type: Some(Fr::from_str("1").unwrap()),
            chunk: Some(Fr::from_str("4").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[4]),
            sig_msg: Some(sig_msg.clone()),
            signature: signature.clone(),
            signer_pub_key_x: Some(sender_x.clone()),
            signer_pub_key_y: Some(sender_y.clone()),
            args: op_args.clone(),
            lhs: operation_branch_after.clone(),
            rhs: operation_branch_after.clone(),
        };
        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let instance = FranklinCircuit {
                params,
                old_root: Some(initial_root),
                new_root: Some(new_root),
                operations: vec![
                    operation_zero,
                    operation_one,
                    operation_two,
                    operation_three,
                    operation_four,
                ],
                pub_data_commitment: None,
                block_number: None,
                validator_address: None, 
            };

            instance.synthesize(&mut cs).unwrap();

            println!("{}", cs.find_unconstrained());

            println!("{}", cs.num_constraints());

            let err = cs.which_is_unsatisfied();
            if err.is_some() {
                panic!("ERROR satisfying in {}", err.unwrap());
            }
        }
    }
    #[test]
    fn test_transfer() {
        use crate::account::*;
        use crate::operation::*;
        use crate::utils::*;

        use crypto::digest::Digest;
        use crypto::sha2::Sha256;
        use ff::Field;
        use ff::{BitIterator, PrimeFieldRepr};
        use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
        use franklin_crypto::circuit::float_point::convert_to_float;
        use franklin_crypto::circuit::test::*;
        use franklinmodels::circuit::account::{Balance, CircuitAccount};
        use franklinmodels::{CircuitAccountTree, CircuitBalanceTree};
        use merkle_tree::hasher::Hasher;
        use merkle_tree::PedersenHasher;
        use pairing::bn256::*;
        use rand::{Rng, SeedableRng, XorShiftRng};

        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;

        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let mut from_balance_tree =
            CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32);
        let from_balance_root = from_balance_tree.root_hash();

        let mut to_balance_tree =
            CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32);

        let validator_address = Fr::from_str("7").unwrap();
        let phasher = PedersenHasher::<Bn256>::default();
        let default_subtree_hash = from_balance_root;
        // println!("test subtree root: {}", default_subtree_hash);
        let zero_account = CircuitAccount {
            nonce: Fr::zero(),
            pub_x: Fr::zero(),
            pub_y: Fr::zero(),
            subtree_root_hash: default_subtree_hash,
        };
        let mut tree = CircuitAccountTree::new_with_leaf(
            franklin_constants::ACCOUNT_TREE_DEPTH as u32,
            zero_account,
        );

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << franklin_constants::ACCOUNT_TREE_DEPTH);

        let from_sk = PrivateKey::<Bn256>(rng.gen());
        let from_pk = PublicKey::from_private(&from_sk, p_g, params);
        let (from_x, from_y) = from_pk.0.into_xy();
        println!("x = {}, y = {}", from_x, from_y);

        let to_sk = PrivateKey::<Bn256>(rng.gen());
        let to_pk = PublicKey::from_private(&to_sk, p_g, params);
        let (to_x, to_y) = to_pk.0.into_xy();
        println!("x = {}, y = {}", to_x, to_y);

        // give some funds to sender and make zero balance for recipient

        // let sender_leaf_number = 1;

        let mut from_leaf_number: u32 = rng.gen();
        from_leaf_number %= capacity;
        let from_leaf_number_fe = Fr::from_str(&from_leaf_number.to_string()).unwrap();

        let mut to_leaf_number: u32 = rng.gen();
        to_leaf_number %= capacity;
        let to_leaf_number_fe = Fr::from_str(&to_leaf_number.to_string()).unwrap();

        let from_balance_before: u128 = 2000;

        let from_balance_before_as_field_element =
            Fr::from_str(&from_balance_before.to_string()).unwrap();

        let to_balance_before: u128 = 2100;

        let to_balance_before_as_field_element =
            Fr::from_str(&to_balance_before.to_string()).unwrap();

        let transfer_amount: u128 = 500;

        let transfer_amount_as_field_element = Fr::from_str(&transfer_amount.to_string()).unwrap();

        let transfer_amount_bits = convert_to_float(
            transfer_amount,
            *franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
            *franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let transfer_amount_encoded: Fr = le_bit_vector_into_field_element(&transfer_amount_bits);

        let fee: u128 = 0;

        let fee_as_field_element = Fr::from_str(&fee.to_string()).unwrap();

        let fee_bits = convert_to_float(
            fee,
            *franklin_constants::FEE_EXPONENT_BIT_WIDTH,
            *franklin_constants::FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

        let token: u32 = 2;
        let token_fe = Fr::from_str(&token.to_string()).unwrap();

        from_balance_tree.insert(
            token,
            Balance {
                value: from_balance_before_as_field_element,
            },
        );

        let from_base_balance_root = from_balance_tree.root_hash();

        let from_leaf_before = CircuitAccount::<Bn256> {
            subtree_root_hash: from_base_balance_root.clone(),
            nonce: Fr::zero(),
            pub_x: from_x.clone(),
            pub_y: from_y.clone(),
        };

        to_balance_tree.insert(
            token,
            Balance {
                value: to_balance_before_as_field_element,
            },
        );
        let to_base_balance_root = to_balance_tree.root_hash();
        let to_leaf_before = CircuitAccount::<Bn256> {
            subtree_root_hash: to_base_balance_root.clone(),
            nonce: Fr::zero(),
            pub_x: to_x.clone(),
            pub_y: to_y.clone(),
        };
        tree.insert(from_leaf_number, from_leaf_before.clone());
        tree.insert(to_leaf_number, to_leaf_before.clone());
        println!(
            "hash from leaf {}",
            tree.get_hash((
                franklin_constants::ACCOUNT_TREE_DEPTH as u32,
                from_leaf_number
            ))
        );

        let from_audit_path_before: Vec<Option<Fr>> = tree
            .merkle_path(from_leaf_number)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let to_audit_path_before: Vec<Option<Fr>> = tree
            .merkle_path(to_leaf_number)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let from_audit_balance_path_before: Vec<Option<Fr>> = from_balance_tree
            .merkle_path(token)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let to_audit_balance_path_before: Vec<Option<Fr>> = to_balance_tree
            .merkle_path(token)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let initial_root = tree.root_hash();
        println!("Initial root = {}", initial_root);

        let mut from_balance_after = from_balance_before_as_field_element.clone();
        from_balance_after.sub_assign(&transfer_amount_as_field_element);

        from_balance_tree.insert(
            token,
            Balance {
                value: from_balance_after,
            },
        );

        let mut from_nonce_after_transfer = from_leaf_before.nonce.clone();
        from_nonce_after_transfer.add_assign(&Fr::from_str("1").unwrap());

        let from_leaf_after = CircuitAccount::<Bn256> {
            subtree_root_hash: from_balance_tree.root_hash(),
            nonce: from_nonce_after_transfer,
            pub_x: from_x.clone(),
            pub_y: from_y.clone(),
        };
        tree.insert(from_leaf_number, from_leaf_after.clone());
        let intermediate_root = tree.root_hash();

        let mut to_balance_after = to_balance_before_as_field_element.clone();
        to_balance_after.add_assign(&transfer_amount_as_field_element);

        to_balance_tree.insert(
            token,
            Balance {
                value: to_balance_after,
            },
        );

        let to_nonce_after_transfer = to_leaf_before.nonce.clone();

        let to_leaf_after = CircuitAccount::<Bn256> {
            subtree_root_hash: to_balance_tree.root_hash(),
            nonce: to_nonce_after_transfer,
            pub_x: to_x.clone(),
            pub_y: to_y.clone(),
        };
        tree.insert(to_leaf_number, to_leaf_after.clone());
        let final_root = tree.root_hash();

        // construct signature
        let mut sig_bits = vec![];

        let transfer_tx_type = Fr::from_str("5").unwrap();
        append_le_fixed_width(
            &mut sig_bits,
            &transfer_tx_type,
            *franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_le_fixed_width(
            &mut sig_bits,
            &from_leaf_number_fe,
            franklin_constants::ACCOUNT_TREE_DEPTH,
        );
        append_le_fixed_width(
            &mut sig_bits,
            &token_fe,
            *franklin_constants::BALANCE_TREE_DEPTH,
        );
        append_le_fixed_width(
            &mut sig_bits,
            &from_leaf_before.nonce,
            franklin_constants::NONCE_BIT_WIDTH,
        );
        append_le_fixed_width(
            &mut sig_bits,
            &transfer_amount_encoded,
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );
        append_le_fixed_width(
            &mut sig_bits,
            &fee_encoded,
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );
        let sig_msg = le_bit_vector_into_field_element::<Fr>(&sig_bits);
        let sig_msg_hash = phasher.hash_bits(sig_bits.clone());
        let mut sig_msg_hash_bits = vec![];
        append_le_fixed_width(
            &mut sig_msg_hash_bits,
            &sig_msg_hash,
            franklin_constants::FR_BIT_WIDTH - 8,
        ); //TODO: not clear what capacity is

        println!(
            "test sig_msg_hash={} sig_msg_hash_bits.len={}",
            sig_msg_hash,
            sig_msg_hash_bits.len()
        );

        // construct pubdata
        let mut pubdata_bits = vec![];
        append_be_fixed_width(
            &mut pubdata_bits,
            &transfer_tx_type,
            *franklin_constants::TX_TYPE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &from_leaf_number_fe,
            franklin_constants::ACCOUNT_TREE_DEPTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &token_fe,
            *franklin_constants::TOKEN_EXT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &to_leaf_number_fe,
            franklin_constants::ACCOUNT_TREE_DEPTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &transfer_amount_encoded,
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &fee_encoded,
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );
        assert_eq!(pubdata_bits.len(), 13 * 8);
        pubdata_bits.resize(16 * 8, false); //TODO verify if right padding is okay

        let pub_first_chunk_bits = pubdata_bits[0..franklin_constants::CHUNK_BIT_WIDTH].to_vec();
        let pub_first_chunk = le_bit_vector_into_field_element::<Fr>(&pub_first_chunk_bits);
        println!("pub_first_chunk {}", pub_first_chunk);
        let pub_second_chunk_bits = pubdata_bits
            [franklin_constants::CHUNK_BIT_WIDTH..2 * franklin_constants::CHUNK_BIT_WIDTH]
            .to_vec();
        let pub_second_chunk = le_bit_vector_into_field_element::<Fr>(&pub_second_chunk_bits);
        println!("pub_second_chunk {}", pub_second_chunk);
        // println!(" capacity {}",<Bn256 as JubjubEngine>::Fs::Capacity);

        let mut public_data_initial_bits = vec![];

        // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

        let block_number_bits: Vec<bool> = BitIterator::new(Fr::one().into_repr()).collect();
        for _ in 0..256 - block_number_bits.len() {
            public_data_initial_bits.push(false);
        }
        public_data_initial_bits.extend(block_number_bits.into_iter());

        let validator_id_bits: Vec<bool> =
            BitIterator::new(validator_address.into_repr()).collect();
        for _ in 0..256 - validator_id_bits.len() {
            public_data_initial_bits.push(false);
        }
        public_data_initial_bits.extend(validator_id_bits.into_iter());

        assert_eq!(public_data_initial_bits.len(), 512);

        let mut h = Sha256::new();

        let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

        h.input(&bytes_to_hash);

        let mut hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);

        println!("Initial hash hex {}", hex::encode(hash_result));

        let mut packed_old_root_bits = vec![];
        let old_root_bits: Vec<bool> = BitIterator::new(initial_root.into_repr()).collect();
        for _ in 0..256 - old_root_bits.len() {
            packed_old_root_bits.push(false);
        }
        packed_old_root_bits.extend(old_root_bits);

        let packed_old_root_bytes = be_bit_vector_into_bytes(&packed_old_root_bits);

        let mut packed_with_old_root = vec![];
        packed_with_old_root.extend(hash_result.iter());
        packed_with_old_root.extend(packed_old_root_bytes);

        h = Sha256::new();
        h.input(&packed_with_old_root);
        hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);

        let mut packed_new_root_bits = vec![];
        let new_root_bits: Vec<bool> = BitIterator::new(final_root.into_repr()).collect();
        for _ in 0..256 - new_root_bits.len() {
            packed_new_root_bits.push(false);
        }
        packed_new_root_bits.extend(new_root_bits);

        let packed_new_root_bytes = be_bit_vector_into_bytes(&packed_new_root_bits);

        let mut packed_with_new_root = vec![];
        packed_with_new_root.extend(hash_result.iter());
        packed_with_new_root.extend(packed_new_root_bytes);

        h = Sha256::new();
        h.input(&packed_with_new_root);
        hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);

        println!("hash with new root as hex {}", hex::encode(hash_result));

        let mut final_bytes = vec![];
        let pubdata_bytes = be_bit_vector_into_bytes(&pubdata_bits);
        final_bytes.extend(hash_result.iter());
        final_bytes.extend(pubdata_bytes);

        h = Sha256::new();
        h.input(&final_bytes);
        hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);

        println!("final hash as hex {}", hex::encode(hash_result));

        hash_result[0] &= 0x1f; // temporary solution, this nullifies top bits to be encoded into field element correctly

        let mut repr = Fr::zero().into_repr();
        repr.read_be(&hash_result[..])
            .expect("pack hash as field element");

        let public_data_commitment = Fr::from_repr(repr).unwrap();

        println!(
            "Final data commitment as field element = {}",
            public_data_commitment
        );

        let signature = sign(&sig_bits, &from_sk, p_g, params, rng);

        //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));

        let _from_audit_path_after: Vec<Option<Fr>> = tree
            .merkle_path(from_leaf_number)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let to_audit_path_after: Vec<Option<Fr>> = tree
            .merkle_path(to_leaf_number)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let from_audit_balance_path_after: Vec<Option<Fr>> = from_balance_tree
            .merkle_path(token)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let _to_audit_balance_path_after: Vec<Option<Fr>> = to_balance_tree
            .merkle_path(token)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let mut sum_amount_fee = transfer_amount_as_field_element.clone();
        sum_amount_fee.add_assign(&fee_as_field_element);

        let op_args = OperationArguments::<Bn256> {
            a: Some(from_balance_before_as_field_element),
            b: Some(sum_amount_fee.clone()),
            amount: Some(transfer_amount_encoded.clone()),
            fee: Some(fee_encoded.clone()),
            new_pub_x: Some(from_x.clone()),
            new_pub_y: Some(from_y.clone()),
        };

        let from_operation_branch_before = OperationBranch::<Bn256> {
            address: Some(from_leaf_number_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(from_leaf_before.nonce),
                    pub_x: Some(from_leaf_before.pub_x),
                    pub_y: Some(from_leaf_before.pub_y),
                },
                account_path: from_audit_path_before.clone(),
                balance_value: Some(from_balance_before_as_field_element.clone()),
                balance_subtree_path: from_audit_balance_path_before.clone(),
            },
        };

        let from_operation_branch_after = OperationBranch::<Bn256> {
            address: Some(from_leaf_number_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(from_leaf_after.nonce),
                    pub_x: Some(from_leaf_after.pub_x),
                    pub_y: Some(from_leaf_after.pub_y),
                },
                account_path: from_audit_path_before.clone(),
                balance_value: Some(from_balance_after.clone()),
                balance_subtree_path: from_audit_balance_path_after.clone(),
            },
        };

        let to_operation_branch_before = OperationBranch::<Bn256> {
            address: Some(to_leaf_number_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(to_leaf_before.nonce),
                    pub_x: Some(to_leaf_before.pub_x),
                    pub_y: Some(to_leaf_before.pub_y),
                },
                account_path: to_audit_path_before.clone(),
                balance_value: Some(to_balance_before_as_field_element.clone()),
                balance_subtree_path: to_audit_balance_path_before.clone(),
            },
        };

        let to_operation_branch_after = OperationBranch::<Bn256> {
            address: Some(to_leaf_number_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(to_leaf_before.nonce),
                    pub_x: Some(to_leaf_before.pub_x),
                    pub_y: Some(to_leaf_before.pub_y),
                },
                account_path: to_audit_path_after.clone(),
                balance_value: Some(to_balance_before_as_field_element.clone()),
                balance_subtree_path: to_audit_balance_path_before.clone(),
            },
        };

        let operation_zero = Operation {
            new_root: Some(intermediate_root.clone()),
            tx_type: Some(Fr::from_str("5").unwrap()),
            chunk: Some(Fr::from_str("0").unwrap()),
            pubdata_chunk: Some(pub_first_chunk),
            sig_msg: Some(sig_msg.clone()),
            signature: signature.clone(),
            signer_pub_key_x: Some(from_x.clone()),
            signer_pub_key_y: Some(from_y.clone()),
            args: op_args.clone(),
            lhs: from_operation_branch_before.clone(),
            rhs: to_operation_branch_before.clone(),
        };

        let operation_one = Operation {
            new_root: Some(final_root.clone()),
            tx_type: Some(Fr::from_str("5").unwrap()),
            chunk: Some(Fr::from_str("1").unwrap()),
            pubdata_chunk: Some(pub_second_chunk),
            sig_msg: Some(sig_msg.clone()),
            signature: signature.clone(),
            signer_pub_key_x: Some(from_x.clone()),
            signer_pub_key_y: Some(from_y.clone()),
            args: op_args.clone(),
            lhs: from_operation_branch_after.clone(),
            rhs: to_operation_branch_after.clone(),
        };

        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let instance = FranklinCircuit {
                params,
                old_root: Some(initial_root),
                new_root: Some(final_root),
                operations: vec![operation_zero, operation_one],
                pub_data_commitment: Some(public_data_commitment),
                block_number: Some(Fr::one()),
                validator_address: Some(validator_address),
            };

            instance.synthesize(&mut cs).unwrap();

            println!("{}", cs.find_unconstrained());

            println!("{}", cs.num_constraints());

            let err = cs.which_is_unsatisfied();
            if err.is_some() {
                panic!("ERROR satisfying in {}", err.unwrap());
            }
        }
    }
}
