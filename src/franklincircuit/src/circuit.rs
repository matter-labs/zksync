use franklinmodels::params as franklin_constants;
use crate::account::{AccountContentBase, AccountContentBitForm, AccountWitness};
use crate::allocated_structures::*;
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
use crate::operation::{Operation, OperationBranch, OperationBranchWitness};
use pairing::bn256::Bn256;
use pairing::Engine;
use crate::utils::append_packed_public_key;

const OPERATION_NUMBER: usize = 4;
const DIFFERENT_TRANSACTIONS_TYPE_NUMBER: usize = 11;

struct FranklinCircuit<'a, E: JubjubEngine> {
    pub params: &'a E::Params,
    /// The old root of the tree
    pub old_root: Option<E::Fr>,

    /// The new root of the tree
    pub new_root: Option<E::Fr>,

    /// Final truncated rolling SHA256
    pub public_data_commitment: Option<E::Fr>,

    pub operations: Vec<Operation<E>>,
}

#[derive(Clone)]
struct PreviousData<E: JubjubEngine> {
    //lhs, rhs //TODO: #merkle
    new_root: AllocatedNum<E>,
}

struct Computed<E: JubjubEngine> {
    // pub last_chunk: Option<AllocatedBit>,
    // pub chunk_number: Option<AllocatedNum<E>>,
    pub pubdata: AllocatedNum<E>,
    pub range_checked: Option<AllocatedBit>,
    pub new_pubkey_hash: Option<AllocatedNum<E>>,
    pub cur: Option<OperationBranch<E>>,
}

macro_rules! csprintln {
    ($x:expr,$($arg:tt)*) => (if $x {println!($($arg)*)});
}

// Implementation of our circuit:
//
impl<'a, E: JubjubEngine> Circuit<E> for FranklinCircuit<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let mut _running_hash: Option<E::Fr>; // TODO: should be initial hash (verify)
        let initial_pubdata =
            AllocatedNum::alloc(cs.namespace(|| "initial pubdata"), || Ok(E::Fr::zero()))?;
        initial_pubdata.assert_zero(cs.namespace(|| "initial pubdata is zero"))?;
        let mut computed = Computed::<E> {
            // last_chunk: None,
            // chunk_number: None,
            pubdata: initial_pubdata,
            range_checked: None,
            new_pubkey_hash: None,
            cur: None,
        };

        let rolling_root =
            AllocatedNum::alloc(cs.namespace(|| "rolling_root"), || self.old_root.grab())?;

        let initial_new_root =
            AllocatedNum::alloc(cs.namespace(|| "zero initial previous new root"), || {
                Ok(E::Fr::zero())
            })?;
        initial_new_root.assert_zero(cs.namespace(|| "initial new_root"))?;
        let prev = PreviousData {
            new_root: initial_new_root,
        };
        let mut next_chunk_number =
            AllocatedNum::alloc(cs.namespace(|| "next_chunk_number"), || Ok(E::Fr::zero()))?;
        next_chunk_number.assert_zero(cs.namespace(|| "initial next_chunk_number"))?;

        let mut allocated_chunk_data: AllocatedChunkData<E>;
        let mut allocated_rolling_pubdata =
            AllocatedNum::alloc(cs.namespace(|| "initial rolling_pubdata"), || {
                Ok(E::Fr::zero())
            })?;
        allocated_rolling_pubdata.assert_zero(cs.namespace(|| "initial next_chunk_number"))?;

        for (i, operation) in self.operations.iter().enumerate() {
            let cs = &mut cs.namespace(|| format!("chunk number {}", i));

            let (next_chunk, chunk_data) = self.verify_correct_chunking(
                &operation,
                &prev,
                &mut next_chunk_number,
                i,
                cs.namespace(|| "verify_correct_chunking"),
            )?;
            allocated_chunk_data = chunk_data;
            next_chunk_number = next_chunk;

            // mutates computed.pubdata
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
            let (new_state_root, is_account_empty) = self.check_account_data(
                cs.namespace(|| "calculate new account root"),
                &current_branch,
            )?;
            let operation_new_root =
                AllocatedNum::alloc(cs.namespace(|| "op_new_root"), || operation.new_root.grab())?;
        }
        //TODO enforce correct block new root
        Ok(())
    }
}
impl<'a, E: JubjubEngine> FranklinCircuit<'a, E> {
    fn verify_correct_chunking<CS: ConstraintSystem<E>>(
        &self,
        op: &Operation<E>,
        _prev: &PreviousData<E>,
        next_chunk_number: &AllocatedNum<E>,
        _index: usize,
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
        op: &Operation<E>,
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
        let (cur_account_leaf_hash, is_account_empty) = self.allocate_account_leaf_hash(
            cs.namespace(|| "allocate current_account_leaf_hash"),
            cur,
        )?;
        let leaf_bits =
            &cur_account_leaf_hash.into_bits_le(cs.namespace(|| "cur_account_leaf_hash_bits"))?;

        Ok((
            allocate_merkle_root(
                cs.namespace(|| "account_merkle_root"),
                leaf_bits,
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
            AllocatedNum::alloc(cs.namespace(|| "new_pub_y"), || op.args.new_pub_x.grab())?;
        let mut new_pubkey_x_bits = new_pubkey_x.into_bits_le(cs.namespace(|| "new_pub_x_bits"))?;
        new_pubkey_x_bits.truncate(1);

        let mut new_pubkey_y_bits = new_pubkey_y.into_bits_le(cs.namespace(|| "new_pub_y_bits"))?;
        new_pubkey_y_bits.truncate(franklin_constants::FR_BIT_WIDTH - 1);

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
            new_pubkey_hash: new_pubkey_hash_bits,
            a: a,
            b: b,
        };

        let op_valid = self.deposit(
            cs.namespace(|| "deposit"),
            &mut cur,
            &op,
            &chunk_data,
            &is_account_empty,
            &operation_data,
            &pubdata,
        )?;

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
        op: &Operation<E>,
        chunk_data: &AllocatedChunkData<E>,
        is_account_empty: &Boolean,
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
        let mut is_pubkey_correct = Boolean::Constant(false);
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
        //TODO rangechecked
        //TODO compact amount correct

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
        let is_pubdata_correct = Boolean::and(
            cs.namespace(|| "is_pubdata_correct"),
            &Boolean::from(chunk_data.is_chunk_last.clone()),
            &is_pubdata_equal.not(),
        )?
        .not();
        //TODO a and b correct

        let mut tx_valid = Boolean::and(
            cs.namespace(|| "deposit and pubkey_corect"),
            &Boolean::from(is_deposit),
            &is_pubkey_correct,
        )?;
        tx_valid = Boolean::and(
            cs.namespace(|| "and pubdata_correct"),
            &tx_valid,
            &is_pubdata_correct,
        )?;

        //TODO precompute
        let zero = AllocatedNum::alloc(cs.namespace(|| "zero"), || Ok(E::Fr::zero()))?;
        zero.assert_zero(cs.namespace(|| "zero is zero"))?;
        let is_first_chunk = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_first_chunk"),
            &chunk_data.chunk_number,
            &zero,
        )?);

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
            |lc| lc + op_data.amount.get_variable() - op_data.fee.get_variable(),
        );

