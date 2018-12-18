use ff::{
    PrimeField,
    Field,
    BitIterator,
    PrimeFieldRepr
};

use bellman::{
    SynthesisError,
    ConstraintSystem,
    Circuit
};

use sapling_crypto::jubjub::{
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
use super::sha256;
use super::num;
use super::multipack;
use super::num::{AllocatedNum, Num};
use super::float_point::{parse_with_exponent_le, convert_to_float};
use super::baby_eddsa::EddsaSignature;

use sapling_crypto::eddsa::{
    Signature,
    PrivateKey,
    PublicKey
};

<<<<<<< HEAD
use crate::models::params;
use crate::circuit::utils::le_bit_vector_into_field_element;
use super::transaction::{Transaction};
=======
use super::super::plasma_constants;
use super::super::leaf::{LeafWitness, LeafContent, make_leaf_content};
use crate::circuit::utils::{le_bit_vector_into_field_element, allocate_audit_path, append_packed_public_key, count_number_of_ones};
use super::transaction::{Transaction, TransactionContent};
>>>>>>> more_ff

#[derive(Clone)]
pub struct TransactionWitness<E: JubjubEngine> {
    pub leaf_from: LeafWitness<E>,
    pub auth_path_from: Vec<Option<E::Fr>>,

    pub leaf_to: LeafWitness<E>,
    pub auth_path_to: Vec<Option<E::Fr>>,
}

/// This is an instance of the `Spend` circuit.
pub struct Transfer<'a, E: JubjubEngine> {
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
    pub transactions: Vec<(Transaction<E>, TransactionWitness<E>)>,
}

impl<'a, E: JubjubEngine> Circuit<E> for Transfer<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError>
    {
        // Check that transactions are in a right quantity
        assert!(self.number_of_transactions == self.transactions.len());

        let old_root_value = self.old_root;
        // Expose inputs and do the bits decomposition of hash
        let mut old_root = AllocatedNum::alloc(
            cs.namespace(|| "old root"),
            || Ok(*old_root_value.get()?)
        )?;
        old_root.inputize(cs.namespace(|| "old root input"))?;

        let new_root_value = self.new_root;
        let new_root = AllocatedNum::alloc(
            cs.namespace(|| "new root"),
            || Ok(*new_root_value.get()?)
        )?;
        new_root.inputize(cs.namespace(|| "new root input"))?;

        let rolling_hash_value = self.public_data_commitment;
        let rolling_hash = AllocatedNum::alloc(
            cs.namespace(|| "rolling hash"),
            || Ok(*rolling_hash_value.get()?)
        )?;
        rolling_hash.inputize(cs.namespace(|| "rolling hash input"))?;

        let mut fees = vec![];
        let mut block_numbers = vec![];

        let mut public_data_vector: Vec<boolean::Boolean> = vec![];

        let public_generator = self.params.generator(FixedGenerators::SpendingKeyGenerator).clone();
        let generator = ecc::EdwardsPoint::witness(
            cs.namespace(|| "allocate public generator"),
            Some(public_generator),
            self.params
        )?;

        // Ok, now we need to update the old root by applying transactions in sequence
        let transactions = self.transactions.clone();

        for (i, tx) in transactions.into_iter().enumerate() {
            let (transaction, witness) = tx;
            let (intermediate_root, fee, block_number, public_data) = apply_transaction(
                cs.namespace(|| format!("applying transaction {}", i)),
                old_root,
                transaction, 
                witness,
                self.params,
                generator.clone()
            )?;
            old_root = intermediate_root;
            fees.push(fee);
            block_numbers.push(block_number);

            // flatten the public transaction data
            public_data_vector.extend(public_data.into_iter());
        }

        // constraint the new hash to be equal to updated hash

        cs.enforce(
            || "enforce new root equal to recalculated one",
            |lc| lc + new_root.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + old_root.get_variable()
        );

        // Inside the circuit with work with LE bit order, 
        // so an account number "1" that would have a natural representation of e.g. 0x000001
        // will have a bit decomposition [1, 0, 0, 0, ......]

        // Don't deal with it here, but rather do on application layer when parsing the data!
        // The only requirement is to properly seed initial hash value with block number and fees,
        // as those are going to be naturally represented as Ethereum units

        // First calculate a final fee amount

        let total_fee_allocated = AllocatedNum::alloc(
            cs.namespace(|| "allocate total fees"),
            || {
                let total_fee = self.total_fee;
                Ok(*total_fee.clone().get()?)
            }
        )?;

        cs.enforce(
            || "enforce total fee",
            |lc| lc + total_fee_allocated.get_variable(),
            |lc| lc + CS::one(),
            |lc| {
                let mut final_lc = lc;
                for fee in fees.into_iter() {
                    final_lc = final_lc + fee.get_variable();
                }

                final_lc
                }
        );

        // Then check that for every transaction in this block 
        // the parameter "good until" was greater or equal
        // than the current block number

        let block_number_allocated = AllocatedNum::alloc(
            cs.namespace(|| "allocate block number"),
            || {
                Ok(*self.block_number.get()?)
            }
        )?;

        for (i, block_number_in_tx) in block_numbers.into_iter().enumerate() {
            // first name a new value and constraint that it's a proper subtraction

            let difference_allocated = AllocatedNum::alloc(
                cs.namespace(|| format!("allocate block number difference {}", i)),
                || {
                    let mut difference = *block_number_in_tx.get_value().get()?;
                    difference.sub_assign(self.block_number.get()?);

                    Ok(difference)
                }
            )?;

            // check for overflow

            difference_allocated.limit_number_of_bits(
                cs.namespace(|| format!("check for subtraction overflow {}", i)),
                params::BLOCK_NUMBER_BIT_WIDTH
            )?;

            // enforce proper subtraction
            cs.enforce(
                || format!("enforce subtraction in block number calculation {}", i),
                |lc| lc + difference_allocated.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + block_number_in_tx.get_variable() - block_number_allocated.get_variable()
            );

        }

        // Now it's time to pack the initial SHA256 hash due to Ethereum BE encoding
        // and start rolling the hash

        let mut initial_hash_data: Vec<boolean::Boolean> = vec![];

        // make initial hash as sha256(uint256(block_number)||uint256(total_fees))
        let mut block_number_bits = block_number_allocated.into_bits_le(
            cs.namespace(|| "unpack block number for hashing")
        )?;

        block_number_bits.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));
        block_number_bits.reverse();
        initial_hash_data.extend(block_number_bits.into_iter());

        let mut total_fees_bits = total_fee_allocated.into_bits_le(
            cs.namespace(|| "unpack fees for hashing")
        )?;
        total_fees_bits.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));
        total_fees_bits.reverse();
        initial_hash_data.extend(total_fees_bits.into_iter());

        assert_eq!(initial_hash_data.len(), 512);

        let mut hash_block = sha256::sha256(
            cs.namespace(|| "initial rolling sha256"),
            &initial_hash_data
        )?;

        // // now we do a "dense packing", i.e. take 256 / public_data.len() items 
        // // and push them into the second half of sha256 block

        // let public_data_size = params::BALANCE_TREE_DEPTH 
        //                             + params::BALANCE_TREE_DEPTH
        //                             + params::AMOUNT_EXPONENT_BIT_WIDTH
        //                             + params::AMOUNT_MANTISSA_BIT_WIDTH
        //                             + params::FEE_EXPONENT_BIT_WIDTH
        //                             + params::FEE_MANTISSA_BIT_WIDTH;


        // // pad with zeroes up to the block size
        // let required_padding = 256 - (public_data_vector.len() % public_data_size);

        let mut pack_bits = vec![];
        pack_bits.extend(hash_block);
        pack_bits.extend(public_data_vector.into_iter());

        hash_block = sha256::sha256(
            cs.namespace(|| "hash public data"),
            &pack_bits
        )?;

        // // now pack and enforce equality to the input

        hash_block.reverse();
        hash_block.truncate(E::Fr::CAPACITY as usize);

        let mut packed_hash_lc = Num::<E>::zero();
        let mut coeff = E::Fr::one();
        for bit in hash_block {
            packed_hash_lc = packed_hash_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
            coeff.double();
        }

        cs.enforce(
            || "enforce external data hash equality",
            |lc| lc + rolling_hash.get_variable(),
            |lc| lc + CS::one(),
            |_| packed_hash_lc.lc(E::Fr::one())
        );

        Ok(())
    }
}

