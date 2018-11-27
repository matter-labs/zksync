use ff::{
    PrimeField,
    PrimeFieldRepr,
    Field,
};

use bellman::{
    SynthesisError,
    ConstraintSystem,
    Circuit
};

use jubjub::{
    JubjubEngine,
    FixedGenerators,
    Unknown,
    edwards,
    JubjubParams
};

use super::Assignment;
use super::boolean;
use super::ecc;
use super::pedersen_hash;
use super::blake2s;
use super::num;
use super::multipack;
use super::num::{AllocatedNum, Num};

use ::eddsa::{
    Signature,
    PublicKey
};

use super::plasma_constants;
use super::float_point::*;
use super::baby_eddsa::EddsaSignature;

/// This is transaction data

#[derive(Clone)]
pub struct Transaction<E: JubjubEngine> {
    pub from: Option<E::Fr>,
    pub to: Option<E::Fr>,
    pub amount: Option<E::Fr>,
    pub fee: Option<E::Fr>,
    pub nonce: Option<E::Fr>,
    pub good_until_block: Option<E::Fr>,
    pub signature: Option<TransactionSignature<E>>
}

#[derive(Clone)]
pub struct TransactionSignature<E: JubjubEngine> {
    pub r: edwards::Point<E, Unknown>,
    pub s: E::Fr,
}

#[derive(Clone)]
pub struct TransactionWitness<E: JubjubEngine> {
    /// The authentication path of the "from" in the tree
    pub auth_path_from: Vec<Option<(E::Fr, bool)>>,
    pub balance_from: Option<E::Fr>,
    pub nonce_from: Option<E::Fr>,
    pub pub_x_from: Option<E::Fr>,
    pub pub_y_from: Option<E::Fr>,

    /// The authentication path of the "to" in the tree
    pub auth_path_to: Vec<Option<(E::Fr, bool)>>,
    pub balance_to: Option<E::Fr>,
    pub nonce_to: Option<E::Fr>,
    pub pub_x_to: Option<E::Fr>,
    pub pub_y_to: Option<E::Fr>
}

/// This is an instance of the `Spend` circuit.
pub struct Update<'a, E: JubjubEngine> {
    pub params: &'a E::Params,

    // number of transactions per block
    pub number_of_transactions: usize,

    /// The old root of the tree
    pub old_root: Option<E::Fr>,

    /// The new root of the tree
    pub new_root: Option<E::Fr>,

    /// Final truncated rolling SHA256
    pub public_data_commitment: Option<E::Fr>,

    /// Block number
    pub block_number: Option<E::Fr>,

    /// Total fee
    pub total_fee: Option<E::Fr>,

    /// Transactions for this block
    pub transactions: Vec<Option<(Transaction<E>, TransactionWitness<E>)>>,
}

impl<'a, E: JubjubEngine> Circuit<E> for Update<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError>
    {
        // Check that transactions are in a right quantity
        assert!(self.number_of_transactions == self.transactions.len());

        let old_root_value = self.old_root.unwrap();
        // Expose inputs and do the bits decomposition of hash
        let mut old_root = AllocatedNum::alloc(
            cs.namespace(|| "old root"),
            || Ok(old_root_value.clone())
        )?;
        old_root.inputize(cs.namespace(|| "old root input"))?;

        let new_root_value = self.new_root.unwrap();
        let new_root = AllocatedNum::alloc(
            cs.namespace(|| "new root"),
            || Ok(new_root_value.clone())
        )?;
        new_root.inputize(cs.namespace(|| "new root input"))?;

        let rolling_hash_value = self.public_data_commitment.unwrap();
        let rolling_hash = AllocatedNum::alloc(
            cs.namespace(|| "rolling hash"),
            || Ok(rolling_hash_value.clone())
        )?;
        rolling_hash.inputize(cs.namespace(|| "rolling hash input"))?;

        let mut hash_bits = boolean::field_into_boolean_vec_le(
            cs.namespace(|| "rolling hash bits"), 
            rolling_hash.get_value()
        )?;

        assert_eq!(hash_bits.len(), E::Fr::NUM_BITS as usize);

        // Pad hash bits to 256 bits for use in hashing rounds

        for _ in 0..(256 - hash_bits.len()) {
            hash_bits.push(boolean::Boolean::Constant(false));
        }

        let mut fees = vec![];

        let public_generator = self.params.generator(FixedGenerators::SpendingKeyGenerator).clone();
        let generator = ecc::EdwardsPoint::witness(cs.namespace(|| "allocate public generator"), Some(public_generator), self.params).unwrap();

        for (i, tx) in self.transactions.into_iter().enumerate() {
            let (intermediate_root, fee, _) = apply_transaction(
                cs.namespace(|| format!("applying transaction {}", i)),
                old_root,
                tx, 
                self.params,
                generator.clone()
            ).unwrap();
            old_root = intermediate_root;
            fees.push(fee);
        }

        // Ok, now we need to update the old root by applying transactions in sequence

        Ok(())
    }
}

