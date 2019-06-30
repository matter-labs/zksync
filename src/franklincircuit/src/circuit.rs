use crate::account::AccountContentBase;
use crate::allocated_structures::*;
use crate::operation::{Operation, OperationBranch};
use crate::utils::append_packed_public_key;
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField};
use franklin_crypto::circuit::baby_eddsa::EddsaSignature;
use franklin_crypto::circuit::boolean::{AllocatedBit, Boolean};
use franklin_crypto::circuit::ecc;
use franklin_crypto::circuit::float_point::parse_with_exponent_le;
use franklin_crypto::circuit::num::{AllocatedNum, Num};
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::circuit::polynomial_lookup::{do_the_lookup, generate_powers};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};
use franklinmodels::params as franklin_constants;

const OPERATION_NUMBER: usize = 4;
const DIFFERENT_TRANSACTIONS_TYPE_NUMBER: usize = 11;

struct FranklinCircuit<'a, E: JubjubEngine> {
    pub params: &'a E::Params,
    /// The old root of the tree
    pub old_root: Option<E::Fr>,

    /// The new root of the tree
    pub new_root: Option<E::Fr>,

    pub operations: Vec<Operation<E>>,
}

#[derive(Clone)]
struct PreviousData<E: JubjubEngine> {
    //lhs, rhs //TODO: #merkle
    new_root: AllocatedNum<E>,
}

macro_rules! csprintln {
    ($x:expr,$($arg:tt)*) => (if $x {println!($($arg)*)});
}