// returns a bit vector with ones up to the first point of divergence
fn find_common_prefix<E, CS>(
        mut cs: CS,
        a: &[boolean::Boolean],
        b: &[boolean::Boolean]
    ) -> Result<Vec<boolean::Boolean>, SynthesisError>
        where E: JubjubEngine,
        CS: ConstraintSystem<E>
{
    assert_eq!(a.len(), b.len());

    // initiall divergence did NOT happen yet

    let mut no_divergence_bool = boolean::Boolean::Constant(true);
 
    let mut mask_bools = vec![];

    for (i, (a_bit, b_bit)) in a.iter().zip(b.iter()).enumerate() {

        // on common prefix mean a == b AND divergence_bit

        // a == b -> NOT (a XOR b)

        let a_xor_b = boolean::Boolean::xor(
            cs.namespace(|| format!("Common prefix a XOR b {}", i)),
            &a_bit,
            &b_bit
        )?;

        let mask_bool = boolean::Boolean::and(
            cs.namespace(|| format!("Common prefix mask bit {}", i)),
            &a_xor_b.not(),
            &no_divergence_bool
        )?;

        // is no_divergence_bool == true: mask_bool = a == b
        // else: mask_bool == false
        // -->
        // if mask_bool == false: divergence = divergence AND mask_bool

        no_divergence_bool = boolean::Boolean::and(
            cs.namespace(|| format!("recalculate divergence bit {}", i)),
            &no_divergence_bool,
            &mask_bool
        )?;

        mask_bools.push(no_divergence_bool.clone());
    }

    Ok(mask_bools)
}

fn find_intersection_point<E, CS> (
    mut cs: CS,
    from_path_bits: Vec<boolean::Boolean>,
    to_path_bits: Vec<boolean::Boolean>,
    audit_path_from: &[AllocatedNum<E>],
    audit_path_to: &[AllocatedNum<E>],
) -> Result<Vec<boolean::Boolean>, SynthesisError>
    where E: JubjubEngine,
          CS: ConstraintSystem<E>
{
<<<<<<< HEAD
    // Calculate leaf value commitment

    let mut leaf_content = vec![];

    let value_from_allocated = AllocatedNum::alloc(
        cs.namespace(|| "allocate value from"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.balance_from.clone().get()?)
        }
    )?;

    let mut value_content_from = value_from_allocated.into_bits_le(
        cs.namespace(|| "unpack from leaf value")
    )?;

    value_content_from.truncate(params::BALANCE_BIT_WIDTH);
    leaf_content.extend(value_content_from.clone());

    let nonce_from_allocated = AllocatedNum::alloc(
        cs.namespace(|| "allocate nonce from"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.nonce_from.clone().get()?)
        }
    )?;

    let mut nonce_content_from = nonce_from_allocated.into_bits_le(
        cs.namespace(|| "from leaf nonce bits")
    )?;

    nonce_content_from.truncate(params::NONCE_BIT_WIDTH);
    leaf_content.extend(nonce_content_from.clone());

    // we allocate (witness) public X and Y to use them also later for signature check

    let sender_pk_x = AllocatedNum::alloc(
        cs.namespace(|| "sender public key x"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.pub_x_from.get()?)
        }
    )?;

    let sender_pk_y = AllocatedNum::alloc(
        cs.namespace(|| "sender public key y"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.pub_y_from.get()?)
        }
    )?;

    let mut pub_x_content_from = sender_pk_x.into_bits_le(
        cs.namespace(|| "from leaf pub_x bits")
    )?;
    pub_x_content_from.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));

    leaf_content.extend(pub_x_content_from.clone());

    let mut pub_y_content_from = sender_pk_y.into_bits_le(
        cs.namespace(|| "from leaf pub_y bits")
    )?;
    pub_y_content_from.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));

    leaf_content.extend(pub_y_content_from.clone());

    assert_eq!(leaf_content.len(), params::BALANCE_BIT_WIDTH 
                                + params::NONCE_BIT_WIDTH
                                + 2 * (params::FR_BIT_WIDTH)
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

    let from_address_allocated = AllocatedNum::alloc(
        cs.namespace(|| "sender address"),
        || {
            let tx = &transaction.get()?.0;
            Ok(*tx.from.get()?)
        }
    )?;

    let mut from_path_bits = from_address_allocated.into_bits_le(
        cs.namespace(|| "sender address bit decomposition")
    )?;

    from_path_bits.truncate(params::BALANCE_TREE_DEPTH);

    // This is an injective encoding, as cur is a
    // point in the prime order subgroup.
    let mut cur_from = from_leaf_hash.get_x().clone();

    let audit_path_from = transaction.get()?.1.clone().auth_path_from;
    // Ascend the merkle tree authentication path
    for (i, (e, direction_bit)) in audit_path_from.clone().into_iter().zip(from_path_bits.clone().into_iter()).enumerate() {
        let cs = &mut cs.namespace(|| format!("from merkle tree hash {}", i));

        // "direction_bit" determines if the current subtree
        // is the "right" leaf at this depth of the tree.

        // Witness the authentication path element adjacent
        // at this depth.
        let path_element = num::AllocatedNum::alloc(
            cs.namespace(|| "path element"),
            || {
                Ok(*e.get()?)
            }
        )?;

        // Swap the two if the current subtree is on the right
        let (xl, xr) = num::AllocatedNum::conditionally_reverse(
            cs.namespace(|| "conditional reversal of preimage"),
            &cur_from,
            &path_element,
            &direction_bit
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

    let value_to_allocated = AllocatedNum::alloc(
        cs.namespace(|| "allocate value to"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.balance_to.clone().get()?)
        }
    )?;

    let mut value_content_to = value_to_allocated.into_bits_le(
        cs.namespace(|| "unpack to leaf value")
    )?;

    value_content_to.truncate(params::BALANCE_BIT_WIDTH);
    leaf_content.extend(value_content_to.clone());

    let nonce_to_allocated = AllocatedNum::alloc(
        cs.namespace(|| "allocate nonce to"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.nonce_to.clone().get()?)
        }
    )?;

    let mut nonce_content_to = nonce_to_allocated.into_bits_le(
        cs.namespace(|| "unpack to leaf nonce")
    )?;

    nonce_content_to.truncate(params::NONCE_BIT_WIDTH);
    leaf_content.extend(nonce_content_to.clone());

    // recipient public keys

    let recipient_pk_x = AllocatedNum::alloc(
        cs.namespace(|| "recipient public key x"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.pub_x_to.get()?)
        }
    )?;

    let recipient_pk_y = AllocatedNum::alloc(
        cs.namespace(|| "recipient public key y"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.pub_y_to.get()?)
        }
    )?;

    let mut pub_x_content_to = recipient_pk_x.into_bits_le(
        cs.namespace(|| "to leaf pub_x bits")
    )?;

    pub_x_content_to.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));
    leaf_content.extend(pub_x_content_to.clone());
    
    let mut pub_y_content_to = recipient_pk_y.into_bits_le(
        cs.namespace(|| "to leaf pub_y bits")
    )?;

    pub_y_content_to.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));
    leaf_content.extend(pub_y_content_to.clone());

    assert_eq!(leaf_content.len(), params::BALANCE_BIT_WIDTH 
                                + params::NONCE_BIT_WIDTH
                                + 2 * (params::FR_BIT_WIDTH)
    );