// returns a bit vector with ones up to the first point of divergence
fn find_common_prefix<E, CS>(
        mut cs: CS,
        a: Vec<boolean::Boolean>,
        b: Vec<boolean::Boolean>
    ) -> Result<Vec<boolean::Boolean>, SynthesisError>
        where E: JubjubEngine,
        CS: ConstraintSystem<E>
{
    assert_eq!(a.len(), b.len());
    let mut result = vec![];

    // this is how it usually works
    // - calculate result like you work with normal operators
    // - constraint

    let mut first_divergence_found = false;

    for (a_bit, b_bit) in a.iter().zip(b.iter()) {
        if first_divergence_found {
            result.push(boolean::Boolean::Constant(false));
        } else {
            if a_bit.clone().get_value().unwrap() == b_bit.clone().get_value().unwrap() {
                result.push(boolean::Boolean::Constant(true));
            } else {
                first_divergence_found = true;
                result.push(boolean::Boolean::Constant(false));
            }
        }
    }

    for (i, ((a_bit, b_bit), mask_bit) ) in a.iter().zip(b.iter()).zip(result.clone().iter()).enumerate() {
        // This calculated bitmask makes it easy to constraint equality
        // first we calculate an AND between bitmask and vectors A and B
        let a_masked = boolean::Boolean::and(
            cs.namespace(|| format!("bitmask vector a for bit {}", i)), 
            &a_bit, 
            &mask_bit
        ).unwrap();

        let b_masked = boolean::Boolean::and(
            cs.namespace(|| format!("bitmask vector b for bit {}", i)), 
            &b_bit, 
            &mask_bit
        ).unwrap();

        boolean::Boolean::enforce_equal(
            cs.namespace(|| format!("constraint bitmasked values equal for bit {}", i)),
            &a_masked, 
            &b_masked
        ).unwrap();
    }

    Ok(result)
}