        //mutate current branch if it is first chunk of valid deposit transaction
        cur.base.balance_value = AllocatedNum::conditionally_select(
            cs.namespace(|| "update balance if valid first"),
            &updated_balance_value,
            &cur.base.balance_value,
            &is_valid_first,
        )?;
        cur.base.account.pub_x = AllocatedNum::conditionally_select(
            cs.namespace(|| "update pub_x if valid first"),
            &op_data.new_pubkey_x,
            &cur.base.account.pub_x,
            &is_valid_first,
        )?;

        cur.base.account.pub_y = AllocatedNum::conditionally_select(
            cs.namespace(|| "update pub_y if valid first"),
            &op_data.new_pubkey_y,
            &cur.base.account.pub_y,
            &is_valid_first,
        )?;
        Ok(tx_valid)
    }
    fn allocate_account_leaf_hash<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        branch: &AllocatedOperationBranch<E>,
    ) -> Result<(AllocatedNum<E>, Boolean), SynthesisError> {
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

        let subtree_root = pedersen_hash::pedersen_hash(
            cs.namespace(|| "subtree_root"),
            pedersen_hash::Personalization::NoteCommitment,
            &subtree_data,
            self.params,
        )?
        .get_x()
        .clone();

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

        account_data.extend(subtree_root.into_bits_le(cs.namespace(|| "subtree_root_bits"))?);

        //TODO: assert_eq length of account_data

        let account_leaf_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "account leaf content hash"),
            pedersen_hash::Personalization::NoteCommitment,//TODO change personalization
            &account_data,
            self.params,
        )?
        .get_x()
        .clone();
        Ok((account_leaf_hash.clone(), Boolean::from(is_account_empty)))
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