=======
// Intersection point is the only element required in outside scope
    let mut intersection_point_lc = Num::<E>::zero();

    let mut bitmap_path_from = from_path_bits.clone();
    bitmap_path_from.reverse();
    
    let mut bitmap_path_to = to_path_bits.clone();
    bitmap_path_to.reverse();

>>>>>>> more_ff

    let common_prefix = find_common_prefix(
        cs.namespace(|| "common prefix search"), 
        &bitmap_path_from,
        &bitmap_path_to
    )?;

<<<<<<< HEAD
    // Constraint that "to" field in transaction is 
    // equal to the merkle proof path

    let to_address_allocated = AllocatedNum::alloc(
        cs.namespace(|| "recipient address"),
        || {
            let tx = &transaction.get()?.0;
            Ok(*tx.to.get()?)
        }
    )?;

    let mut to_path_bits = to_address_allocated.into_bits_le(
        cs.namespace(|| "recipient address bit decomposition")
    )?;

    to_path_bits.truncate(params::BALANCE_TREE_DEPTH);

    // This is an injective encoding, as cur is a
    // point in the prime order subgroup.
    let mut cur_to = to_leaf_hash.get_x().clone();

    let audit_path_to = transaction.get()?.1.clone().auth_path_to;
    // Ascend the merkle tree authentication path
    for (i, (e, direction_bit)) in audit_path_to.clone().into_iter().zip(to_path_bits.clone().into_iter()).enumerate() {
        let cs = &mut cs.namespace(|| format!("to merkle tree hash {}", i));

        // "direction_bit" determines if the current subtree 
        //is the "right" leaf at this depth of the tree.
        
        // Witness the authentication path element adjacent
        // at this depth.
        let path_element = num::AllocatedNum::alloc(
            cs.namespace(|| "path element"),
            || {
                Ok(*e.get()?)
            }
        )?;

        // Swap the two if the current subtree is on the right
        let (xl, xr) = num::AllocatedNum::conditionally_reverse(
            cs.namespace(|| "conditional reversal of preimage"),
            &cur_to,
            &path_element,
            &direction_bit
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
=======
>>>>>>> more_ff

    let common_prefix_iter = common_prefix.clone().into_iter();
    // Common prefix is found, not we enforce equality of 
    // audit path elements on a common prefix

    for (i, bitmask_bit) in common_prefix_iter.enumerate()
    {
        let path_element_from = &audit_path_from[i];
        let path_element_to = &audit_path_to[i];

        cs.enforce(
            || format!("enforce audit path equality for {}", i),
            |lc| lc + path_element_from.get_variable() - path_element_to.get_variable(),
            |_| bitmask_bit.lc(CS::one(), E::Fr::one()),
            |lc| lc
        );
    }

    // Now we have to find a "point of intersection"
    // Good for us it's just common prefix interpreted as binary number + 1
    // and bit decomposed

    let mut coeff = E::Fr::one();
    for bit in common_prefix.into_iter() {
        intersection_point_lc = intersection_point_lc.add_bool_with_coeff(
            CS::one(), 
            &bit, 
            coeff
        );
        coeff.double();
    }

    // and add one
    intersection_point_lc = intersection_point_lc.add_bool_with_coeff(
        CS::one(), 
        &boolean::Boolean::Constant(true), 
        E::Fr::one()
    );

    // Intersection point is a number with a single bit that indicates how far
    // from the root intersection is

    let intersection_point = AllocatedNum::alloc(
        cs.namespace(|| "intersection as number"),
        || Ok(*intersection_point_lc.get_value().get()?)
    )?;

    cs.enforce(
        || "pack intersection",
        |lc| lc + intersection_point.get_variable(),
        |lc| lc + CS::one(),
        |_| intersection_point_lc.lc(E::Fr::one())
    );

    // Intersection point into bits to use for root recalculation
    let mut intersection_point_bits = intersection_point.into_bits_le(
        cs.namespace(|| "unpack intersection")
    )?;

    // truncating guarantees that even if the common prefix coincides everywhere
    // up to the last bit, it can still be properly used in next actions
    intersection_point_bits.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
    // reverse cause bits here are counted from root, and later we need from the leaf
    intersection_point_bits.reverse();
    
    Ok(intersection_point_bits)
}

fn check_message_signature<E, CS>(
    mut cs: CS,
    from_path_bits: Vec<boolean::Boolean>,
    to_path_bits: Vec<boolean::Boolean>,
    leaf: &LeafContent<E>,
    transaction: &Transaction<E>,
    params: &E::Params,
    generator: ecc::EdwardsPoint<E>
) -> Result<TransactionContent<E>, SynthesisError>
    where E: JubjubEngine,
          CS: ConstraintSystem<E>
{
    let mut message_bits: Vec<boolean::Boolean> = vec![];

    // add sender and recipient addresses to check
    message_bits.extend(from_path_bits.clone());
    message_bits.extend(to_path_bits.clone());

    let amount_encoded = AllocatedNum::alloc(
        cs.namespace(|| "allocate encoded transaction amount"),
        || {
            Ok(*transaction.amount.get()?)
        }
    )?;

    let mut amount_bits = amount_encoded.into_bits_le(
        cs.namespace(|| "amount bits")
    )?;

    amount_bits.truncate(params::AMOUNT_EXPONENT_BIT_WIDTH + params::AMOUNT_MANTISSA_BIT_WIDTH);
    
    // add amount to check
    message_bits.extend(amount_bits.clone());

    let fee_encoded = AllocatedNum::alloc(
        cs.namespace(|| "allocate encoded transaction fee"),
        || {
            Ok(*transaction.fee.get()?)
        }
    )?;

    let mut fee_bits = fee_encoded.into_bits_le(
        cs.namespace(|| "fee bits")
    )?;

    fee_bits.truncate(params::FEE_EXPONENT_BIT_WIDTH + params::FEE_MANTISSA_BIT_WIDTH);

    // add fee to check
    message_bits.extend(fee_bits.clone());

    // add nonce to check
    message_bits.extend(leaf.nonce_bits.clone());

    let transaction_max_block_number_allocated = AllocatedNum::alloc(
        cs.namespace(|| "allocate transaction good until block"),
        || {
            Ok(*transaction.good_until_block.get()?)
        }
    )?;

    let mut block_number_bits = transaction_max_block_number_allocated.into_bits_le(
        cs.namespace(|| "block number bits")
    )?;

    block_number_bits.truncate(params::BLOCK_NUMBER_BIT_WIDTH);

    // add block number to check
    message_bits.extend(block_number_bits.clone());

    let sender_pk = ecc::EdwardsPoint::interpret(
        cs.namespace(|| "sender public key"),
        &leaf.pub_x,
        &leaf.pub_y,
        params
    )?;

    let signature_r_x = AllocatedNum::alloc(
        cs.namespace(|| "signature r_x witness"),
        || {
            Ok(transaction.signature.get()?.r.into_xy().0)
        }
    )?;

    let signature_r_y = AllocatedNum::alloc(
        cs.namespace(|| "signature r_y witness"),
        || {
            Ok(transaction.signature.get()?.r.into_xy().1)
        }
    )?;

    let signature_r = ecc::EdwardsPoint::interpret(
        cs.namespace(|| "signature r as point"),
        &signature_r_x,
        &signature_r_y,
        params
    )?;

    let signature_s = AllocatedNum::alloc(
        cs.namespace(|| "signature s witness"),
        || {
            Ok(transaction.signature.get()?.s)
        }
    )?;

    let signature = EddsaSignature {
        r: signature_r,
        s: signature_s,
        pk: sender_pk
    };

    let max_message_len = params::BALANCE_TREE_DEPTH 
                        + params::BALANCE_TREE_DEPTH 
                        + params::AMOUNT_EXPONENT_BIT_WIDTH 
                        + params::AMOUNT_MANTISSA_BIT_WIDTH
                        + params::FEE_EXPONENT_BIT_WIDTH
                        + params::FEE_MANTISSA_BIT_WIDTH
                        + params::NONCE_BIT_WIDTH
                        + params::BLOCK_NUMBER_BIT_WIDTH;

    signature.verify_raw_message_signature(
        cs.namespace(|| "verify transaction signature"),
        params, 
        &message_bits,
        generator,
        max_message_len
    )?;

    Ok(TransactionContent {
        amount_bits: amount_bits,
        fee_bits: fee_bits,
        good_until_block: transaction_max_block_number_allocated
    })
}

/// Applies one transaction to the tree,
/// outputs a new root
fn apply_transaction<E, CS>(
    mut cs: CS,
    old_root: AllocatedNum<E>,
    transaction: Transaction<E>,
    witness: TransactionWitness<E>,
    params: &E::Params,
    generator: ecc::EdwardsPoint<E>
) -> Result<(AllocatedNum<E>, AllocatedNum<E>, AllocatedNum<E>, Vec<boolean::Boolean>), SynthesisError>
    where E: JubjubEngine,
          CS: ConstraintSystem<E>
{
    // Calculate leaf value commitment

    let leaf_from = make_leaf_content(
        cs.namespace(|| "create sender's leaf"),
        witness.clone().leaf_from
    )?;

    // Compute the hash of the from leaf
    let mut from_leaf_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "sender's leaf content hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &leaf_from.leaf_bits,
        params
    )?;

    // Constraint that "from" field in transaction is 
    // equal to the merkle proof path

    let from_address_allocated = AllocatedNum::alloc(
        cs.namespace(|| "sender address"),
        || {
            Ok(*transaction.from.get()?)
        }
    )?;

    let mut from_path_bits = from_address_allocated.into_bits_le(
        cs.namespace(|| "sender address bit decomposition")
    )?;

    from_path_bits.truncate(*plasma_constants::BALANCE_TREE_DEPTH);

    let audit_path_from = allocate_audit_path(
        cs.namespace(|| "allocate audit path for sender"), 
        witness.clone().auth_path_from
    )?;

    {
        // This is an injective encoding, as cur is a
        // point in the prime order subgroup.
        let mut cur_from = from_leaf_hash.get_x().clone();

        // Ascend the merkle tree authentication path
        for (i, direction_bit) in from_path_bits.clone().into_iter()
                                            .enumerate() {
            let cs = &mut cs.namespace(|| format!("from merkle tree hash {}", i));

            // "direction_bit" determines if the current subtree
            // is the "right" leaf at this depth of the tree.

            // Witness the authentication path element adjacent
            // at this depth.
            let path_element = &audit_path_from[i];

            // Swap the two if the current subtree is on the right
            let (xl, xr) = num::AllocatedNum::conditionally_reverse(
                cs.namespace(|| "conditional reversal of preimage"),
                &cur_from,
                path_element,
                &direction_bit
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
    }

    // Do the same for "to" leaf

    let leaf_to = make_leaf_content(
        cs.namespace(|| "create recipients's leaf"),
        witness.clone().leaf_to
    )?;

    // Compute the hash of the from leaf
    let mut to_leaf_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "to leaf content hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &leaf_to.leaf_bits,
        params
    )?;

    // Constraint that "to" field in transaction is 
    // equal to the merkle proof path

    let to_address_allocated = AllocatedNum::alloc(
        cs.namespace(|| "recipient address"),
        || {
            Ok(*transaction.to.get()?)
        }
    )?;

    let mut to_path_bits = to_address_allocated.into_bits_le(
        cs.namespace(|| "recipient address bit decomposition")
    )?;

    to_path_bits.truncate(*plasma_constants::BALANCE_TREE_DEPTH);

    let audit_path_to = allocate_audit_path(
        cs.namespace(|| "allocate audit path for recipient"), 
        witness.clone().auth_path_to
    )?;

    {
        // This is an injective encoding, as cur is a
        // point in the prime order subgroup.
        let mut cur_to = to_leaf_hash.get_x().clone();

        // Ascend the merkle tree authentication path
        for (i, direction_bit) in to_path_bits.clone().into_iter()
                                            .enumerate() {
            let cs = &mut cs.namespace(|| format!("to merkle tree hash {}", i));

            // "direction_bit" determines if the current subtree 
            //is the "right" leaf at this depth of the tree.
            
            // Witness the authentication path element adjacent
            // at this depth.
            let path_element = &audit_path_to[i];

            // Swap the two if the current subtree is on the right
            let (xl, xr) = num::AllocatedNum::conditionally_reverse(
                cs.namespace(|| "conditional reversal of preimage"),
                &cur_to,
                path_element,
                &direction_bit
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
    }

    // Initial leaf values are allocated, not we can find a common prefix

    // before having fun with leafs calculate the common prefix
    // of two audit paths

    // Ok, old leaf values are exposed, so we can check 
    // the signature and parse the rest of transaction data

    let transaction_content = check_message_signature(
        cs.namespace(|| "parse and check transaction"),
        from_path_bits.clone(),
        to_path_bits.clone(),
        &leaf_from,
        &transaction,
        params,
        generator
    )?;

    let amount = parse_with_exponent_le(
        cs.namespace(|| "parse amount"),
<<<<<<< HEAD
        &amount_bits,
        params::AMOUNT_EXPONENT_BIT_WIDTH,
        params::AMOUNT_MANTISSA_BIT_WIDTH,
=======
        &transaction_content.amount_bits,
        *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH,
>>>>>>> more_ff
        10
    )?;

    let fee = parse_with_exponent_le(
        cs.namespace(|| "parse fee"),
<<<<<<< HEAD
        &fee_bits,
        params::FEE_EXPONENT_BIT_WIDTH,
        params::FEE_MANTISSA_BIT_WIDTH,
=======
        &transaction_content.fee_bits,
        *plasma_constants::FEE_EXPONENT_BIT_WIDTH,
        *plasma_constants::FEE_MANTISSA_BIT_WIDTH,
>>>>>>> more_ff
        10
    )?;

    // repack balances as we have truncated bit decompositions already
    let mut old_balance_from_lc = Num::<E>::zero();
    let mut coeff = E::Fr::one();
    for bit in &leaf_from.value_bits {
        old_balance_from_lc = old_balance_from_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let mut old_balance_to_lc = Num::<E>::zero();
    coeff = E::Fr::one();
    for bit in &leaf_to.value_bits {
        old_balance_to_lc = old_balance_to_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let mut nonce_lc = Num::<E>::zero();
    coeff = E::Fr::one();
    for bit in &leaf_from.nonce_bits {
        nonce_lc = nonce_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let old_balance_from = AllocatedNum::alloc(
        cs.namespace(|| "allocate old balance from"),
        || Ok(*old_balance_from_lc.get_value().get()?)
    )?;

    cs.enforce(
        || "pack old balance from",
        |lc| lc + old_balance_from.get_variable(),
        |lc| lc + CS::one(),
        |_| old_balance_from_lc.lc(E::Fr::one())
    );

    let old_balance_to = AllocatedNum::alloc(
        cs.namespace(|| "allocate old balance to"),
        || Ok(*old_balance_to_lc.get_value().get()?)
    )?;

    cs.enforce(
        || "pack old balance to",
        |lc| lc + old_balance_to.get_variable(),
        |lc| lc + CS::one(),
        |_| old_balance_to_lc.lc(E::Fr::one())
    );

    let nonce = AllocatedNum::alloc(
        cs.namespace(|| "nonce"),
        || Ok(*nonce_lc.get_value().get()?)
    )?;

    cs.enforce(
        || "pack nonce",
        |lc| lc + nonce.get_variable(),
        |lc| lc + CS::one(),
        |_| nonce_lc.lc(E::Fr::one())
    );

    let new_balance_from = AllocatedNum::alloc(
        cs.namespace(|| "new balance from"),
        || {
            let old_balance_from_value = old_balance_from.get_value().get()?.clone();
            let transfer_amount_value = amount.clone().get_value().get()?.clone();
            let fee_value = fee.clone().get_value().get()?.clone();
            let mut new_balance_from_value = old_balance_from_value;
            new_balance_from_value.sub_assign(&transfer_amount_value);
            new_balance_from_value.sub_assign(&fee_value);

            Ok(new_balance_from_value)
        }
    )?;

    // constraint no overflow
    new_balance_from.limit_number_of_bits(
        cs.namespace(|| "limit number of bits for new balance from"),
        params::BALANCE_BIT_WIDTH
    )?;

    // enforce reduction of balance
    cs.enforce(
        || "enforce sender's balance reduced",
        |lc| lc + old_balance_from.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + new_balance_from.get_variable() + fee.get_variable() + amount.get_variable()
    );

    // let number_of_bits_in_recipient = count_number_of_ones(
    //     cs.namespace(|| "number of non-zero bits in recipient address"),
    //     &from_path_bits
    // )?;

    let recipient_is_zero = AllocatedNum::alloc(
        cs.namespace(|| "recipient is zero"),
        || {
            let to = *to_address_allocated.clone().get_value().get()?;
            if to == E::Fr::zero() {
                return Ok(E::Fr::one());
            }
            Ok(E::Fr::zero())
        }
    )?;

    // enforce that recipient_is_zero is actually a boolean
    // a * (1-a) == 0
    cs.enforce(
        || "enforce recipient_is_zero is either zero or one",
        |lc| lc + recipient_is_zero.get_variable(),
        |lc| lc + CS::one() - recipient_is_zero.get_variable(),
        |lc| lc
    );


    // we have to enforce that recipient_is_zero = 1 
    // if and only if to == 0
    // b * a = 0
    // either b is zero or a is zero
    // in our case b = recipient_is_zero = 1 or 0
    // a can be anything
    cs.enforce(
        || "enforce recipient_is_zero is one only if recipient is zero",
        |lc| lc + recipient_is_zero.get_variable(),
        |lc| lc + to_address_allocated.get_variable(),
        |lc| lc
    );

    // Ok, now a tricky part for an account zero having a special meaning
    // If to == 0 then balance of to is not increased

    let new_balance_to = AllocatedNum::alloc(
        cs.namespace(|| "new balance to"),
        || {
            let to = *to_address_allocated.clone().get_value().get()?;
            if to == E::Fr::zero() {
                return Ok(E::Fr::zero());
            }

            let transfer_amount_value = amount.clone().get_value().get()?.clone();
            let old_balance_to_value = old_balance_to.clone().get_value().get()?.clone();

            let mut new_balance_to_value = old_balance_to_value;
            new_balance_to_value.add_assign(&transfer_amount_value);

            Ok(new_balance_to_value)
        }
    )?;

    // constraint no overflow
    new_balance_to.limit_number_of_bits(
        cs.namespace(|| "limit number of bits for new balance to"),
        params::BALANCE_BIT_WIDTH
    )?;

    // enforce increase of balance with a special case of to == 0
    // that's trivial with a previous constraints
    // (a + b) * (1 - is_zero) = c
    // if is_zero == 1 -> c == 0
    // if is_zero == 0 -> a + b = c
    cs.enforce(
        || "enforce recipients's balance increased",
        |lc| lc + old_balance_to.get_variable() + amount.get_variable(),
        |lc| lc + CS::one() - recipient_is_zero.get_variable(),
        |lc| lc + new_balance_to.get_variable()
    );

    let new_nonce = AllocatedNum::alloc(
        cs.namespace(|| "new nonce"),
        || {
            let mut new_nonce_value = nonce.get_value().get()?.clone();
            new_nonce_value.add_assign(&E::Fr::one());

            Ok(new_nonce_value)
        }
    )?;

    // constraint no overflow
    new_nonce.limit_number_of_bits(
        cs.namespace(|| "limit number of bits for new nonce from"),
        params::NONCE_BIT_WIDTH
    )?;

    // enforce increase of balance
    cs.enforce(
        || "enforce sender's nonce to increase",
        |lc| lc + new_nonce.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + nonce.get_variable() + CS::one()
    );

    // Now we should assemble a new root. It's more tricky as it requires
    // to calculate an intersection point and for a part of the tree that is
    // "below" intersection point use individual merkle brancher,
    // for the intersection - use the other current value,
    // for the rest - use any of the braches, as it's constrained that 
    // those coincide

    // this operation touches a lot of previous values, so it's done in this function
    // with some scoping

    // first of new "from" leaf
    {

        let mut leaf_content = vec![];

        // change balance and nonce

        let mut value_content = new_balance_from.into_bits_le(
            cs.namespace(|| "from leaf updated amount bits")
        )?;

<<<<<<< HEAD

        value_content.truncate(params::BALANCE_BIT_WIDTH);
=======
        value_content.truncate(*plasma_constants::BALANCE_BIT_WIDTH);
>>>>>>> more_ff
        leaf_content.extend(value_content.clone());

        let mut nonce_content = new_nonce.into_bits_le(
            cs.namespace(|| "from leaf updated nonce bits")
        )?;

        nonce_content.truncate(params::NONCE_BIT_WIDTH);
        leaf_content.extend(nonce_content);

        // keep public keys
        append_packed_public_key(& mut leaf_content, leaf_from.pub_x_bit, leaf_from.pub_y_bits);

<<<<<<< HEAD
        assert_eq!(leaf_content.len(), params::BALANCE_BIT_WIDTH 
                                    + params::NONCE_BIT_WIDTH
                                    + 2 * (params::FR_BIT_WIDTH)
=======
        assert_eq!(leaf_content.len(), *plasma_constants::BALANCE_BIT_WIDTH 
                                    + *plasma_constants::NONCE_BIT_WIDTH
                                    + *plasma_constants::FR_BIT_WIDTH
>>>>>>> more_ff
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

        let mut leaf_content = vec![];

        // change balance only
        let mut value_content = new_balance_to.into_bits_le(
            cs.namespace(|| "to leaf updated amount bits")
        )?;

<<<<<<< HEAD
        value_content.truncate(params::BALANCE_BIT_WIDTH);
        leaf_content.extend(value_content.clone());
=======
        value_content.truncate(*plasma_constants::BALANCE_BIT_WIDTH);
        leaf_content.extend(value_content);
>>>>>>> more_ff

        // everything else remains the same
        leaf_content.extend(leaf_to.nonce_bits);
        append_packed_public_key(& mut leaf_content, leaf_to.pub_x_bit, leaf_to.pub_y_bits);

<<<<<<< HEAD
        assert_eq!(leaf_content.len(), params::BALANCE_BIT_WIDTH 
                                    + params::NONCE_BIT_WIDTH
                                    + 2 * (params::FR_BIT_WIDTH)
=======
        assert_eq!(leaf_content.len(), *plasma_constants::BALANCE_BIT_WIDTH 
                                    + *plasma_constants::NONCE_BIT_WIDTH
                                    + *plasma_constants::FR_BIT_WIDTH
>>>>>>> more_ff
        );


        // Compute the hash of the from leaf
        to_leaf_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "to leaf content hash updated"),
            pedersen_hash::Personalization::NoteCommitment,
            &leaf_content,
            params
        )?;

    }

<<<<<<< HEAD
    // Intersection point into bits to use for root recalculation
    let mut intersection_point_bits = intersection_point.into_bits_le(
        cs.namespace(|| "unpack intersection")
    )?;

    // truncating guarantees that even if the common prefix coincides everywhere
    // up to the last bit, it can still be properly used in next actions
    intersection_point_bits.truncate(params::BALANCE_TREE_DEPTH);
    // reverse cause bits here are counted from root, and later we need from the leaf
    intersection_point_bits.reverse();

=======
>>>>>>> more_ff
    // First assemble new leafs
    let mut cur_from = from_leaf_hash.get_x().clone();
    let mut cur_to = to_leaf_hash.get_x().clone();

    let intersection_point_bits = find_intersection_point(
        cs.namespace(|| "find intersection point for merkle paths"),
        from_path_bits.clone(), 
        to_path_bits.clone(), 
        &audit_path_from, 
        &audit_path_to
    )?;

    {
        // Ascend the merkle tree authentication path
        for (i, ((direction_bit_from, direction_bit_to), intersection_bit)) in from_path_bits.clone().into_iter()
                                                                                        .zip(to_path_bits.clone().into_iter())
                                                                                        .zip(intersection_point_bits.into_iter()).enumerate() 
            {

            let cs = &mut cs.namespace(|| format!("assemble new state root{}", i));

            let original_path_element_from = &audit_path_from[i];

            let original_path_element_to = &audit_path_to[i];

            // Now the most fancy part is to determine when to use path element form witness,
            // or recalculated element from another subtree

            // If we are on intersection place take a current hash from another branch instead of path element
            let path_element_from = num::AllocatedNum::conditionally_select(
                cs.namespace(|| "conditional select of preimage from"),
                &cur_to,
                original_path_element_from, 
                &intersection_bit
            )?;

            // Swap the two if the current subtree is on the right
            let (xl_from, xr_from) = num::AllocatedNum::conditionally_reverse(
                cs.namespace(|| "conditional reversal of preimage from"),
                &cur_from,
                &path_element_from,
                &direction_bit_from
            )?;

            let mut preimage_from = vec![];
            preimage_from.extend(xl_from.into_bits_le(cs.namespace(|| "xl_from into bits"))?);
            preimage_from.extend(xr_from.into_bits_le(cs.namespace(|| "xr_from into bits"))?);

            // same for to

            // If we are on intersection place take a current hash from another branch instead of path element
            let path_element_to = num::AllocatedNum::conditionally_select(
                cs.namespace(|| "conditional select of preimage to"),
                &cur_from,
                original_path_element_to, 
                &intersection_bit
            )?;

            // Swap the two if the current subtree is on the right
            let (xl_to, xr_to) = num::AllocatedNum::conditionally_reverse(
                cs.namespace(|| "conditional reversal of preimage to"),
                &cur_to,
                &path_element_to,
                &direction_bit_to
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

    // the last step - we expose public data for later commitment

    // convert to BE for further use in Ethereum
    let mut from_path_be = from_path_bits.clone();
    from_path_be.reverse();

    let mut to_path_be = to_path_bits.clone();
    to_path_be.reverse();

    let mut public_data = vec![];
    public_data.extend(from_path_be);
    public_data.extend(to_path_be);
    public_data.extend(transaction_content.amount_bits.clone());
    public_data.extend(transaction_content.fee_bits.clone());

    assert_eq!(public_data.len(), params::BALANCE_TREE_DEPTH 
                                    + params::BALANCE_TREE_DEPTH
                                    + params::AMOUNT_EXPONENT_BIT_WIDTH
                                    + params::AMOUNT_MANTISSA_BIT_WIDTH
                                    + params::FEE_EXPONENT_BIT_WIDTH
                                    + params::FEE_MANTISSA_BIT_WIDTH);

    Ok((cur_from, fee, transaction_content.good_until_block, public_data))
}

#[test]
fn test_bits_into_fr(){
    use ff::{PrimeField};
    use pairing::bn256::*;
    use std::str::FromStr;

    // representation of 4 + 8 + 256 = 12 + 256 = 268 = 0x010c;
    let bits: Vec<bool> = [false, false, true, true, false, false, false, false, true].to_vec();

    let fe: Fr = le_bit_vector_into_field_element::<Fr>(&bits);

    print!("{}\n", fe);
}

fn print_boolean_vector(vector: &[boolean::Boolean]) {
    for b in vector {
        if b.get_value().unwrap() {
            print!("1");
        } else {
            print!("0");
        }
    }
    print!("\n");
}

#[test]
fn test_transfer_circuit_with_witness() {
    use ff::{Field};
    use pairing::bn256::*;
    use rand::{SeedableRng, Rng, XorShiftRng, Rand};
    use sapling_crypto::circuit::test::*;
    use sapling_crypto::alt_babyjubjub::{AltJubjubBn256, fs, edwards, PrimeOrder};
    use crate::models::circuit::{AccountTree, Account};
    use crypto::sha2::Sha256;
    use crypto::digest::Digest;
    use crate::circuit::utils::be_bit_vector_into_bytes;

    extern crate hex;

    let p_g = FixedGenerators::SpendingKeyGenerator;
    let params = &AltJubjubBn256::new();

    let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1 {
        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let mut tree = AccountTree::new(tree_depth);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << params::BALANCE_TREE_DEPTH);

        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
    
        let recipient_sk = PrivateKey::<Bn256>(rng.gen());
        let recipient_pk = PublicKey::from_private(&recipient_sk, p_g, params);
        let (recipient_x, recipient_y) = recipient_pk.0.into_xy();

        // give some funds to sender and make zero balance for recipient

        // let sender_leaf_number = 1;
        // let recipient_leaf_number = 2;

        let mut sender_leaf_number : u32 = rng.gen();
        sender_leaf_number = sender_leaf_number % capacity;
        let mut recipient_leaf_number : u32 = rng.gen();
        recipient_leaf_number = recipient_leaf_number % capacity;

        let transfer_amount : u128 = 500;

        let transfer_amount_as_field_element = Fr::from_str(&transfer_amount.to_string()).unwrap();

        let transfer_amount_bits = convert_to_float(
            transfer_amount,
            params::AMOUNT_EXPONENT_BIT_WIDTH,
            params::AMOUNT_MANTISSA_BIT_WIDTH,
            10
        ).unwrap();

        let transfer_amount_encoded: Fr = le_bit_vector_into_field_element(&transfer_amount_bits);

        let fee : u128 = 0;

        let fee_as_field_element = Fr::from_str(&fee.to_string()).unwrap();

        let fee_bits = convert_to_float(
            fee,
            params::FEE_EXPONENT_BIT_WIDTH,
            params::FEE_MANTISSA_BIT_WIDTH,
            10
        ).unwrap();

        let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

        let sender_leaf = Account {
                balance:    Fr::from_str("1000").unwrap(),
                nonce:      Fr::zero(),
                pub_x:      sender_x,
                pub_y:      sender_y,
        };

        let recipient_leaf = Account {
                balance:    Fr::zero(),
                nonce:      Fr::one(),
                pub_x:      recipient_x,
                pub_y:      recipient_y,
        };

        let initial_root = tree.root_hash();
        print!("Empty root = {}\n", initial_root);

        tree.insert(sender_leaf_number, sender_leaf.clone());
        tree.insert(recipient_leaf_number, recipient_leaf.clone());

        let old_root = tree.root_hash();
        print!("Old root = {}\n", old_root);

        print!("Sender leaf hash is {}\n", tree.get_hash((tree_depth, sender_leaf_number)));
        print!("Recipient leaf hash is {}\n", tree.get_hash((tree_depth, recipient_leaf_number)));

        // check empty leafs 

        // print!("Empty leaf hash is {}\n", tree.get_hash((tree_depth, 0)));

        // print!("Verifying merkle proof for an old leaf\n");
        //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));
        // print!("Done verifying merkle proof for an old leaf, result {}\n", inc);

        let path_from : Vec<Option<Fr>> = tree.merkle_path(sender_leaf_number).into_iter().map(|e| Some(e.0)).collect();
        let path_to: Vec<Option<Fr>>  = tree.merkle_path(recipient_leaf_number).into_iter().map(|e| Some(e.0)).collect();

        let from = Fr::from_str(& sender_leaf_number.to_string());
        let to = Fr::from_str(& recipient_leaf_number.to_string());

        let mut transaction : Transaction<Bn256> = Transaction {
            from: from,
            to: to,
            amount: Some(transfer_amount_encoded),
            fee: Some(fee_encoded),
            nonce: Some(Fr::zero()),
            good_until_block: Some(Fr::one()),
            signature: None
        };

        transaction.sign(
            &sender_sk,
            p_g,
            params,
            rng
        );

        assert!(transaction.signature.is_some());

        let mut updated_sender_leaf = sender_leaf.clone();
        let mut updated_recipient_leaf = recipient_leaf.clone();

        let leaf_witness_from = LeafWitness {
            balance: Some(sender_leaf.balance),
            nonce: Some(sender_leaf.nonce),
            pub_x: Some(sender_leaf.pub_x),
            pub_y: Some(sender_leaf.pub_y),
        };

        let leaf_witness_to = LeafWitness {
            balance: Some(recipient_leaf.balance),
            nonce: Some(recipient_leaf.nonce),
            pub_x: Some(recipient_leaf.pub_x),
            pub_y: Some(recipient_leaf.pub_y),
        };

        let transaction_witness = TransactionWitness {
            leaf_from: leaf_witness_from,
            auth_path_from: path_from,
            leaf_to: leaf_witness_to,
            auth_path_to: path_to,
        };

        updated_sender_leaf.balance.sub_assign(&transfer_amount_as_field_element);
        updated_sender_leaf.nonce.add_assign(&Fr::one());

        print!("Updated sender: \n");
        print!("Amount: {}\n", updated_sender_leaf.clone().balance);
        print!("Nonce: {}\n", updated_sender_leaf.clone().nonce);

        updated_recipient_leaf.balance.add_assign(&transfer_amount_as_field_element);
        print!("Updated recipient: \n");
        print!("Amount: {}\n", updated_recipient_leaf.clone().balance);
        print!("Nonce: {}\n", updated_recipient_leaf.clone().nonce);

        tree.insert(sender_leaf_number, updated_sender_leaf.clone());
        tree.insert(recipient_leaf_number, updated_recipient_leaf.clone());

<<<<<<< HEAD
        //assert!(tree.verify_proof(sender_leaf_number, updated_sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));
        //assert!(tree.verify_proof(recipient_leaf_number, updated_recipient_leaf.clone(), tree.merkle_path(recipient_leaf_number)));
=======
        print!("Final sender leaf hash is {}\n", tree.get_hash((tree_depth, sender_leaf_number)));
        print!("Final recipient leaf hash is {}\n", tree.get_hash((tree_depth, recipient_leaf_number)));

        assert!(tree.verify_proof(sender_leaf_number, updated_sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));
        assert!(tree.verify_proof(recipient_leaf_number, updated_recipient_leaf.clone(), tree.merkle_path(recipient_leaf_number)));
>>>>>>> more_ff

        let new_root = tree.root_hash();

        print!("New root = {}\n", new_root);

        assert!(old_root != new_root);

        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let mut public_data_initial_bits = vec![];

            // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

            let block_number_bits: Vec<bool> = BitIterator::new(Fr::one().into_repr()).collect();
            for _ in 0..256-block_number_bits.len() {
                public_data_initial_bits.push(false);
            }
            public_data_initial_bits.extend(block_number_bits.into_iter());

            let total_fee_bits: Vec<bool> = BitIterator::new(Fr::zero().into_repr()).collect();
            for _ in 0..256-total_fee_bits.len() {
                public_data_initial_bits.push(false);
            }
            public_data_initial_bits.extend(total_fee_bits.into_iter());

            assert_eq!(public_data_initial_bits.len(), 512);

            let mut h = Sha256::new();

            let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

            h.input(&bytes_to_hash);

            let mut hash_result = [0u8; 32];
            h.result(&mut hash_result[..]);


            print!("Initial hash hex {}\n", hex::encode(hash_result.clone()));

            let mut packed_transaction_data = vec![];
            let transaction_data = transaction.public_data_into_bits();
            packed_transaction_data.extend(transaction_data.clone().into_iter());

            let packed_transaction_data_bytes = be_bit_vector_into_bytes(&packed_transaction_data);

            print!("Packed transaction data hex {}\n", hex::encode(packed_transaction_data_bytes.clone()));

            let mut next_round_hash_bytes = vec![];
            next_round_hash_bytes.extend(hash_result.iter());
            next_round_hash_bytes.extend(packed_transaction_data_bytes);
            // assert_eq!(next_round_hash_bytes.len(), 64);

            h = Sha256::new();
            h.input(&next_round_hash_bytes);
            hash_result = [0u8; 32];
            h.result(&mut hash_result[..]);

            print!("Final hash as hex {}\n", hex::encode(hash_result.clone()));

            hash_result[0] &= 0x1f; // temporary solution

            let mut repr = Fr::zero().into_repr();
            repr.read_be(&hash_result[..]).expect("pack hash as field element");

            let public_data_commitment = Fr::from_repr(repr).unwrap();

            print!("Final data commitment as field element = {}\n", public_data_commitment);

            let instance = Transfer {
                params: params,
                number_of_transactions: 1,
                old_root: Some(old_root),
                new_root: Some(new_root),
                public_data_commitment: Some(public_data_commitment),
                block_number: Some(Fr::one()),
                total_fee: Some(Fr::zero()),
                transactions: vec![(transaction, transaction_witness)],
            };

            instance.synthesize(&mut cs).unwrap();

            print!("{}\n", cs.find_unconstrained());

            print!("{}\n", cs.num_constraints());

            assert_eq!(cs.num_inputs(), 4);

            let err = cs.which_is_unsatisfied();
            if err.is_some() {
                panic!("ERROR satisfying in {}\n", err.unwrap());
            }
        }
    }
}