/// Applies one transaction to the tree,
/// outputs a new root
fn apply_transaction<E, CS>(
    mut cs: CS,
    old_root: AllocatedNum<E>,
    transaction: Option<(Transaction<E>, TransactionWitness<E>)>,
    params: &E::Params,
    generator: ecc::EdwardsPoint<E>
) -> Result<(AllocatedNum<E>, AllocatedNum<E>, AllocatedNum<E>), SynthesisError>
    where E: JubjubEngine,
          CS: ConstraintSystem<E>
{
    let tx_data = transaction.unwrap();
    let tx = tx_data.0;
    let tx_witness = tx_data.1;

    // before having fun with leafs calculate the common prefix
    // of two audit paths

    let mut common_prefix: Vec<boolean::Boolean> = vec![];

    {
        let cs = & mut cs.namespace(|| "common prefix search");
        
        let mut reversed_path_from = tx_witness.auth_path_from.clone();
        reversed_path_from.reverse();
        let bitmap_path_from: Vec<boolean::Boolean> = reversed_path_from.clone().into_iter().enumerate().map(|(i, e)| 
        {
            let bit = boolean::Boolean::from(
                boolean::AllocatedBit::alloc(
                    cs.namespace(|| format!("merkle tree path for from leaf for bit {}", i)),
                    e.map(|e| e.1)
                ).unwrap()
            );
            bit
        }
        ).collect();

        let mut reversed_path_to = tx_witness.auth_path_to.clone();
        reversed_path_to.reverse();
        let bitmap_path_to: Vec<boolean::Boolean> = reversed_path_to.clone().into_iter().enumerate().map(|(i, e)| 
        {
            let bit = boolean::Boolean::from(
                boolean::AllocatedBit::alloc(
                    cs.namespace(|| format!("merkle tree path for to leaf for bit {}", i)),
                    e.map(|e| e.1)
                ).unwrap()
            );
            bit
        }
        ).collect();

        common_prefix = find_common_prefix(
            cs.namespace(|| "common prefix search"), 
            bitmap_path_from,
            bitmap_path_to
        ).unwrap();

        // Common prefix is found, not we enforce equality of 
        // audit path elements on a common prefix

        for (i, ((e_from, e_to), bitmask_bit)) in reversed_path_from.into_iter().zip(reversed_path_to.into_iter()).zip(common_prefix.clone().into_iter()).enumerate()
        {
            let path_element_from = num::AllocatedNum::alloc(
                cs.namespace(|| format!("path element from {}", i)),
                || {
                    Ok(e_from.get()?.0)
                }
            )?;

            let path_element_to = num::AllocatedNum::alloc(
                cs.namespace(|| format!("path element to {}", i)),
                || {
                    Ok(e_to.get()?.0)
                }
            )?;

            cs.enforce(
                || format!("enforce audit path equality for {}", i),
                |lc| lc + path_element_from.get_variable() - path_element_to.get_variable(),
                |_| bitmask_bit.lc(CS::one(), E::Fr::one()),
                |lc| lc
            );
        }
    }

    // Now we calculate leaf value commitment

    let mut leaf_content = vec![];

    let mut value_content_from = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "from leaf amount bits"), 
        tx_witness.balance_from
    ).unwrap();

    value_content_from.truncate(*plasma_constants::BALANCE_BIT_WIDTH);
    leaf_content.extend(value_content_from.clone());

    let mut nonce_content_from = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "from leaf nonce bits"), 
        tx_witness.nonce_from
    ).unwrap();

    nonce_content_from.truncate(*plasma_constants::NONCE_BIT_WIDTH);
    leaf_content.extend(nonce_content_from.clone());

    let mut pub_x_content_from = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "from leaf pub_x bits"), 
        tx_witness.pub_x_from
    ).unwrap();

    for _ in 0..(*plasma_constants::FR_BIT_WIDTH - pub_x_content_from.len())
    {
        pub_x_content_from.push(boolean::Boolean::Constant(false));
    }
    leaf_content.extend(pub_x_content_from.clone());

    let mut pub_y_content_from = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "from leaf pub_y bits"), 
        tx_witness.pub_y_from
    ).unwrap();

    for _ in 0..(*plasma_constants::FR_BIT_WIDTH - pub_y_content_from.len())
    {
        pub_y_content_from.push(boolean::Boolean::Constant(false));
    }
    leaf_content.extend(pub_y_content_from.clone());

    assert_eq!(leaf_content.len(), *plasma_constants::BALANCE_BIT_WIDTH 
                                + *plasma_constants::NONCE_BIT_WIDTH
                                + 2 * (*plasma_constants::FR_BIT_WIDTH)
    );

    // Compute the hash of the from leaf
    let mut from_leaf_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "from leaf content hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &leaf_content,
        params
    )?;

    // Constraint that "from" field in transaction is 
    // equal to the merkle proof path

    let mut from_path_bits = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "from bit decomposition"), 
        tx.from
    ).unwrap();

    from_path_bits.truncate(*plasma_constants::BALANCE_TREE_DEPTH);

    // This is an injective encoding, as cur is a
    // point in the prime order subgroup.
    let mut cur_from = from_leaf_hash.get_x().clone();

    let audit_path_from = tx_witness.auth_path_from.clone();
    // Ascend the merkle tree authentication path
    for (i, e) in audit_path_from.into_iter().enumerate() {
        let cs = &mut cs.namespace(|| format!("from merkle tree hash {}", i));

        // Determines if the current subtree is the "right" leaf at this
        // depth of the tree.
        let cur_is_right = boolean::Boolean::from(boolean::AllocatedBit::alloc(
            cs.namespace(|| "position bit"),
            e.map(|e| e.1)
        )?);

        // Constraint this bit immediately
        boolean::Boolean::enforce_equal(
            cs.namespace(|| "position bit is equal to from field bit"),
            &cur_is_right, 
            &from_path_bits[i]
        )?;

        // Witness the authentication path element adjacent
        // at this depth.
        let path_element = num::AllocatedNum::alloc(
            cs.namespace(|| "path element"),
            || {
                Ok(e.get()?.0)
            }
        )?;

        // Swap the two if the current subtree is on the right
        let (xl, xr) = num::AllocatedNum::conditionally_reverse(
            cs.namespace(|| "conditional reversal of preimage"),
            &cur_from,
            &path_element,
            &cur_is_right
        )?;

        // We don't need to be strict, because the function is
        // collision-resistant. If the prover witnesses a congruency,
        // they will be unable to find an authentication path in the
        // tree with high probability.
        let mut preimage = vec![];
        preimage.extend(xl.into_bits_le(cs.namespace(|| "xl into bits"))?);
        preimage.extend(xr.into_bits_le(cs.namespace(|| "xr into bits"))?);

        // Compute the new subtree value
        cur_from = pedersen_hash::pedersen_hash(
            cs.namespace(|| "computation of pedersen hash"),
            pedersen_hash::Personalization::MerkleTree(i),
            &preimage,
            params
        )?.get_x().clone(); // Injective encoding
    }

    // enforce old root before update
    cs.enforce(
        || "enforce correct old root for from leaf",
        |lc| lc + cur_from.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + old_root.get_variable()
    );

    // Do the same for "to" leaf

    leaf_content = vec![];

    let mut value_content_to = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "to leaf amount bits"), 
        tx_witness.balance_to
    ).unwrap();

    value_content_to.truncate(*plasma_constants::BALANCE_BIT_WIDTH);
    leaf_content.extend(value_content_to.clone());

    let mut nonce_content_to = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "to leaf nonce bits"), 
        tx_witness.nonce_to
    ).unwrap();

    nonce_content_to.truncate(*plasma_constants::NONCE_BIT_WIDTH);
    leaf_content.extend(nonce_content_to.clone());

    let mut pub_x_content_to = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "to leaf pub_x bits"), 
        tx_witness.pub_x_to
    ).unwrap();

    for _ in 0..(*plasma_constants::FR_BIT_WIDTH - pub_x_content_to.len())
    {
        pub_x_content_to.push(boolean::Boolean::Constant(false));
    }
    leaf_content.extend(pub_x_content_to.clone());

    let mut pub_y_content_to = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "to leaf pub_y bits"), 
        tx_witness.pub_y_to
    ).unwrap();

    for _ in 0..(*plasma_constants::FR_BIT_WIDTH - pub_y_content_to.len())
    {
        pub_y_content_to.push(boolean::Boolean::Constant(false));
    }
    leaf_content.extend(pub_y_content_to.clone());

    assert_eq!(leaf_content.len(), *plasma_constants::BALANCE_BIT_WIDTH 
                                + *plasma_constants::NONCE_BIT_WIDTH
                                + 2 * (*plasma_constants::FR_BIT_WIDTH)
    );

    // Compute the hash of the from leaf
    let mut to_leaf_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "to leaf content hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &leaf_content,
        params
    )?;

    // Constraint that "from" field in transaction is 
    // equal to the merkle proof path

    let mut to_path_bits = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "to bit decomposition"), 
        tx.to
    ).unwrap();

    to_path_bits.truncate(*plasma_constants::BALANCE_TREE_DEPTH);

    // This is an injective encoding, as cur is a
    // point in the prime order subgroup.
    let mut cur_to = to_leaf_hash.get_x().clone();

    let audit_path_to = tx_witness.auth_path_to.clone();
    // Ascend the merkle tree authentication path
    for (i, e) in audit_path_to.into_iter().enumerate() {
        let cs = &mut cs.namespace(|| format!("to merkle tree hash {}", i));

        // Determines if the current subtree is the "right" leaf at this
        // depth of the tree.
        let cur_is_right = boolean::Boolean::from(boolean::AllocatedBit::alloc(
            cs.namespace(|| "position bit"),
            e.map(|e| e.1)
        )?);

        // Constraint this bit immediately
        boolean::Boolean::enforce_equal(
            cs.namespace(|| "position bit is equal to from field bit"),
            &cur_is_right, 
            &to_path_bits[i]
        )?;

        // Witness the authentication path element adjacent
        // at this depth.
        let path_element = num::AllocatedNum::alloc(
            cs.namespace(|| "path element"),
            || {
                Ok(e.get()?.0)
            }
        )?;

        // Swap the two if the current subtree is on the right
        let (xl, xr) = num::AllocatedNum::conditionally_reverse(
            cs.namespace(|| "conditional reversal of preimage"),
            &cur_to,
            &path_element,
            &cur_is_right
        )?;

        // We don't need to be strict, because the function is
        // collision-resistant. If the prover witnesses a congruency,
        // they will be unable to find an authentication path in the
        // tree with high probability.
        let mut preimage = vec![];
        preimage.extend(xl.into_bits_le(cs.namespace(|| "xl into bits"))?);
        preimage.extend(xr.into_bits_le(cs.namespace(|| "xr into bits"))?);

        // Compute the new subtree value
        cur_to = pedersen_hash::pedersen_hash(
            cs.namespace(|| "computation of pedersen hash"),
            pedersen_hash::Personalization::MerkleTree(i),
            &preimage,
            params
        )?.get_x().clone(); // Injective encoding
    }

    // enforce old root before update
    cs.enforce(
        || "enforce correct old root for to leaf",
        |lc| lc + cur_to.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + old_root.get_variable()
    );

    // Ok, old leaf values are exposed, so we can check 
    // the signature and parse the rest of transaction data

    let mut message_bits = vec![];

    message_bits.extend(from_path_bits.clone());
    message_bits.extend(to_path_bits.clone());

    let mut amount_bits = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "amount bits"),
        tx.amount
    ).unwrap();

    amount_bits.truncate(*plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH);
    message_bits.extend(amount_bits.clone());

    let mut fee_bits = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "fee bits"),
        tx.fee
    ).unwrap();

    fee_bits.truncate(*plasma_constants::FEE_EXPONENT_BIT_WIDTH + *plasma_constants::FEE_MANTISSA_BIT_WIDTH);
    message_bits.extend(fee_bits.clone());

    let mut block_number_bits = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "block number bits"),
        tx.good_until_block
    ).unwrap();

    block_number_bits.truncate(*plasma_constants::BLOCK_NUMBER_BIT_WIDTH);
    message_bits.extend(block_number_bits.clone());

    let sender_pk_x = AllocatedNum::alloc(
        cs.namespace(|| "sender public key x"),
        || Ok(tx_witness.pub_x_from.unwrap())
    ).unwrap();

    let sender_pk_y = AllocatedNum::alloc(
        cs.namespace(|| "sender public key y"),
        || Ok(tx_witness.pub_y_from.unwrap())
    ).unwrap();

    let sender_pk = ecc::EdwardsPoint::interpret(
        cs.namespace(|| "sender public key"),
        &sender_pk_x,
        &sender_pk_y,
        params
    ).unwrap();

    let tx_signature = tx.clone().signature.unwrap();

    let (signature_r_x_value, signature_r_y_value) = tx_signature.r.into_xy();

    let signature_r_x = AllocatedNum::alloc(
        cs.namespace(|| "signature r x"),
        || Ok(signature_r_x_value)
    ).unwrap();

    let signature_r_y = AllocatedNum::alloc(
        cs.namespace(|| "signature r y"),
        || Ok(signature_r_y_value)
    ).unwrap();

    let signature_r = ecc::EdwardsPoint::interpret(
        cs.namespace(|| "signature r"),
        &signature_r_x,
        &signature_r_y,
        params
    ).unwrap();

    let signature_s = AllocatedNum::alloc(
        cs.namespace(|| "signature s"),
        || Ok(tx_signature.s)
    ).unwrap();

    let signature = EddsaSignature {
        r: signature_r,
        s: signature_s,
        pk: sender_pk
    };

    let max_message_len = *plasma_constants::BALANCE_TREE_DEPTH 
                        + *plasma_constants::BALANCE_TREE_DEPTH 
                        + *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH 
                        + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH
                        + *plasma_constants::FEE_EXPONENT_BIT_WIDTH
                        + *plasma_constants::FEE_MANTISSA_BIT_WIDTH
                        + *plasma_constants::BLOCK_NUMBER_BIT_WIDTH;

    signature.verify_raw_message_signature(
        cs.namespace(|| "verify transaction signature"),
        params, 
        &message_bits,
        generator,
        max_message_len
    )?;

    let amount = parse_with_exponent_le(
        cs.namespace(|| "parse amount"),
        &amount_bits,
        *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10
    ).unwrap();

    let fee = parse_with_exponent_le(
        cs.namespace(|| "parse fee"),
        &fee_bits,
        *plasma_constants::FEE_EXPONENT_BIT_WIDTH,
        *plasma_constants::FEE_MANTISSA_BIT_WIDTH,
        10
    ).unwrap();

    // repack balances as we have truncated bit decompositions already
    let mut old_balance_from_lc = Num::<E>::zero();
    let mut coeff = E::Fr::one();
    for bit in value_content_from {
        old_balance_from_lc = old_balance_from_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let mut old_balance_to_lc = Num::<E>::zero();
    coeff = E::Fr::one();
    for bit in value_content_to {
        old_balance_to_lc = old_balance_to_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let mut nonce_lc = Num::<E>::zero();
    coeff = E::Fr::one();
    for bit in nonce_content_from {
        nonce_lc = nonce_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    // TODO this is insecure, keep for now, use UInt32 gadgets later for nonce and manual implementation for UInt128

    let old_balance_from = AllocatedNum::alloc(
        cs.namespace(|| "allocate old balance from"),
        || Ok(old_balance_from_lc.get_value().unwrap())
    ).unwrap();

    cs.enforce(
        || "pack old balance from",
        |lc| lc + old_balance_from.get_variable(),
        |lc| lc + CS::one(),
        |_| old_balance_from_lc.lc(E::Fr::one())
    );

    let old_balance_to = AllocatedNum::alloc(
        cs.namespace(|| "allocate old balance to"),
        || Ok(old_balance_to_lc.get_value().unwrap())
    ).unwrap();

    cs.enforce(
        || "pack old balance to",
        |lc| lc + old_balance_to.get_variable(),
        |lc| lc + CS::one(),
        |_| old_balance_to_lc.lc(E::Fr::one())
    );

    let nonce = AllocatedNum::alloc(
        cs.namespace(|| "nonce"),
        || Ok(nonce_lc.get_value().unwrap())
    ).unwrap();

    cs.enforce(
        || "pack nonce",
        |lc| lc + nonce.get_variable(),
        |lc| lc + CS::one(),
        |_| nonce_lc.lc(E::Fr::one())
    );

    let mut new_balance_from_value = old_balance_from.get_value().unwrap();
    new_balance_from_value.sub_assign(&amount.get_value().clone().unwrap());
    new_balance_from_value.sub_assign(&fee.get_value().clone().unwrap());

    let new_balance_from = AllocatedNum::alloc(
        cs.namespace(|| "new balance from"),
        || Ok(new_balance_from_value)
    ).unwrap();

    // constraint no overflow
    num::AllocatedNum::limit_number_of_bits(
        cs.namespace(|| "limit number of bits for new balance from"),
        &new_balance_from,
        *plasma_constants::NONCE_BIT_WIDTH
    )?;

    // enforce reduction of balance
    cs.enforce(
        || "enforce sender's balance reduced",
        |lc| lc + old_balance_from.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + new_balance_from.get_variable() + fee.get_variable() + amount.get_variable()
    );

    let mut new_balance_to_value = old_balance_from.get_value().unwrap();
    new_balance_to_value.add_assign(&amount.get_value().clone().unwrap());

    let new_balance_to = AllocatedNum::alloc(
        cs.namespace(|| "new balance to"),
        || Ok(new_balance_to_value)
    ).unwrap();

    // constraint no overflow
    num::AllocatedNum::limit_number_of_bits(
        cs.namespace(|| "limit number of bits for new balance to"),
        &new_balance_to,
        *plasma_constants::BALANCE_BIT_WIDTH
    )?;

    // enforce increase of balance
    cs.enforce(
        || "enforce recipients's balance increased",
        |lc| lc + new_balance_to.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + old_balance_from.get_variable() + amount.get_variable()
    );

    let mut new_nonce_value = nonce.get_value().unwrap();
    new_nonce_value.add_assign(&E::Fr::one());

    let new_nonce = AllocatedNum::alloc(
        cs.namespace(|| "new nonce"),
        || Ok(new_nonce_value)
    ).unwrap();

    // constraint no overflow
    num::AllocatedNum::limit_number_of_bits(
        cs.namespace(|| "limit number of bits for new nonce from"),
        &new_nonce,
        *plasma_constants::BALANCE_BIT_WIDTH
    )?;

    // enforce increase of balance
    cs.enforce(
        || "enforce sender's nonce to increase",
        |lc| lc + new_nonce.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + nonce.get_variable() + CS::one()
    );

    let allocated_block_number = AllocatedNum::alloc(
        cs.namespace(|| "allocate block number in transaction"),
        || Ok(tx.good_until_block.clone().unwrap())
    ).unwrap();

    // Now we should assemble a new root. It's more tricky as it requires
    // to calculate an intersection point and for a part of the tree that is
    // "below" intersection point use individual merkle brancher,
    // for the intersection - use the other current value,
    // for the rest - use any of the braches, as it's constrained that 
    // those coincide

    // first of new "from" leaf
    {

        leaf_content = vec![];

        // change balance and nonce
        let mut value_content = boolean::field_into_boolean_vec_le(
            cs.namespace(|| "from leaf amount bits update"), 
            new_balance_from.get_value()
        ).unwrap();

        value_content.truncate(*plasma_constants::BALANCE_BIT_WIDTH);
        leaf_content.extend(value_content.clone());

        let mut nonce_content = boolean::field_into_boolean_vec_le(
            cs.namespace(|| "from leaf nonce bits updated"), 
            new_nonce.get_value()
        ).unwrap();

        nonce_content.truncate(*plasma_constants::NONCE_BIT_WIDTH);
        leaf_content.extend(nonce_content);

        // keep public keys
        leaf_content.extend(pub_x_content_from);
        leaf_content.extend(pub_y_content_from);

        assert_eq!(leaf_content.len(), *plasma_constants::BALANCE_BIT_WIDTH 
                                    + *plasma_constants::NONCE_BIT_WIDTH
                                    + 2 * (*plasma_constants::FR_BIT_WIDTH)
        );

        // Compute the hash of the from leaf
        from_leaf_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "from leaf content hash updated"),
            pedersen_hash::Personalization::NoteCommitment,
            &leaf_content,
            params
        )?;
    }

    // first of new "to" leaf
    {

        leaf_content = vec![];

        // change balance only
        let mut value_content = boolean::field_into_boolean_vec_le(
            cs.namespace(|| "to leaf amount bits upted"), 
            new_balance_to.get_value()
        ).unwrap();

        value_content.truncate(*plasma_constants::BALANCE_BIT_WIDTH);
        leaf_content.extend(value_content.clone());

        // everything else remains the same
        leaf_content.extend(nonce_content_to);
        leaf_content.extend(pub_x_content_to);
        leaf_content.extend(pub_y_content_to);

        assert_eq!(leaf_content.len(), *plasma_constants::BALANCE_BIT_WIDTH 
                                    + *plasma_constants::NONCE_BIT_WIDTH
                                    + 2 * (*plasma_constants::FR_BIT_WIDTH)
        );

        // Compute the hash of the from leaf
        to_leaf_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "to leaf content hash updated"),
            pedersen_hash::Personalization::NoteCommitment,
            &leaf_content,
            params
        )?;

    }

    // Now we have to find a "point of intersection"
    // Good for us it's just common prefix interpreted as binary number + 1
    // and bit decomposed

    let mut intersection_point_lc = Num::<E>::zero();
    coeff = E::Fr::one();
    for bit in common_prefix.into_iter() {
        intersection_point_lc = intersection_point_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }
    // and add one
    intersection_point_lc = intersection_point_lc.add_bool_with_coeff(CS::one(), &boolean::Boolean::Constant(true), E::Fr::one());

    let intersection_point = AllocatedNum::alloc(
        cs.namespace(|| "intersection as number"),
        || Ok(intersection_point_lc.get_value().unwrap())
    ).unwrap();

    cs.enforce(
        || "pack intersection",
        |lc| lc + intersection_point.get_variable(),
        |lc| lc + CS::one(),
        |_| intersection_point_lc.lc(E::Fr::one())
    );

    // parse it backwards

    let mut intersection_point_bits = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "unpack intersection"),
        intersection_point.get_value()
    ).unwrap();

    // truncating guarantees that even if the common prefix coincides everywhere
    // up to the last bit, it can still be properly used in next actions
    intersection_point_bits.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
    // reverse cause bits here are counted from root, and later we need from the leaf
    intersection_point_bits.reverse();

    // First assemble new leafs
    cur_from = from_leaf_hash.get_x().clone();
    cur_to = to_leaf_hash.get_x().clone();

    {
        let audit_path_from = tx_witness.auth_path_from.clone();
        let audit_path_to = tx_witness.auth_path_to.clone();
        // Ascend the merkle tree authentication path
        for (i, ((e_from, e_to), intersection_bit) ) in audit_path_from.into_iter().zip(audit_path_to.into_iter()).zip(intersection_point_bits.into_iter()).enumerate() {
            let cs = &mut cs.namespace(|| format!("assemble new state root{}", i));

            let cur_from_is_right = boolean::Boolean::from(boolean::AllocatedBit::alloc(
                cs.namespace(|| "position bit from"),
                e_from.map(|e| e.1)
            )?);

            let cur_to_is_right = boolean::Boolean::from(boolean::AllocatedBit::alloc(
                cs.namespace(|| "position bit to"),
                e_to.map(|e| e.1)
            )?);

            let mut path_element_from = num::AllocatedNum::alloc(
                cs.namespace(|| "path element from"),
                || {
                    Ok(e_from.get()?.0)
                }
            )?;

            let mut path_element_to = num::AllocatedNum::alloc(
                cs.namespace(|| "path element to"),
                || {
                    Ok(e_to.get()?.0)
                }
            )?;

            // Now the most fancy part is to determine when to use path element form witness,
            // or recalculated element from another subtree

            path_element_from = num::AllocatedNum::conditionally_select(
                cs.namespace(|| "conditional select of preimage from"),
                &cur_to,
                &path_element_to, 
                &intersection_bit
            ).unwrap();

            // Swap the two if the current subtree is on the right
            let (xl_from, xr_from) = num::AllocatedNum::conditionally_reverse(
                cs.namespace(|| "conditional reversal of preimage from"),
                &cur_from,
                &path_element_from,
                &cur_from_is_right
            )?;

            let mut preimage_from = vec![];
            preimage_from.extend(xl_from.into_bits_le(cs.namespace(|| "xl_from into bits"))?);
            preimage_from.extend(xr_from.into_bits_le(cs.namespace(|| "xr_from into bits"))?);

            // same for to

            path_element_to = num::AllocatedNum::conditionally_select(
                cs.namespace(|| "conditional select of preimage to"),
                &cur_from,
                &path_element_to, 
                &intersection_bit
            ).unwrap();

            // Swap the two if the current subtree is on the right
            let (xl_to, xr_to) = num::AllocatedNum::conditionally_reverse(
                cs.namespace(|| "conditional reversal of preimage to"),
                &cur_to,
                &path_element_to,
                &cur_from_is_right
            )?;

            let mut preimage_to = vec![];
            preimage_to.extend(xl_to.into_bits_le(cs.namespace(|| "xl_to into bits"))?);
            preimage_to.extend(xr_to.into_bits_le(cs.namespace(|| "xr_to into bits"))?);

            // Compute the new subtree value
            cur_from = pedersen_hash::pedersen_hash(
                cs.namespace(|| "computation of pedersen hash from"),
                pedersen_hash::Personalization::MerkleTree(i),
                &preimage_from,
                params
            )?.get_x().clone(); // Injective encoding

            // Compute the new subtree value
            cur_to = pedersen_hash::pedersen_hash(
                cs.namespace(|| "computation of pedersen hash to"),
                pedersen_hash::Personalization::MerkleTree(i),
                &preimage_to,
                params
            )?.get_x().clone(); // Injective encoding
        }

        // enforce roots are the same
        cs.enforce(
            || "enforce correct new root recalculation",
            |lc| lc + cur_to.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + cur_from.get_variable()
        );
    }
    Ok((cur_from, fee, allocated_block_number))
}


