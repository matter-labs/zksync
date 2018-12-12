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

use sapling_crypto::eddsa::{
    Signature,
    PrivateKey,
    PublicKey
};

use crate::models::{params, tx::TransactionSignature};
use super::baby_eddsa::EddsaSignature;
use super::utils::*;


// This is transaction data

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


impl <E: JubjubEngine> Transaction<E> {
    pub fn public_data_into_bits(
        &self
    ) -> Vec<bool> {
        // fields are
        // - from
        // - to
        // - amount
        // - fee
        let mut from: Vec<bool> = BitIterator::new(self.from.clone().unwrap().into_repr()).collect();
        from.reverse();
        from.truncate(params::BALANCE_TREE_DEPTH);
        let mut to: Vec<bool> = BitIterator::new(self.to.clone().unwrap().into_repr()).collect();
        to.reverse();
        to.truncate(params::BALANCE_TREE_DEPTH);
        let mut amount: Vec<bool> = BitIterator::new(self.amount.clone().unwrap().into_repr()).collect();
        amount.reverse();
        amount.truncate(params::AMOUNT_EXPONENT_BIT_WIDTH + params::AMOUNT_MANTISSA_BIT_WIDTH);
        let mut fee: Vec<bool> = BitIterator::new(self.fee.clone().unwrap().into_repr()).collect();
        fee.reverse();
        fee.truncate(params::FEE_EXPONENT_BIT_WIDTH + params::FEE_MANTISSA_BIT_WIDTH);
        
        let mut packed: Vec<bool> = vec![];
        packed.extend(from.into_iter());
        packed.extend(to.into_iter());
        packed.extend(amount.into_iter());
        packed.extend(fee.into_iter());

        packed
    }

    pub fn data_for_signature_into_bits(
        &self
    ) -> Vec<bool> {
        // fields are
        // - from
        // - to
        // - amount
        // - fee
        // - nonce
        // - good_until_block
        let mut nonce: Vec<bool> = BitIterator::new(self.nonce.clone().unwrap().into_repr()).collect();
        nonce.reverse();
        nonce.truncate(params::NONCE_BIT_WIDTH);
        let mut good_until_block: Vec<bool> = BitIterator::new(self.good_until_block.clone().unwrap().into_repr()).collect();
        good_until_block.reverse();
        good_until_block.truncate(params::BLOCK_NUMBER_BIT_WIDTH);
        let mut packed: Vec<bool> = vec![];
        
        packed.extend(self.public_data_into_bits().into_iter());
        packed.extend(nonce.into_iter());
        packed.extend(good_until_block.into_iter());

        packed
    }

    pub fn data_as_bytes(
        & self
    ) -> Vec<u8> {
        let raw_data: Vec<bool> = self.data_for_signature_into_bits();

        let mut message_bytes: Vec<u8> = vec![];

        let byte_chunks = raw_data.chunks(8);
        for byte_chunk in byte_chunks
        {
            let mut byte = 0u8;
            for (i, bit) in byte_chunk.into_iter().enumerate()
            {
                if *bit {
                    byte |= 1 << i;
                }
            }
            message_bytes.push(byte);
        }

        message_bytes
    }