// Implementation of our circuit:
//
impl<'a, E: JubjubEngine> Circuit<E> for FranklinCircuit<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let initial_pubdata =
            AllocatedNum::alloc(cs.namespace(|| "initial pubdata"), || Ok(E::Fr::zero()))?;
        initial_pubdata.assert_zero(cs.namespace(|| "initial pubdata is zero"))?;

        let mut rolling_root =
            AllocatedNum::alloc(cs.namespace(|| "rolling_root"), || self.old_root.grab())?;

        let mut next_chunk_number =
            AllocatedNum::alloc(cs.namespace(|| "next_chunk_number"), || Ok(E::Fr::zero()))?;
        next_chunk_number.assert_zero(cs.namespace(|| "initial next_chunk_number"))?;

        let mut allocated_chunk_data: AllocatedChunkData<E>;
        let mut allocated_rolling_pubdata =
            AllocatedNum::alloc(cs.namespace(|| "initial rolling_pubdata"), || {
                Ok(E::Fr::zero())
            })?;
        allocated_rolling_pubdata
            .assert_zero(cs.namespace(|| "initial allocated_rolling_pubdata"))?;

        for (i, operation) in self.operations.iter().enumerate() {
            println!("operation numer {} started \n", i);
            let cs = &mut cs.namespace(|| format!("chunk number {}", i));

            let (next_chunk, chunk_data) = self.verify_correct_chunking(
                &operation,
                &mut next_chunk_number,
                cs.namespace(|| "verify_correct_chunking"),
            )?;
            allocated_chunk_data = chunk_data;
            next_chunk_number = next_chunk;

            allocated_rolling_pubdata = self.accumulate_pubdata(
                cs.namespace(|| "accumulate_pubdata"),
                &operation,
                &allocated_rolling_pubdata,
                &allocated_chunk_data,
            )?;

            let lhs = allocate_operation_branch(cs.namespace(|| "lhs"), &operation.lhs)?;
            let rhs = allocate_operation_branch(cs.namespace(|| "rhs"), &operation.rhs)?;
            let mut current_branch = self.select_branch(
                cs.namespace(|| "select appropriate branch"),
                &lhs,
                &rhs,
                operation,
                &allocated_chunk_data,
            )?;
            let (state_root, is_account_empty) = self
                .check_account_data(cs.namespace(|| "calculate account root"), &current_branch)?;
            println!("state_root: {}", state_root.get_value().unwrap());
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
                &operation,
                &allocated_chunk_data,
                &is_account_empty,
                &allocated_rolling_pubdata,
            )?;
            let (new_state_root, _is_account_empty) = self.check_account_data(
                cs.namespace(|| "calculate new account root"),
                &current_branch,
            )?;
            let operation_new_root =
                AllocatedNum::alloc(cs.namespace(|| "op_new_root"), || operation.new_root.grab())?;
            //TODO inputize
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
        //TODO enforce correct block new root
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
        let tx_type = AllocatedNum::alloc(cs.namespace(|| "tx_type"), || op.tx_type.grab())?;
        enforce_lies_between(
            cs.namespace(|| "tx_type is valid"),
            &tx_type,
            0 as i32,
            DIFFERENT_TRANSACTIONS_TYPE_NUMBER as i32,
        )?;

        let max_chunks_powers = generate_powers(
            cs.namespace(|| "generate powers of max chunks"),
            &tx_type,
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

    fn accumulate_pubdata<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        op: &Operation<E>,
        old_pubdata: &AllocatedNum<E>,
        chunk_data: &AllocatedChunkData<E>,
    ) -> Result<AllocatedNum<E>, SynthesisError> {
        let operation_pub_data =
            AllocatedNum::alloc(cs.namespace(|| "operation_pub_data"), || {
                op.clone().pubdata_chunk.grab()
            })?;

        let shifted_pub_data = AllocatedNum::alloc(cs.namespace(|| "shifted_pub_data"), || {
            let pub_data = op.clone().pubdata_chunk.grab()?;
            let mut computed_data = old_pubdata.get_value().grab()?;
            computed_data.mul_assign(&E::Fr::from_str("256").unwrap());
            computed_data.add_assign(&pub_data);
            Ok(computed_data)
        })?;
        cs.enforce(
            || "enforce one byte shift",
            |lc| {
                lc + (E::Fr::from_str("256").unwrap(), old_pubdata.get_variable())
                    + operation_pub_data.get_variable()
            },
            |lc| lc + CS::one(),
            |lc| lc + shifted_pub_data.get_variable(),
        );

        let zero_chunk_number =
            AllocatedNum::alloc(cs.namespace(|| "initial pubdata"), || Ok(E::Fr::zero()))?;

        zero_chunk_number.assert_zero(cs.namespace(|| "initial pubdata is zero"))?;

        let pubdata = AllocatedNum::select_ifeq(
            cs.namespace(|| "select appropriate pubdata chunk"),
            &zero_chunk_number,
            &chunk_data.chunk_number,
            &operation_pub_data,
            &shifted_pub_data,
        )?;
        Ok(pubdata)
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
            &chunk_data.tx_type,
            &deposit_allocated,
            &left_side,
            &chunk_data.chunk_number,
        )?;
        let operation_branch_base = AllocatedOperationBranchBase {
            account: AccountContentBase {
                nonce: AllocatedNum::select_ifeq(
                    cs.namespace(|| "nonce"),
                    &left_side,
                    &cur_side,
                    &first.base.account.nonce,
                    &second.base.account.nonce,
                )?,
                pub_x: AllocatedNum::select_ifeq(
                    cs.namespace(|| "pub_x"),
                    &left_side,
                    &cur_side,
                    &first.base.account.pub_x,
                    &second.base.account.pub_x,
                )?,
                pub_y: AllocatedNum::select_ifeq(
                    cs.namespace(|| "pub_y"),
                    &left_side,
                    &cur_side,
                    &first.base.account.pub_y,
                    &second.base.account.pub_y,
                )?,
            },
            account_audit_path: select_vec_ifeq(
                cs.namespace(|| "account_audit_path"),
                &left_side,
                &cur_side,
                &first.base.account_audit_path,
                &second.base.account_audit_path,
            )?,
            account_address: AllocatedNum::select_ifeq(
                cs.namespace(|| "account_address"),
                &left_side,
                &cur_side,
                &first.base.account_address,
                &second.base.account_address,
            )?,
            balance_value: AllocatedNum::select_ifeq(
                cs.namespace(|| "balance_value"),
                &left_side,
                &cur_side,
                &first.base.balance_value,
                &second.base.balance_value,
            )?,
            balance_audit_path: select_vec_ifeq(
                cs.namespace(|| "balance_audit_path"),
                &left_side,
                &cur_side,
                &first.base.balance_audit_path,
                &second.base.balance_audit_path,
            )?,
            token: AllocatedNum::select_ifeq(
                cs.namespace(|| "token"),
                &left_side,
                &cur_side,
                &first.base.token,
                &second.base.token,
            )?,
            dummmy_subaccount_value: AllocatedNum::select_ifeq(
                cs.namespace(|| "dummmy_subaccount_value"),
                &left_side,
                &cur_side,
                &first.base.dummmy_subaccount_value,
                &second.base.dummmy_subaccount_value,
            )?,
            subaccount_audit_path: select_vec_ifeq(
                cs.namespace(|| "subaccount_audit_path"),
                &left_side,
                &cur_side,
                &first.base.subaccount_audit_path,
                &second.base.subaccount_audit_path,
            )?,
            subaccount_number: AllocatedNum::select_ifeq(
                cs.namespace(|| "subaccount_number"),
                &left_side,
                &cur_side,
                &first.base.subaccount_number,
                &second.base.subaccount_number,
            )?,
        };

        let bit_form = operation_branch_base
            .make_bit_form(cs.namespace(|| "operation_branch_base_bit_form"))?;
        Ok(AllocatedOperationBranch {
            base: operation_branch_base,
            bits: bit_form,
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
                &cur.bits.account_address,
                &cur.base.account_audit_path,
                self.params,
            )?,
            is_account_empty,
        ))
    }

    fn execute_op<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        mut cur: &mut AllocatedOperationBranch<E>,
        op: &Operation<E>,
        chunk_data: &AllocatedChunkData<E>,
        is_account_empty: &Boolean,
        pubdata: &AllocatedNum<E>,
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

        let allocated_amount = AllocatedNum::alloc(cs.namespace(|| "transaction_amount"), || {
            op.args.amount.grab()
        })?;
        let allocated_fee =
            AllocatedNum::alloc(cs.namespace(|| "transaction_fee"), || op.args.fee.grab())?;
        let allocated_compact_amount =
            AllocatedNum::alloc(cs.namespace(|| "transaction_compact_amount"), || {
                op.args.compact_amount.grab()
            })?;

        let mut allocated_amount_bits =
            allocated_amount.into_bits_le(cs.namespace(|| "transaction_amount_bits"))?;
        allocated_amount_bits.truncate(
            franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH
                + franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        );
        let mut allocated_fee_bits =
            allocated_fee.into_bits_le(cs.namespace(|| "transaction_fee_bits"))?;
        allocated_fee_bits.truncate(
            franklin_constants::FEE_EXPONENT_BIT_WIDTH + franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        );
        let mut allocated_compact_amount_bits =
            allocated_compact_amount.into_bits_le(cs.namespace(|| "compact_amount_fee_bits"))?;
        allocated_compact_amount_bits.truncate(
            franklin_constants::COMPACT_AMOUNT_EXPONENT_BIT_WIDTH
                + franklin_constants::COMPACT_AMOUNT_MANTISSA_BIT_WIDTH,
        );

        let amount = parse_with_exponent_le(
            cs.namespace(|| "parse amount"),
            &allocated_amount_bits,
            *franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
            *franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )?;

        let fee = parse_with_exponent_le(
            cs.namespace(|| "parse fee"),
            &allocated_fee_bits,
            *franklin_constants::FEE_EXPONENT_BIT_WIDTH,
            *franklin_constants::FEE_MANTISSA_BIT_WIDTH,
            10,
        )?;
        let compact_amount = parse_with_exponent_le(
            cs.namespace(|| "parse compact amount"),
            &allocated_compact_amount_bits,
            *franklin_constants::COMPACT_AMOUNT_EXPONENT_BIT_WIDTH,
            *franklin_constants::COMPACT_AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )?;
        let is_compact_amount_correct = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "compact amount correct"),
            &amount,
            &compact_amount,
        )?);

        let allocated_message =
            AllocatedNum::alloc(cs.namespace(|| "signature_message_x"), || op.sig_msg.grab())?;
        let mut message_bits =
            allocated_message.into_bits_le(cs.namespace(|| "signature message bits"))?;
        message_bits.truncate(256 as usize);
        let allocated_signer_pubkey_x =
            AllocatedNum::alloc(cs.namespace(|| "signer_pub_x"), || {
                op.signer_pub_key_x.grab()
            })?;
        let allocated_signer_pubkey_y =
            AllocatedNum::alloc(cs.namespace(|| "signer_pub_y"), || {
                op.signer_pub_key_y.grab()
            })?;

        let sender_pk = ecc::EdwardsPoint::interpret(
            cs.namespace(|| "sender public key"),
            &allocated_signer_pubkey_x,
            &allocated_signer_pubkey_y,
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
            &message_bits,
            generator,
            max_message_len,
        )?;

        let new_pubkey_x =
            AllocatedNum::alloc(cs.namespace(|| "new_pub_x"), || op.args.new_pub_x.grab())?;
        let new_pubkey_y =
            AllocatedNum::alloc(cs.namespace(|| "new_pub_y"), || op.args.new_pub_y.grab())?;
        let mut new_pubkey_x_bits = new_pubkey_x.into_bits_le(cs.namespace(|| "new_pub_x_bits"))?;
        new_pubkey_x_bits.truncate(1);

        let mut new_pubkey_y_bits = new_pubkey_y.into_bits_le(cs.namespace(|| "new_pub_y_bits"))?;
        new_pubkey_y_bits.resize(
            franklin_constants::FR_BIT_WIDTH - 1,
            Boolean::Constant(false),
        );

        let mut new_pubkey_bits = vec![];
        append_packed_public_key(&mut new_pubkey_bits, new_pubkey_x_bits, new_pubkey_y_bits);
        let new_pubkey_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "new_pubkey_hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &new_pubkey_bits,
            self.params,
        )?
        .get_x()
        .clone();

        let mut new_pubkey_hash_bits =
            new_pubkey_hash.into_bits_le(cs.namespace(|| "new_pubkey_hash_bits"))?;
        new_pubkey_hash_bits.truncate(*franklin_constants::NEW_PUBKEY_HASH_WIDTH);

        let a = AllocatedNum::alloc(cs.namespace(|| "a"), || op.args.a.grab())?;
        let b = AllocatedNum::alloc(cs.namespace(|| "b"), || op.args.b.grab())?;

        let diff_a_b = AllocatedNum::alloc(cs.namespace(|| "a-b"), || {
            let mut a_val = a.get_value().grab()?;
            a_val.sub_assign(b.get_value().get()?);
            Ok(a_val)
        })?;
        cs.enforce(
            || "a-b is correct",
            |lc| lc + a.get_variable() - b.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + diff_a_b.get_variable(),
        );
        let mut diff_a_b_bits = diff_a_b.into_bits_le(cs.namespace(|| "a - b bits"))?;
        diff_a_b_bits.truncate(*franklin_constants::BALANCE_BIT_WIDTH);
        let diff_a_b_repacked =
            pack_bits_to_element(cs.namespace(|| "pack a-b bits"), &diff_a_b_bits)?;
        let is_a_geq_b = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "diff equal to repacked"),
            &diff_a_b,
            &diff_a_b_repacked,
        )?);

        let operation_data = AllocatedOperationData {
            new_pubkey_x: new_pubkey_x,
            new_pubkey_y: new_pubkey_y,
            amount: amount,
            amount_packed: allocated_amount_bits,
            fee: fee,
            fee_packed: allocated_fee_bits,
            compact_amount: compact_amount,
            compact_amount_packed: allocated_compact_amount_bits,
            signer_pub_x: allocated_signer_pubkey_x,
            signer_pub_y: allocated_signer_pubkey_y,
            sig_msg_bits: message_bits,
            sig_msg: allocated_message,
            new_pubkey_hash: new_pubkey_hash_bits,
            a: a,
            b: b,
        };

        let op_valid = self.deposit(
            cs.namespace(|| "deposit"),
            &mut cur,
            &chunk_data,
            &is_a_geq_b,
            &is_account_empty,
            &is_compact_amount_correct,
            &operation_data,
            &pubdata,
        )?;
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
        is_compact_amount_correct: &Boolean,
        op_data: &AllocatedOperationData<E>,
        pubdata: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        let allocated_deposit_tx_type =
            AllocatedNum::alloc(cs.namespace(|| "deposit_tx_type"), || Ok(E::Fr::one()))?;
        allocated_deposit_tx_type
            .assert_number(cs.namespace(|| "deposit_tx_type equals one"), &E::Fr::one())?;
        let is_deposit = AllocatedNum::equals(
            cs.namespace(|| "is_deposit"),
            &chunk_data.tx_type,
            &allocated_deposit_tx_type,
        )?;
        let mut is_pubkey_correct = Boolean::Constant(true);
        let is_pub_x_correct = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "new_pub_x equals old_x"),
            &op_data.new_pubkey_x,
            &cur.base.account.pub_x,
        )?);

        let is_pub_y_correct = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "new_pub_y equals old_y"),
            &op_data.new_pubkey_y,
            &cur.base.account.pub_y,
        )?);
        is_pubkey_correct = Boolean::and(
            cs.namespace(|| "and pub_x"),
            &is_pub_x_correct,
            &is_pubkey_correct,
        )?;

        is_pubkey_correct = Boolean::and(
            cs.namespace(|| "and pub_y"),
            &is_pub_y_correct,
            &is_pubkey_correct,
        )?;

        //keys are same or account is empty
        is_pubkey_correct = Boolean::and(
            cs.namespace(|| "acc not empty and keys are not the same"),
            &is_pubkey_correct.not(),
            &is_account_empty.not(),
        )?
        .not();

        let mut pubdata_bits = vec![];
        pubdata_bits.extend(cur.bits.account_address.clone());
        pubdata_bits.extend(cur.bits.token.clone());
        pubdata_bits.extend(op_data.compact_amount_packed.clone());
        pubdata_bits.extend(op_data.new_pubkey_hash.clone());
        pubdata_bits.extend(op_data.fee_packed.clone());
        let mut pubdata_from_lc = Num::<E>::zero();
        let mut coeff = E::Fr::one();
        for bit in &pubdata_bits {
            pubdata_from_lc = pubdata_from_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
            coeff.double();
        }

        let supposed_pubdata_packed =
            AllocatedNum::alloc(cs.namespace(|| "allocate account data packed"), || {
                Ok(*pubdata_from_lc.get_value().get()?)
            })?;

        cs.enforce(
            || "pack account data",
            |lc| lc + supposed_pubdata_packed.get_variable(),
            |lc| lc + CS::one(),
            |_| pubdata_from_lc.lc(E::Fr::one()),
        );
        let is_pubdata_equal = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_pubdata_equal"),
            &supposed_pubdata_packed,
            pubdata,
        )?);
        let _is_pubdata_correct = Boolean::and(
            cs.namespace(|| "is_pubdata_correct"),
            &Boolean::from(chunk_data.is_chunk_last.clone()),
            &is_pubdata_equal.not(),
        )?
        .not();
        let is_a_correct = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "a == amount"),
            &op_data.amount,
            &op_data.a,
        )?);
        let is_b_correct = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "b == fee"),
            &op_data.fee,
            &op_data.b,
        )?);

        let mut tx_valid = Boolean::and(
            cs.namespace(|| "tx_valid and deposit and pubkey_correct"),
            &Boolean::from(is_deposit.clone()),
            &is_pubkey_correct,
        )?;
        println!("is deposit {}", is_deposit.get_value().unwrap());
        println!(
            "is pubkeycorrect {}",
            is_pubkey_correct.get_value().unwrap()
        );

        tx_valid = Boolean::and(
            cs.namespace(|| "tx_valid and is_compact_amount_correct"),
            &tx_valid,
            &is_compact_amount_correct,
        )?;

        tx_valid = Boolean::and(
            cs.namespace(|| "tx_valid and is_a_geq_b"),
            &tx_valid,
            &is_a_geq_b,
        )?;

        tx_valid = Boolean::and(
            cs.namespace(|| "tx_valid and is_a_correct"),
            &tx_valid,
            &is_a_correct,
        )?;

        tx_valid = Boolean::and(
            cs.namespace(|| "tx_valid and is_b_correct"),
            &tx_valid,
            &is_b_correct,
        )?;
        //TODO: uncomment pubdata_check
        // tx_valid = Boolean::and(
        //     cs.namespace(|| "and pubdata_correct"),
        //     &tx_valid,
        //     &is_pubdata_correct,
        // )?;
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
        let updated_balance_value =
            AllocatedNum::alloc(cs.namespace(|| "updated_balance_value"), || {
                let mut new_balance = cur.base.balance_value.get_value().grab()?;
                new_balance.add_assign(&op_data.amount.get_value().grab()?);
                new_balance.sub_assign(&op_data.fee.get_value().grab()?);
                Ok(new_balance)
            })?;
        cs.enforce(
            || "correct_updated_balance",
            |lc| lc + updated_balance_value.get_variable(),
            |lc| lc + CS::one(),
            |lc| {
                lc + cur.base.balance_value.get_variable() + op_data.amount.get_variable()
                    - op_data.fee.get_variable()
            },
        );

        //mutate current branch if it is first chunk of valid deposit transaction
        cur.base.balance_value = AllocatedNum::conditionally_select(
            cs.namespace(|| "update balance if valid first"),
            &updated_balance_value,
            &cur.base.balance_value,
            &is_valid_first,
        )?;
        println!(
            "changed bal data: {}",
            cur.base.balance_value.get_value().unwrap()
        );
        cur.base.account.pub_x = AllocatedNum::conditionally_select(
            cs.namespace(|| "update pub_x if valid first"),
            &op_data.new_pubkey_x,
            &cur.base.account.pub_x,
            &is_valid_first,
        )?;
        println!(
            "changed pubx data: {}",
            cur.base.account.pub_x.get_value().unwrap()
        );
        cur.base.account.pub_y = AllocatedNum::conditionally_select(
            cs.namespace(|| "update pub_y if valid first"),
            &op_data.new_pubkey_y,
            &cur.base.account.pub_y,
            &is_valid_first,
        )?;
        println!(
            "changed puby data: {}",
            cur.base.account.pub_y.get_value().unwrap()
        );

        cur.bits = cur
            .base
            .make_bit_form(cs.namespace(|| "update bit form of branch"))?;
        Ok(tx_valid)
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
        is_compact_amount_correct: &Boolean,
        op_data: &AllocatedOperationData<E>,
        pubdata: &AllocatedNum<E>,
    ) -> Result<Boolean, SynthesisError> {
        let allocated_transfer_tx_type =
            AllocatedNum::alloc(cs.namespace(|| "transfer_tx_type"), || Ok(E::Fr::one()))?;
        allocated_transfer_tx_type
            .assert_number(cs.namespace(|| "transfer_tx_type equals two"), &E::Fr::from_str("two").unwrap())?;
        let is_transfer = AllocatedNum::equals(
            cs.namespace(|| "is_transfer"),
            &chunk_data.tx_type,
            &allocated_transfer_tx_type,
        )?;
       
        let zero = AllocatedNum::alloc(cs.namespace(|| "zero"), || Ok(E::Fr::zero()))?;
        zero.assert_zero(cs.namespace(|| "zero is zero"))?;
        let is_first_chunk = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            &zero,
        )?);

        // construct signature message
        let mut sig_bits = vec![];
        let mut transfer_tx_type_bits = allocated_transfer_tx_type.into_bits_le(cs.namespace(||"transfer_tx_type_bits"))?;
        transfer_tx_type_bits.truncate(*franklin_constants::TX_TYPE_BIT_WIDTH); 
        sig_bits.extend(transfer_tx_type_bits);
        sig_bits.extend(lhs.bits.account_address.clone());
        sig_bits.extend(lhs.bits.token.clone());
        sig_bits.extend(lhs.bits.account.nonce_bits.clone());
        sig_bits.extend(op_data.amount_packed.clone());
        sig_bits.extend(op_data.fee_packed.clone());
        append_packed_public_key(&mut sig_bits, rhs.bits.account.pub_x_bit, rhs.bits.account.pub_x_bit);
        let sig_msg = pack_bits_to_element(cs.namespace(||"sig_msg from bits"), &sig_bits)?;
        let is_sig_msg_correct = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_sig_msg_correct"),
            &op_data.sig_msg_bits,
            &op_data.a,
        )?);
    }

    fn allocate_account_leaf_bits<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        branch: &AllocatedOperationBranch<E>,
    ) -> Result<(Vec<Boolean>, Boolean), SynthesisError> {
        //first we prove calculate root of the subtree to obtain account_leaf_data:
        let mut subtree_data = vec![];
        let balance_data = &branch.bits.balance_value;
        let balance_root = allocate_merkle_root(
            cs.namespace(|| "balance_subtree_root"),
            balance_data,
            &branch.bits.token,
            &branch.base.balance_audit_path,
            self.params,
        )?;
        subtree_data
            .extend(balance_root.into_bits_le(cs.namespace(|| "balance_subtree_root_bits"))?);
        // println!("balance root: {}", balance_root.get_value().unwrap());
        let subaccount_data = &branch.bits.subaccount_data;
        let subaccount_root = allocate_merkle_root(
            cs.namespace(|| "subaccount_subtree_root"),
            subaccount_data,
            &branch.bits.subaccount_number,
            &branch.base.subaccount_audit_path,
            self.params,
        )?;
        subtree_data
            .extend(subaccount_root.into_bits_le(cs.namespace(|| "subaccount_subtree_root_bits"))?);
        // println!("subaccount root: {}", subaccount_root.get_value().unwrap());
        let subtree_root = pedersen_hash::pedersen_hash(
            cs.namespace(|| "subtree_root"),
            pedersen_hash::Personalization::MerkleTree(*franklin_constants::BALANCE_TREE_DEPTH),
            &subtree_data,
            self.params,
        )?
        .get_x()
        .clone();
        // println!("subtree root: {}", subtree_root.get_value().unwrap());
        let mut account_data = vec![];
        account_data.extend(branch.bits.account.nonce_bits.clone());
        append_packed_public_key(
            &mut account_data,
            branch.bits.account.pub_x_bit.clone(),
            branch.bits.account.pub_y_bits.clone(),
        );

        let mut account_data_from_lc = Num::<E>::zero();
        let mut coeff = E::Fr::one();
        for bit in &account_data {
            account_data_from_lc = account_data_from_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
            coeff.double();
        }

        let account_packed =
            AllocatedNum::alloc(cs.namespace(|| "allocate account data packed"), || {
                Ok(*account_data_from_lc.get_value().get()?)
            })?;

        cs.enforce(
            || "pack account data",
            |lc| lc + account_packed.get_variable(),
            |lc| lc + CS::one(),
            |_| account_data_from_lc.lc(E::Fr::one()),
        );

        let zero = AllocatedNum::alloc(cs.namespace(|| "zero"), || Ok(E::Fr::zero()))?;
        zero.assert_zero(cs.namespace(|| "zero is zero"))?;

        let is_account_empty =
            AllocatedNum::equals(cs.namespace(|| "is_account_empty"), &account_packed, &zero)?;
        let mut subtree_root_bits =
            subtree_root.into_bits_le(cs.namespace(|| "subtree_root_bits"))?;
        subtree_root_bits.resize(*franklin_constants::FR_BIT_WIDTH, Boolean::Constant(false));

        account_data.extend(subtree_root_bits);
        // println!("acc len {}", account_data.len());
        // //TODO: assert_eq length of account_data

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
    // println!("leaf_hash: {}", cur_hash.get_value().unwrap());

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
    for i in &[0, 3, 4, 5, 6] {
        //noop, increment_nonce, partial_exit, close_account, escalation
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::zero();
        points.push((x, y));
    }

    for i in &[7, 8, 9, 10] {
        //transfer, create_subaccount, close_subaccount, fill_orders
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::from_str("1").unwrap();
        points.push((x, y));
    }
    for i in &[1, 2] {
        //deposit, transfer_to_new
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::from_str("3").unwrap();
        points.push((x, y));
    }

    let interpolation = interpolate::<E>(&points[..]).expect("must interpolate");
    assert_eq!(interpolation.len(), DIFFERENT_TRANSACTIONS_TYPE_NUMBER);

    interpolation
}