#[test]
fn test_update_circuit_with_bn256() {
    use ff::{Field};
    use pairing::bn256::*;
    use rand::{SeedableRng, Rng, XorShiftRng, Rand};
    use ::circuit::test::*;
    use alt_babyjubjub::{AltJubjubBn256, fs, edwards, PrimeOrder};

    let params = &AltJubjubBn256::new();
    let mut rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1 {

        // No cofactor check in here
        let r = edwards::Point::rand(rng, params);

        let signature = TransactionSignature {
            r: r,
            s: Fr::rand(& mut rng)
        };

        let transaction = Transaction {
            from: Some(Fr::rand(& mut rng)),
            to: Some(Fr::rand(& mut rng)),
            amount: Some(Fr::rand(& mut rng)),
            fee: Some(Fr::rand(& mut rng)),
            nonce: Some(Fr::rand(& mut rng)),
            good_until_block: Some(Fr::rand(& mut rng)),
            signature: Some(signature)
        };

        let pub_from : edwards::Point<Bn256, PrimeOrder> = edwards::Point::rand(rng, params).mul_by_cofactor(params);
        let (pub_from_x, pub_from_y)= pub_from.into_xy();

        let pub_to : edwards::Point<Bn256, PrimeOrder>= edwards::Point::rand(rng, params).mul_by_cofactor(params);
        let (pub_to_x, pub_to_y): (Fr, Fr) = pub_to.into_xy();

        let path_from: Vec<Option<(Fr, bool)>> = (0..(*plasma_constants::BALANCE_TREE_DEPTH)).into_iter().map(|_| {
            let witness = rng.gen();
            let right_or_left = rng.gen();

            Some((witness, right_or_left))
        }).collect();


        let path_to: Vec<Option<(Fr, bool)>> = (0..(*plasma_constants::BALANCE_TREE_DEPTH)).into_iter().map(|_| {
            let witness = rng.gen();
            let right_or_left = rng.gen();

            Some((witness, right_or_left))
        }).collect();

        let transaction_witness = TransactionWitness {
            auth_path_from: path_from,
            balance_from: Some(Fr::rand(& mut rng)),
            nonce_from: Some(Fr::rand(& mut rng)),
            pub_x_from: Some(pub_from_x),
            pub_y_from: Some(pub_from_y),
            auth_path_to: path_to,
            balance_to: Some(Fr::rand(& mut rng)),
            nonce_to: Some(Fr::rand(& mut rng)),
            pub_x_to: Some(pub_to_x),
            pub_y_to: Some(pub_to_y)
        };

        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let old_root = Fr::rand(& mut rng);
            let new_root = Fr::rand(& mut rng);
            let public_data_commitment = Fr::rand(& mut rng);

            let instance = Update {
                params: params,
                number_of_transactions: 1,
                old_root: Some(old_root),
                new_root: Some(new_root),
                public_data_commitment: Some(public_data_commitment),
                block_number: Some(Fr::rand(& mut rng)),
                total_fee: Some(Fr::rand(& mut rng)),
                transactions: vec![Some((transaction, transaction_witness))],
            };

            instance.synthesize(&mut cs).unwrap();

            print!("{}\n", cs.num_constraints());

            assert_eq!(cs.num_inputs(), 4);
            // assert!(cs.is_satisfied());

            // assert_eq!(cs.num_constraints(), 7827);
            // assert_eq!(cs.hash(), "c26d5cdfe6ccd65c03390902c02e11393ea6bb96aae32a7f2ecb12eb9103faee");

            // let expected_cm = payment_address.create_note(
            //     value_commitment.value,
            //     commitment_randomness,
            //     params
            // ).expect("should be valid").cm(params);

            // let expected_value_cm = value_commitment.cm(params).into_xy();

            // let expected_epk = payment_address.g_d(params).expect("should be valid").mul(esk, params);
            // let expected_epk_xy = expected_epk.into_xy();

            // assert_eq!(cs.num_inputs(), 6);
            // assert_eq!(cs.get_input(0, "ONE"), Fr::one());
            // assert_eq!(cs.get_input(1, "value commitment/commitment point/x/input variable"), expected_value_cm.0);
            // assert_eq!(cs.get_input(2, "value commitment/commitment point/y/input variable"), expected_value_cm.1);
            // assert_eq!(cs.get_input(3, "epk/x/input variable"), expected_epk_xy.0);
            // assert_eq!(cs.get_input(4, "epk/y/input variable"), expected_epk_xy.1);
            // assert_eq!(cs.get_input(5, "commitment/input variable"), expected_cm);
        }
    }
}