    pub fn sign<R>(
        & mut self,
        private_key: &PrivateKey<E>,
        p_g: FixedGenerators,
        params: &E::Params,
        rng: & mut R
    ) where R: rand::Rng {

        let message_bytes = self.data_as_bytes();

        let max_message_len = params::BALANCE_TREE_DEPTH 
                        + params::BALANCE_TREE_DEPTH 
                        + params::AMOUNT_EXPONENT_BIT_WIDTH 
                        + params::AMOUNT_MANTISSA_BIT_WIDTH
                        + params::FEE_EXPONENT_BIT_WIDTH
                        + params::FEE_MANTISSA_BIT_WIDTH
                        + params::NONCE_BIT_WIDTH
                        + params::BLOCK_NUMBER_BIT_WIDTH;
        
        let signature = private_key.sign_raw_message(
            &message_bytes, 
            rng, 
            p_g, 
            params,
            max_message_len / 8
        );

        let pk = PublicKey::from_private(&private_key, p_g, params);
        let is_valid_signature = pk.verify_for_raw_message(&message_bytes, 
                                        &signature.clone(), 
                                        p_g, 
                                        params, 
                                        max_message_len/8);
        if !is_valid_signature {
            return;
        }

        let mut sigs_le_bits: Vec<bool> = BitIterator::new(signature.s.into_repr()).collect();
        sigs_le_bits.reverse();

        let sigs_converted = le_bit_vector_into_field_element(&sigs_le_bits);

        // let mut sigs_bytes = [0u8; 32];
        // signature.s.into_repr().write_le(& mut sigs_bytes[..]).expect("get LE bytes of signature S");
        // let mut sigs_repr = E::Fr::zero().into_repr();
        // sigs_repr.read_le(&sigs_bytes[..]).expect("interpret S as field element representation");
        // let sigs_converted = E::Fr::from_repr(sigs_repr).unwrap();

        let converted_signature = TransactionSignature {
            r: signature.r,
            s: sigs_converted
        };

        self.signature = Some(converted_signature);

    }
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
        // let mut public_data_vector: Vec<Vec<boolean::Boolean>> = vec![];

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
            let (intermediate_root, fee, block_number, public_data) = apply_transaction(
                cs.namespace(|| format!("applying transaction {}", i)),
                old_root,
                tx, 
                self.params,
                generator.clone()
            )?;
            old_root = intermediate_root;
            fees.push(fee);
            block_numbers.push(block_number);

            // flatten the public transaction data
            public_data_vector.extend(public_data.into_iter());
        }

        // return  Ok(());
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

        for _ in 0..(params::FR_BIT_WIDTH - block_number_bits.len()) {
            block_number_bits.push(boolean::Boolean::Constant(false));
        }
        block_number_bits.reverse();
        initial_hash_data.extend(block_number_bits.into_iter());

        let mut total_fees_bits = total_fee_allocated.into_bits_le(
            cs.namespace(|| "unpack fees for hashing")
        )?;

        for _ in 0..(params::FR_BIT_WIDTH - total_fees_bits.len()) {
            total_fees_bits.push(boolean::Boolean::Constant(false));
        }
        total_fees_bits.reverse();
        initial_hash_data.extend(total_fees_bits.into_iter());

        assert_eq!(initial_hash_data.len(), 512);

        let mut hash_block = sha256::sha256(
            cs.namespace(|| "initial rolling sha256"),
            &initial_hash_data
        )?;

        // now we do a "dense packing", i.e. take 256 / public_data.len() items 
        // and push them into the second half of sha256 block

        let public_data_size = params::BALANCE_TREE_DEPTH 
                                    + params::BALANCE_TREE_DEPTH
                                    + params::AMOUNT_EXPONENT_BIT_WIDTH
                                    + params::AMOUNT_MANTISSA_BIT_WIDTH
                                    + params::FEE_EXPONENT_BIT_WIDTH
                                    + params::FEE_MANTISSA_BIT_WIDTH;


        // pad with zeroes up to the block size
        let required_padding = 256 - (public_data_vector.len() % public_data_size);

        // let pack_by = 256 / public_data_size;

        // let number_of_packs = self.number_of_transactions / pack_by;
        // let remaining_to_pack = self.number_of_transactions % pack_by;
        // let padding_in_pack = 256 - pack_by*public_data_size;
        // let padding_in_remainder = 256 - remaining_to_pack*public_data_size;

        // let mut public_data_iterator = public_data_vector.into_iter();

        // for i in 0..number_of_packs 
        // {
        //     let cs = & mut cs.namespace(|| format!("packing a batch number {}", i));
        //     let mut pack_bits: Vec<boolean::Boolean> = vec![];
        //     // put previous hash as first 256 bits of the SHA256 block
        //     pack_bits.extend(hash_block.into_iter());

        //     for _ in 0..pack_by 
        //     {
        //         let next: Vec<boolean::Boolean> = public_data_iterator.next().get()?.clone();
        //         pack_bits.extend(next);
        //     }

        //     for _ in 0..padding_in_pack
        //     {
        //         pack_bits.push(boolean::Boolean::Constant(false));
        //     }

        //     hash_block = sha256::sha256(
        //         cs.namespace(|| format!("hash for block {}", i)),
        //         &pack_bits
        //     )?;
        // }

        // // now pack the remainder

        // let mut pack_bits: Vec<boolean::Boolean> = vec![];
        // pack_bits.extend(hash_block.into_iter());
        // for _ in 0..remaining_to_pack
        // {
        //     let next: Vec<boolean::Boolean> = public_data_iterator.next().get()?.clone();
        //     pack_bits.extend(next);
        // }

        // for _ in 0..padding_in_remainder
        // {
        //     pack_bits.push(boolean::Boolean::Constant(false));
        // }

        // hash_block = sha256::sha256(
        //     cs.namespace(|| "hash the remainder"),
        //     &pack_bits
        // )?;

        let mut pack_bits = vec![];
        pack_bits.extend(hash_block);
        pack_bits.extend(public_data_vector.into_iter());

        // for _ in 0..required_padding
        // {
        //     pack_bits.push(boolean::Boolean::Constant(false));
        // }

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
 
    // let mut no_divergence_bool = boolean::Boolean::from(
    //     boolean::AllocatedBit::alloc(
    //         cs.namespace(|| "Allocate divergence bit initial value"),
    //         Some(true)
    //     )?
    // );

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