fn pack_bits_to_element<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    bits: &[Boolean],
) -> Result<AllocatedNum<E>, SynthesisError> {
    let mut data_from_lc = Num::<E>::zero();
    let mut coeff = E::Fr::one();
    for bit in bits {
        data_from_lc = data_from_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let data_packed = AllocatedNum::alloc(cs.namespace(|| "allocate account data packed"), || {
        Ok(*data_from_lc.get_value().get()?)
    })?;

    cs.enforce(
        || "pack account data",
        |lc| lc + data_packed.get_variable(),
        |lc| lc + CS::one(),
        |_| data_from_lc.lc(E::Fr::one()),
    );

    Ok(data_packed)
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
        use franklinmodels::{CircuitAccountTree, CircuitBalanceTree, CircuitSubaccountTree};
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
        let subaccount_tree =
            CircuitSubaccountTree::new(*franklin_constants::SUBACCOUNT_TREE_DEPTH as u32);
        let subaccount_root = subaccount_tree.root_hash();
        // println!("test subaccount root: {}", subaccount_root);
        let phasher = PedersenHasher::<Bn256>::default();
        let default_subtree_hash = phasher.compress(
            &balance_root,
            &subaccount_root,
            *franklin_constants::BALANCE_TREE_DEPTH,
        );
        // println!("test subtree root: {}", default_subtree_hash);
        let zero_account = CircuitAccount {
            nonce: Fr::zero(),
            pub_x: Fr::zero(),
            pub_y: Fr::zero(),
            subtree_root_hash: default_subtree_hash,
        };
        let mut tree = CircuitAccountTree::new_with_leaf(
            *franklin_constants::ACCOUNT_TREE_DEPTH as u32,
            zero_account,
        );
        let initial_root = tree.root_hash();
        println!("Initial root = {}", initial_root);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << *franklin_constants::ACCOUNT_TREE_DEPTH);

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
                *franklin_constants::ACCOUNT_TREE_DEPTH as u32,
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

        let transfer_compact_amount_bits = convert_to_float(
            transfer_amount,
            *franklin_constants::COMPACT_AMOUNT_EXPONENT_BIT_WIDTH,
            *franklin_constants::COMPACT_AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let transfer_compact_amount_encoded: Fr =
            le_bit_vector_into_field_element(&transfer_compact_amount_bits);

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

        let subaccount_number: u32 = 2;
        let subaccount_number_fe = Fr::from_str(&subaccount_number.to_string()).unwrap();

        balance_tree.insert(
            token,
            Balance {
                value: transfer_amount_as_field_element,
            },
        );
        let after_deposit_balance_root = balance_tree.root_hash();

        let after_deposit_subtree_hash = phasher.compress(
            &after_deposit_balance_root,
            &subaccount_root,
            *franklin_constants::BALANCE_TREE_DEPTH,
        );

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
                *franklin_constants::ACCOUNT_TREE_DEPTH as u32,
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
        let audit_subaccount_path: Vec<Option<Fr>> = subaccount_tree
            .merkle_path(token)
            .into_iter()
            .map(|e| Some(e.0))
            .collect();

        let op_args = OperationArguments {
            a: Some(transfer_amount_as_field_element.clone()),
            b: Some(fee_as_field_element.clone()),
            amount: Some(transfer_amount_encoded.clone()),
            fee: Some(fee_encoded.clone()),
            compact_amount: Some(transfer_compact_amount_encoded.clone()),
            new_pub_x: Some(sender_x.clone()),
            new_pub_y: Some(sender_y.clone()),
        };
        let operation_branch_before = OperationBranch {
            address: Some(sender_leaf_number_fe),
            token: Some(token_fe),
            subaccount_number: Some(subaccount_number_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(Fr::zero()),
                    pub_x: Some(Fr::zero()),
                    pub_y: Some(Fr::zero()),
                },
                account_path: audit_path.clone(),
                balance_value: Some(Fr::zero()),
                balance_subtree_path: audit_balance_path.clone(),
                dummmy_subaccount_value: Some(Fr::zero()),
                subaccount_path: audit_subaccount_path.clone(),
            },
        };
        let operation_branch_after = OperationBranch::<Bn256> {
            address: Some(sender_leaf_number_fe),
            token: Some(token_fe),
            subaccount_number: Some(subaccount_number_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(Fr::zero()),
                    pub_x: Some(sender_x.clone()),
                    pub_y: Some(sender_y.clone()),
                },
                account_path: audit_path.clone(),
                balance_value: Some(transfer_amount_as_field_element.clone()),
                balance_subtree_path: audit_balance_path.clone(),
                dummmy_subaccount_value: Some(Fr::zero()),
                subaccount_path: audit_subaccount_path.clone(),
            },
        };
        let operation_zero = Operation {
            new_root: Some(new_root.clone()),
            tx_type: Some(Fr::from_str("1").unwrap()),
            chunk: Some(Fr::from_str("0").unwrap()),
            pubdata_chunk: Some(Fr::from_str("1").unwrap()),
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
            pubdata_chunk: Some(Fr::from_str("1").unwrap()),
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
            pubdata_chunk: Some(Fr::from_str("1").unwrap()),
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
            pubdata_chunk: Some(Fr::from_str("1").unwrap()),
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
                ],
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