// returns a bit vector with ones up to the first point of divergence
fn find_common_prefix<E, CS>(
    mut cs: CS,
    a: &[Boolean],
    b: &[Boolean],
) -> Result<Vec<Boolean>, SynthesisError>
where
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
{
    assert_eq!(a.len(), b.len());

    // initiall divergence did NOT happen yet

    let mut no_divergence_bool = Boolean::Constant(true);

    let mut mask_bools = vec![];

    for (i, (a_bit, b_bit)) in a.iter().zip(b.iter()).enumerate() {
        // on common prefix mean a == b AND divergence_bit

        // a == b -> NOT (a XOR b)

        let a_xor_b = Boolean::xor(
            cs.namespace(|| format!("Common prefix a XOR b {}", i)),
            &a_bit,
            &b_bit,
        )?;

        let mask_bool = Boolean::and(
            cs.namespace(|| format!("Common prefix mask bit {}", i)),
            &a_xor_b.not(),
            &no_divergence_bool,
        )?;

        // is no_divergence_bool == true: mask_bool = a == b
        // else: mask_bool == false
        // -->
        // if mask_bool == false: divergence = divergence AND mask_bool

        no_divergence_bool = Boolean::and(
            cs.namespace(|| format!("recalculate divergence bit {}", i)),
            &no_divergence_bool,
            &mask_bool,
        )?;

        mask_bools.push(no_divergence_bool.clone());
    }

    Ok(mask_bools)
}