/// Applies one transaction to the tree,
/// outputs a new root
fn apply_transaction<E, CS>(
    mut cs: CS,
    old_root: AllocatedNum<E>,
    transaction: Option<(Transaction<E>, TransactionWitness<E>)>,
    params: &E::Params,
    generator: ecc::EdwardsPoint<E>
) -> Result<(AllocatedNum<E>, AllocatedNum<E>, AllocatedNum<E>, Vec<boolean::Boolean>), SynthesisError>
    where E: JubjubEngine,
          CS: ConstraintSystem<E>
{
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

    // for _ in 0..(params::FR_BIT_WIDTH - pub_x_content_from.len())
    // {
    //     pub_x_content_from.push(boolean::Boolean::Constant(false));
    // }
    leaf_content.extend(pub_x_content_from.clone());

    let mut pub_y_content_from = sender_pk_y.into_bits_le(
        cs.namespace(|| "from leaf pub_y bits")
    )?;

    pub_y_content_from.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));

    // for _ in 0..(params::FR_BIT_WIDTH - pub_y_content_from.len())
    // {
    //     pub_y_content_from.push(boolean::Boolean::Constant(false));
    // }
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

    // print!("Inside the snark leaf hash from = {}\n", cur_from.get_value().unwrap());

    let audit_path_from = transaction.get()?.1.clone().auth_path_from;
    // Ascend the merkle tree authentication path
    for (i, e) in audit_path_from.clone().into_iter().enumerate() {
        let cs = &mut cs.namespace(|| format!("from merkle tree hash {}", i));

        // Determines if the current subtree is the "right" leaf at this
        // depth of the tree.
        let cur_is_right = boolean::Boolean::from(
            boolean::AllocatedBit::alloc(
            cs.namespace(|| "position bit"),
            e.map(|e| e.1)
        )?);

        // Constraint this bit immediately
        boolean::Boolean::enforce_equal(
            cs.namespace(|| "position bit is equal to sender address field bit"),
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

    // for _ in 0..(params::FR_BIT_WIDTH - pub_x_content_to.len())
    // {
    //     pub_x_content_to.push(boolean::Boolean::Constant(false));
    // }
    leaf_content.extend(pub_x_content_to.clone());
    
    let mut pub_y_content_to = recipient_pk_y.into_bits_le(
        cs.namespace(|| "to leaf pub_y bits")
    )?;

    pub_y_content_to.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));

    // for _ in 0..(params::FR_BIT_WIDTH - pub_y_content_to.len())
    // {
    //     pub_y_content_to.push(boolean::Boolean::Constant(false));
    // }
    leaf_content.extend(pub_y_content_to.clone());

    assert_eq!(leaf_content.len(), params::BALANCE_BIT_WIDTH 
                                + params::NONCE_BIT_WIDTH
                                + 2 * (params::FR_BIT_WIDTH)
    );

    // Compute the hash of the from leaf
    let mut to_leaf_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "to leaf content hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &leaf_content,
        params
    )?;

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
    for (i, e) in audit_path_to.clone().into_iter().enumerate() {
        let cs = &mut cs.namespace(|| format!("to merkle tree hash {}", i));

        // Determines if the current subtree is the "right" leaf at this
        // depth of the tree.
        let cur_is_right = boolean::Boolean::from(boolean::AllocatedBit::alloc(
            cs.namespace(|| "position bit"),
            e.map(|e| e.1)
        )?);

        // Constraint this bit immediately
        boolean::Boolean::enforce_equal(
            cs.namespace(|| "position bit is equal to recipient address field bit"),
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

    // Initial leaf values are allocated, not we can find a common prefix

    // before having fun with leafs calculate the common prefix
    // of two audit paths

    // let mut common_prefix: Vec<boolean::Boolean> = vec![];
    // Intersection point is the only element required in outside scope
    let mut intersection_point_lc = Num::<E>::zero();
    {
        let cs = & mut cs.namespace(|| "common prefix search");

        let mut reversed_path_from = audit_path_from.clone();
        reversed_path_from.reverse();
        
        let mut bitmap_path_from = from_path_bits.clone();
        bitmap_path_from.reverse();
        
        let mut bitmap_path_to = to_path_bits.clone();
        bitmap_path_to.reverse();

        let mut reversed_path_to = audit_path_to.clone();
        reversed_path_to.reverse();

        let common_prefix = find_common_prefix(
            cs.namespace(|| "common prefix search"), 
            &bitmap_path_from,
            &bitmap_path_to
        )?;


        let common_prefix_iter = common_prefix.clone().into_iter();
        // Common prefix is found, not we enforce equality of 
        // audit path elements on a common prefix

        for (i, ((e_from, e_to), bitmask_bit)) in reversed_path_from.into_iter().zip(reversed_path_to.into_iter()).zip(common_prefix_iter).enumerate()
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
    }

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

    // Ok, old leaf values are exposed, so we can check 
    // the signature and parse the rest of transaction data

    let mut message_bits = vec![];

    // add sender and recipient addresses to check
    message_bits.extend(from_path_bits.clone());
    message_bits.extend(to_path_bits.clone());

    let transaction_amount_allocated = AllocatedNum::alloc(
        cs.namespace(|| "allocate transaction amount"),
        || {
            let tx = &transaction.get()?.0;
            Ok(*tx.amount.get()?)
        }
    )?;

    let mut amount_bits = transaction_amount_allocated.into_bits_le(
        cs.namespace(|| "amount bits")
    )?;

    amount_bits.truncate(params::AMOUNT_EXPONENT_BIT_WIDTH + params::AMOUNT_MANTISSA_BIT_WIDTH);
    
    // add amount to check
    message_bits.extend(amount_bits.clone());

    let transaction_fee_allocated = AllocatedNum::alloc(
        cs.namespace(|| "transaction fee"),
        || {
            let tx = &transaction.get()?.0;
            Ok(*tx.fee.get()?)
        }
    )?;

    let mut fee_bits = transaction_fee_allocated.into_bits_le(
        cs.namespace(|| "fee bits")
    )?;

    fee_bits.truncate(params::FEE_EXPONENT_BIT_WIDTH + params::FEE_MANTISSA_BIT_WIDTH);

    // add fee to check
    message_bits.extend(fee_bits.clone());

    // add nonce to check
    message_bits.extend(nonce_content_from.clone());

    let transaction_max_block_number_allocated = AllocatedNum::alloc(
        cs.namespace(|| "allocate transaction good until block"),
        || {
            let tx = &transaction.get()?.0;
            Ok(*tx.good_until_block.get()?)
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
        &sender_pk_x,
        &sender_pk_y,
        params
    )?;

    let signature_r_x = AllocatedNum::alloc(
        cs.namespace(|| "signature r x"),
        || {
            let tx = &transaction.get()?.0;
            Ok(tx.signature.get()?.r.into_xy().0)
        }
    )?;

    let signature_r_y = AllocatedNum::alloc(
        cs.namespace(|| "signature r y"),
        || {
            let tx = &transaction.get()?.0;
            Ok(tx.signature.get()?.r.into_xy().1)
        }
    )?;

    let signature_r = ecc::EdwardsPoint::interpret(
        cs.namespace(|| "signature r"),
        &signature_r_x,
        &signature_r_y,
        params
    )?;

    let signature_s = AllocatedNum::alloc(
        cs.namespace(|| "signature s"),
        || {
            let tx = &transaction.get()?.0;
            Ok(tx.signature.get()?.s)
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

    let amount = parse_with_exponent_le(
        cs.namespace(|| "parse amount"),
        &amount_bits,
        params::AMOUNT_EXPONENT_BIT_WIDTH,
        params::AMOUNT_MANTISSA_BIT_WIDTH,
        10
    )?;

    let fee = parse_with_exponent_le(
        cs.namespace(|| "parse fee"),
        &fee_bits,
        params::FEE_EXPONENT_BIT_WIDTH,
        params::FEE_MANTISSA_BIT_WIDTH,
        10
    )?;

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
        params::NONCE_BIT_WIDTH
    )?;

    // enforce reduction of balance
    cs.enforce(
        || "enforce sender's balance reduced",
        |lc| lc + old_balance_from.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + new_balance_from.get_variable() + fee.get_variable() + amount.get_variable()
    );

    let new_balance_to = AllocatedNum::alloc(
        cs.namespace(|| "new balance to"),
        || {
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

    // enforce increase of balance
    cs.enforce(
        || "enforce recipients's balance increased",
        |lc| lc + new_balance_to.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + old_balance_to.get_variable() + amount.get_variable()
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

    let allocated_block_number = AllocatedNum::alloc(
        cs.namespace(|| "allocate block number in transaction"),
        || {
            let tx = &transaction.get()?.0;
            Ok(tx.good_until_block.get()?.clone())
        }
    )?;

    // Now we should assemble a new root. It's more tricky as it requires
    // to calculate an intersection point and for a part of the tree that is
    // "below" intersection point use individual merkle brancher,
    // for the intersection - use the other current value,
    // for the rest - use any of the braches, as it's constrained that 
    // those coincide

    // first of new "from" leaf
    {

        let mut leaf_content = vec![];

        // change balance and nonce

        let mut value_content = new_balance_from.into_bits_le(
            cs.namespace(|| "from leaf updated amount bits")
        )?;


        value_content.truncate(params::BALANCE_BIT_WIDTH);
        leaf_content.extend(value_content.clone());

        let mut nonce_content = new_nonce.into_bits_le(
            cs.namespace(|| "from leaf updated nonce bits")
        )?;

        nonce_content.truncate(params::NONCE_BIT_WIDTH);
        leaf_content.extend(nonce_content);

        // keep public keys
        leaf_content.extend(pub_x_content_from);
        leaf_content.extend(pub_y_content_from);

        assert_eq!(leaf_content.len(), params::BALANCE_BIT_WIDTH 
                                    + params::NONCE_BIT_WIDTH
                                    + 2 * (params::FR_BIT_WIDTH)
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

        value_content.truncate(params::BALANCE_BIT_WIDTH);
        leaf_content.extend(value_content.clone());

        // everything else remains the same
        leaf_content.extend(nonce_content_to);
        leaf_content.extend(pub_x_content_to);
        leaf_content.extend(pub_y_content_to);

        assert_eq!(leaf_content.len(), params::BALANCE_BIT_WIDTH 
                                    + params::NONCE_BIT_WIDTH
                                    + 2 * (params::FR_BIT_WIDTH)
        );


        // Compute the hash of the from leaf
        to_leaf_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "to leaf content hash updated"),
            pedersen_hash::Personalization::NoteCommitment,
            &leaf_content,
            params
        )?;

    }

    // Intersection point into bits to use for root recalculation

    let mut intersection_point_bits = intersection_point.into_bits_le(
        cs.namespace(|| "unpack intersection")
    )?;

    // truncating guarantees that even if the common prefix coincides everywhere
    // up to the last bit, it can still be properly used in next actions
    intersection_point_bits.truncate(params::BALANCE_TREE_DEPTH);
    // reverse cause bits here are counted from root, and later we need from the leaf
    intersection_point_bits.reverse();

    // First assemble new leafs
    cur_from = from_leaf_hash.get_x().clone();
    cur_to = to_leaf_hash.get_x().clone();

    {
        let audit_path_from = transaction.get()?.1.auth_path_from.clone();
        let audit_path_to = transaction.get()?.1.auth_path_to.clone();
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

            // If we are on intersection place take a current hash from another branch instead of path element
            path_element_from = num::AllocatedNum::conditionally_select(
                cs.namespace(|| "conditional select of preimage from"),
                &cur_to,
                &path_element_from, 
                &intersection_bit
            )?;

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

            // If we are on intersection place take a current hash from another branch instead of path element
            path_element_to = num::AllocatedNum::conditionally_select(
                cs.namespace(|| "conditional select of preimage to"),
                &cur_from,
                &path_element_to, 
                &intersection_bit
            )?;

            // Swap the two if the current subtree is on the right
            let (xl_to, xr_to) = num::AllocatedNum::conditionally_reverse(
                cs.namespace(|| "conditional reversal of preimage to"),
                &cur_to,
                &path_element_to,
                &cur_to_is_right
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

    let mut public_data = vec![];
    public_data.extend(from_path_bits.clone());
    public_data.extend(to_path_bits.clone());
    public_data.extend(amount_bits.clone());
    public_data.extend(fee_bits.clone());

    assert_eq!(public_data.len(), params::BALANCE_TREE_DEPTH 
                                    + params::BALANCE_TREE_DEPTH
                                    + params::AMOUNT_EXPONENT_BIT_WIDTH
                                    + params::AMOUNT_MANTISSA_BIT_WIDTH
                                    + params::FEE_EXPONENT_BIT_WIDTH
                                    + params::FEE_MANTISSA_BIT_WIDTH);

    Ok((cur_from, fee, allocated_block_number, public_data))
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
fn test_update_circuit_with_witness() {
    use ff::{Field};
    use pairing::bn256::*;
    use rand::{SeedableRng, Rng, XorShiftRng, Rand};
    use sapling_crypto::circuit::test::*;
    use sapling_crypto::alt_babyjubjub::{AltJubjubBn256, fs, edwards, PrimeOrder};
    use crate::models::plasma_models::{AccountTree, Account};
    use crypto::sha2::Sha256;
    use crypto::digest::Digest;

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
        print!("Initial root = {}\n", initial_root);

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

        let path_from : Vec<Option<(Fr, bool)>> = tree.merkle_path(sender_leaf_number).into_iter().map(|e| Some(e)).collect();
        let path_to: Vec<Option<(Fr, bool)>>  = tree.merkle_path(recipient_leaf_number).into_iter().map(|e| Some(e)).collect();

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

        let transaction_witness = TransactionWitness {
            auth_path_from: path_from,
            balance_from: Some(sender_leaf.balance),
            nonce_from: Some(sender_leaf.nonce),
            pub_x_from: Some(sender_leaf.pub_x),
            pub_y_from: Some(sender_leaf.pub_y),
            auth_path_to: path_to,
            balance_to: Some(recipient_leaf.balance),
            nonce_to: Some(recipient_leaf.nonce),
            pub_x_to: Some(recipient_leaf.pub_x),
            pub_y_to: Some(recipient_leaf.pub_y)
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

        //assert!(tree.verify_proof(sender_leaf_number, updated_sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));
        //assert!(tree.verify_proof(recipient_leaf_number, updated_recipient_leaf.clone(), tree.merkle_path(recipient_leaf_number)));

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

            // let first_round_bits = multipack::bytes_to_bits(&hash_result.clone());
            
            // for b in first_round_bits.clone() {
            //     if b {
            //         print!("1");
            //     } else {
            //         print!("0");
            //     }
            // }
            // print!("\n");

            let mut packed_transaction_data = vec![];
            let transaction_data = transaction.public_data_into_bits();
            packed_transaction_data.extend(transaction_data.clone().into_iter());
            // for _ in 0..256 - transaction_data.len() {
            //     packed_transaction_data.push(false);
            // }

            // for b in packed_transaction_data.clone() {
            //     if b {
            //         print!("1");
            //     } else {
            //         print!("0");
            //     }
            // }
            // print!("\n");

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

            let instance = Update {
                params: params,
                number_of_transactions: 1,
                old_root: Some(old_root),
                new_root: Some(new_root),
                public_data_commitment: Some(public_data_commitment),
                block_number: Some(Fr::one()),
                total_fee: Some(Fr::zero()),
                transactions: vec![Some((transaction, transaction_witness))],
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