fn find_intersection_point<E, CS>(
    mut cs: CS,
    from_path_bits: Vec<Boolean>,
    to_path_bits: Vec<Boolean>,
    audit_path_from: &[AllocatedNum<E>],
    audit_path_to: &[AllocatedNum<E>],
    tree_depth: usize,
) -> Result<Vec<Boolean>, SynthesisError>
where
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
{
    assert_eq!(audit_path_from.len(), audit_path_to.len());
    assert_eq!(audit_path_from.len(), tree_depth);

    // Intersection point is the only element required in outside scope
    let mut intersection_point_lc = Num::<E>::zero();

    let mut bitmap_path_from = from_path_bits.clone();
    bitmap_path_from.reverse();

    let mut bitmap_path_to = to_path_bits.clone();
    bitmap_path_to.reverse();

    let common_prefix = find_common_prefix(
        cs.namespace(|| "common prefix search"),
        &bitmap_path_from,
        &bitmap_path_to,
    )?;

    // common prefix is reversed because it's enumerated from the root, while
    // audit path is from the leafs

    let mut common_prefix_reversed = common_prefix.clone();
    common_prefix_reversed.reverse();

    // Common prefix is found, not we enforce equality of
    // audit path elements on a common prefix

    for (i, bitmask_bit) in common_prefix_reversed.into_iter().enumerate() {
        let path_element_from = &audit_path_from[i];
        let path_element_to = &audit_path_to[i];

        cs.enforce(
            || format!("enforce audit path equality for {}", i),
            |lc| lc + path_element_from.get_variable() - path_element_to.get_variable(),
            |_| bitmask_bit.lc(CS::one(), E::Fr::one()),
            |lc| lc,
        );
    }

    // Now we have to find a "point of intersection"
    // Good for us it's just common prefix interpreted as binary number + 1
    // and bit decomposed

    let mut coeff = E::Fr::one();
    for bit in common_prefix.into_iter() {
        intersection_point_lc = intersection_point_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    // and add one
    intersection_point_lc = intersection_point_lc.add_bool_with_coeff(
        CS::one(),
        &Boolean::Constant(true),
        E::Fr::one(),
    );

    // Intersection point is a number with a single bit that indicates how far
    // from the root intersection is

    let intersection_point =
        AllocatedNum::alloc(cs.namespace(|| "intersection as number"), || {
            Ok(*intersection_point_lc.get_value().get()?)
        })?;

    cs.enforce(
        || "pack intersection",
        |lc| lc + intersection_point.get_variable(),
        |lc| lc + CS::one(),
        |_| intersection_point_lc.lc(E::Fr::one()),
    );

    // Intersection point into bits to use for root recalculation
    let mut intersection_point_bits =
        intersection_point.into_bits_le(cs.namespace(|| "unpack intersection"))?;

    // truncating guarantees that even if the common prefix coincides everywhere
    // up to the last bit, it can still be properly used in next actions
    intersection_point_bits.truncate(tree_depth);
    // reverse cause bits here are counted from root, and later we need from the leaf
    intersection_point_bits.reverse();

    Ok(intersection_point_bits)
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


// #[cfg(test)]
// mod tests {

//     use super::*;
//     use franklin_crypto::circuit::test::TestConstraintSystem;

//     #[test]
//     fn test_circuit_success() {
//         let mut cs = TestConstraintSystem::<Bn256>::new();

//         let c = FranklinCircuit {
//             old_root: None,
//             new_root: None,
//             public_data_commitment: None,
//             operations: vec![
//                 Operation::<Bn256>::with_id("0", "1"),
//                 Operation::<Bn256>::with_id("1", "1"),
//                 Operation::<Bn256>::with_id("2", "1"),
//                 Operation::<Bn256>::with_id("3", "1"),
//             ],
//         };

//         c.synthesize(&mut cs).expect("synthesis failed");
//         let unconstrained = cs.find_unconstrained();
//         println!("{}", unconstrained);
//         assert!(unconstrained == "");
//         dbg!(cs.find_unconstrained());
//         dbg!(cs.num_constraints());
//         dbg!(cs.num_inputs());

//         if let Some(token) = cs.which_is_unsatisfied() {
//             eprintln!("Error: {} is unsatisfied", token);
//         }
//         assert!(cs.is_satisfied());

//         let mut cs = TestConstraintSystem::<Bn256>::new();

//         let c = FranklinCircuit {
//             old_root: None,
//             new_root: None,
//             public_data_commitment: None,
//             operations: vec![
//                 Operation::<Bn256>::with_id("0", "7"),
//                 Operation::<Bn256>::with_id("1", "7"),
//                 Operation::<Bn256>::with_id("0", "7"),
//                 Operation::<Bn256>::with_id("1", "7"),
//             ],
//         };

//         c.synthesize(&mut cs).expect("synthesis failed");
//         let unconstrained = cs.find_unconstrained();
//         println!("{}", unconstrained);
//         assert!(unconstrained == "");
//         dbg!(cs.num_constraints());
//         dbg!(cs.num_inputs());

//         if let Some(token) = cs.which_is_unsatisfied() {
//             eprintln!("Error: {} is unsatisfied", token);
//         }
//         assert!(cs.is_satisfied())
//     }
//     #[test]
//     fn test_circuit_failures() {
//         let mut cs = TestConstraintSystem::<Bn256>::new();

//         let c = FranklinCircuit {
//             old_root: None,
//             new_root: None,
//             public_data_commitment: None,
//             operations: vec![
//                 Operation::<Bn256>::with_id("0", "1"),
//                 Operation::<Bn256>::with_id("1", "1"),
//                 Operation::<Bn256>::with_id("2", "1"),
//                 Operation::<Bn256>::with_id("2", "1"),
//             ],
//         };

//         c.synthesize(&mut cs).expect("synthesis failed");
//         dbg!(cs.find_unconstrained());
//         dbg!(cs.num_constraints());
//         dbg!(cs.num_inputs());

//         if let Some(token) = cs.which_is_unsatisfied() {
//             eprintln!("Error: {} is unsatisfied", token);
//         }
//         assert!(!cs.is_satisfied());

//         let mut cs = TestConstraintSystem::<Bn256>::new();

//         let c = FranklinCircuit {
//             old_root: None,
//             new_root: None,
//             public_data_commitment: None,
//             operations: vec![
//                 Operation::<Bn256>::with_id("1", "1"),
//                 Operation::<Bn256>::with_id("2", "1"),
//                 Operation::<Bn256>::with_id("3", "1"),
//                 Operation::<Bn256>::with_id("0", "5"),
//             ],
//         };

//         c.synthesize(&mut cs).expect("synthesis failed");
//         dbg!(cs.find_unconstrained());
//         dbg!(cs.num_constraints());
//         dbg!(cs.num_inputs());

//         if let Some(token) = cs.which_is_unsatisfied() {
//             eprintln!("Error: {} is unsatisfied", token);
//         }
//         assert!(!cs.is_satisfied());

//         let mut cs = TestConstraintSystem::<Bn256>::new();

//         let c = FranklinCircuit {
//             old_root: None,
//             new_root: None,
//             public_data_commitment: None,
//             operations: vec![
//                 Operation::<Bn256>::with_id("0", "1"),
//                 Operation::<Bn256>::with_id("1", "1"),
//                 Operation::<Bn256>::with_id("2", "1"),
//                 Operation::<Bn256>::with_id("4", "1"),
//             ],
//         };

//         c.synthesize(&mut cs).expect("synthesis failed");
//         //NUM.rs
//         dbg!(cs.find_unconstrained());
//         dbg!(cs.num_constraints());
//         dbg!(cs.num_inputs());

//         if let Some(token) = cs.which_is_unsatisfied() {
//             eprintln!("Error: {} is unsatisfied", token);
//         }
//         assert!(!cs.is_satisfied());

//         let mut cs = TestConstraintSystem::<Bn256>::new();

//         let c = FranklinCircuit {
//             old_root: None,
//             new_root: None,
//             public_data_commitment: None,
//             operations: vec![
//                 Operation::<Bn256>::with_id("0", "17"),
//                 Operation::<Bn256>::with_id("1", "17"),
//                 Operation::<Bn256>::with_id("2", "17"),
//                 Operation::<Bn256>::with_id("3", "17"),
//             ],
//         };

//         c.synthesize(&mut cs).expect("synthesis failed");
//         //NUM.rs
//         dbg!(cs.find_unconstrained());
//         dbg!(cs.num_constraints());
//         dbg!(cs.num_inputs());

//         if let Some(token) = cs.which_is_unsatisfied() {
//             eprintln!("Error: {} is unsatisfied", token);
//         }
//         assert!(!cs.is_satisfied());
//     }
// }
