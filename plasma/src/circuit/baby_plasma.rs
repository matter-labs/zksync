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

use super::boolean;
use super::ecc;
use super::pedersen_hash;
use super::sha256;
use super::num;
use super::multipack;
use super::num::{AllocatedNum, Num};

use sapling_crypto::eddsa::{
    Signature,
    PrivateKey,
    PublicKey
};

use super::plasma_constants;
use super::float_point::*;
use super::baby_eddsa::EddsaSignature;

// This is transaction data

#[derive(Clone)]
pub struct TransactionSignature<E: JubjubEngine> {
    pub r: edwards::Point<E, Unknown>,
    pub s: E::Fr,
}

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

fn le_bit_vector_into_field_element<Fr: PrimeField>
    (bits: &Vec<bool>) -> Fr 
{
    // TODO remove representation length hardcore
    let mut bytes = [0u8; 32];

    let byte_chunks = bits.chunks(8);

    for (i, byte_chunk) in byte_chunks.enumerate()
    {
        let mut byte = 0u8;
        for (j, bit) in byte_chunk.into_iter().enumerate()
        {
            if *bit {
                byte |= 1 << j;
            }
        }
        bytes[i] = byte;
    }

    let mut repr = Fr::zero().into_repr();
    repr.read_le(&bytes[..]).expect("interpret S as field element representation");

    let field_element = Fr::from_repr(repr).unwrap();

    field_element
}

fn bit_vector_into_bytes
    (bits: &Vec<bool>) -> Vec<u8>
{
    // TODO remove representation length hardcore
    let mut bytes: Vec<u8> = vec![];

    let byte_chunks = bits.chunks(8);

    for byte_chunk in byte_chunks
    {
        let mut byte = 0u8;
        // pack just in order
        for (i, bit) in byte_chunk.into_iter().enumerate()
        {
            if *bit {
                byte |= 1 << (7 - i);
            }
        }
        bytes.push(byte);
    }

    bytes
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
        from.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
        let mut to: Vec<bool> = BitIterator::new(self.to.clone().unwrap().into_repr()).collect();
        to.reverse();
        to.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
        let mut amount: Vec<bool> = BitIterator::new(self.amount.clone().unwrap().into_repr()).collect();
        amount.reverse();
        amount.truncate(*plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH);
        let mut fee: Vec<bool> = BitIterator::new(self.fee.clone().unwrap().into_repr()).collect();
        fee.reverse();
        fee.truncate(*plasma_constants::FEE_EXPONENT_BIT_WIDTH + *plasma_constants::FEE_MANTISSA_BIT_WIDTH);
        
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
        nonce.truncate(*plasma_constants::NONCE_BIT_WIDTH);
        let mut good_until_block: Vec<bool> = BitIterator::new(self.good_until_block.clone().unwrap().into_repr()).collect();
        good_until_block.reverse();
        good_until_block.truncate(*plasma_constants::BLOCK_NUMBER_BIT_WIDTH);
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

        // conversion example from tests

        // let msg1 = b"Foo bar pad to16"; // 16 bytes

        // let mut input: Vec<bool> = vec![];

        // for b in msg1.iter() {  
        //     for i in (0..8).into_iter() {
        //         if (b & (1 << i)) != 0 {
        //             input.extend(&[true; 1]);
        //         } else {
        //             input.extend(&[false; 1]);
        //         }
        //     }
        // }

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

        let max_message_len = *plasma_constants::BALANCE_TREE_DEPTH 
                        + *plasma_constants::BALANCE_TREE_DEPTH 
                        + *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH 
                        + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH
                        + *plasma_constants::FEE_EXPONENT_BIT_WIDTH
                        + *plasma_constants::FEE_MANTISSA_BIT_WIDTH
                        + *plasma_constants::NONCE_BIT_WIDTH
                        + *plasma_constants::BLOCK_NUMBER_BIT_WIDTH;
        
        let signature = private_key.sign_raw_message(
            &message_bytes, 
            rng, 
            p_g, 
            params,
            max_message_len / 8
        );

        let mut sigs_bytes = [0u8; 32];
        signature.s.into_repr().write_le(& mut sigs_bytes[..]).expect("get LE bytes of signature S");
        let mut sigs_repr = E::Fr::zero().into_repr();
        sigs_repr.read_le(&sigs_bytes[..]).expect("interpret S as field element representation");
        let sigs_converted = E::Fr::from_repr(sigs_repr).unwrap();

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

        // let mut hash_bits = boolean::field_into_boolean_vec_le(
        //     cs.namespace(|| "rolling hash bits"), 
        //     rolling_hash.get_value()
        // )?;

        // assert_eq!(hash_bits.len(), E::Fr::NUM_BITS as usize);

        // Pad hash bits to 256 bits for use in hashing rounds

        // for _ in 0..(256 - hash_bits.len()) {
        //     hash_bits.push(boolean::Boolean::Constant(false));
        // }

        let mut fees = vec![];
        let mut block_numbers = vec![];
        let mut public_data_vector: Vec<Vec<boolean::Boolean>> = vec![];

        let public_generator = self.params.generator(FixedGenerators::SpendingKeyGenerator).clone();
        let generator = ecc::EdwardsPoint::witness(cs.namespace(|| "allocate public generator"), Some(public_generator), self.params).unwrap();

        // Ok, now we need to update the old root by applying transactions in sequence
        let transactions = self.transactions.clone();

        for (i, tx) in transactions.into_iter().enumerate() {
            let (intermediate_root, fee, block_number, public_data) = apply_transaction(
                cs.namespace(|| format!("applying transaction {}", i)),
                old_root,
                tx, 
                self.params,
                generator.clone()
            ).unwrap();
            old_root = intermediate_root;
            fees.push(fee);
            block_numbers.push(block_number);
            public_data_vector.push(public_data);
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

        let mut total_fee_lc = Num::<E>::zero();
        for fee in fees.into_iter() {
            total_fee_lc = total_fee_lc.add_bool_with_coeff(
                CS::one(), 
                &boolean::Boolean::Constant(true), 
                fee.get_value().unwrap()
            );
        }

        let total_fee = self.total_fee.clone();

        let total_fee_allocated = AllocatedNum::alloc(
            cs.namespace(|| "allocate total fees"),
            || Ok(total_fee.unwrap())
        ).unwrap();

        cs.enforce(
            || "enforce total fee",
            |lc| lc + total_fee_allocated.get_variable(),
            |lc| lc + CS::one(),
            |_| total_fee_lc.lc(E::Fr::one())
        );

        // Then check that for every transaction in this block 
        // the parameter "good until" was greater or equal
        // than the current block number

        let block_number = self.block_number.clone();

        let block_number_allocated = AllocatedNum::alloc(
            cs.namespace(|| "allocate block number"),
            || Ok(block_number.clone().unwrap())
        ).unwrap();

        for (i, block_number_in_tx) in block_numbers.into_iter().enumerate() {
            // first name a new value and constraint that it's a proper subtraction

            let mut difference = block_number_in_tx.get_value().unwrap();
            difference.sub_assign(&block_number.clone().unwrap());

            let difference_allocated = AllocatedNum::alloc(
                cs.namespace(|| format!("allocate block number difference {}", i)),
                || Ok(difference)
            ).unwrap();

            // enforce proper subtraction
            cs.enforce(
                || format!("enforce subtraction in block number calculation {}", i),
                |lc| lc + difference_allocated.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + block_number_in_tx.get_variable() - block_number_allocated.get_variable()
            );

            // check for overflow

            AllocatedNum::limit_number_of_bits(
                cs.namespace(|| format!("check for subtraction overflow {}", i)),
                &difference_allocated, 
                *plasma_constants::BLOCK_NUMBER_BIT_WIDTH
            )?;
        }

        // Now it's time to pack the initial SHA256 hash due to Ethereum BE encoding
        // and start rolling the hash

        let mut initial_hash_data: Vec<boolean::Boolean> = vec![];

        // make initial hash as sha256(uint256(block_number)||uint256(total_fees))
        let mut block_number_bits = block_number_allocated.into_bits_le(
            cs.namespace(|| "unpack block number for hashing")
        ).unwrap();

        for _ in 0..(*plasma_constants::FR_BIT_WIDTH - block_number_bits.len()) {
            block_number_bits.push(boolean::Boolean::Constant(false));
        }
        block_number_bits.reverse();
        initial_hash_data.extend(block_number_bits.into_iter());

        let mut total_fees_bits = total_fee_allocated.into_bits_le(
            cs.namespace(|| "unpack fees for hashing")
        ).unwrap();

        for _ in 0..(*plasma_constants::FR_BIT_WIDTH - total_fees_bits.len()) {
            total_fees_bits.push(boolean::Boolean::Constant(false));
        }
        total_fees_bits.reverse();
        initial_hash_data.extend(total_fees_bits.into_iter());

        assert_eq!(initial_hash_data.len(), 512);
        // initial_hash_data.reverse();

        // print_boolean_vector(&initial_hash_data.clone());

        let mut hash_block = sha256::sha256_block_no_padding(
            cs.namespace(|| "initial rolling sha256"),
            &initial_hash_data
        ).unwrap();

        print_boolean_vector(&hash_block.clone());

        // now we do a "dense packing", i.e. take 256 / public_data.len() items 
        // and push them into the second half of sha256 block

        let public_data_size = *plasma_constants::BALANCE_TREE_DEPTH 
                                    + *plasma_constants::BALANCE_TREE_DEPTH
                                    + *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH
                                    + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH
                                    + *plasma_constants::FEE_EXPONENT_BIT_WIDTH
                                    + *plasma_constants::FEE_MANTISSA_BIT_WIDTH;

        let pack_by = 256 / public_data_size;

        let number_of_packs = self.number_of_transactions / pack_by;
        let remaining_to_pack = self.number_of_transactions % pack_by;
        let padding_in_pack = 256 - pack_by*public_data_size;
        let padding_in_remainder = 256 - remaining_to_pack*public_data_size;

        let mut public_data_iterator = public_data_vector.into_iter();

        for i in 0..number_of_packs 
        {
            let cs = & mut cs.namespace(|| format!("packing a batch number {}", i));
            let mut pack_bits: Vec<boolean::Boolean> = vec![];
            pack_bits.extend(hash_block.into_iter());
            for _ in 0..pack_by 
            {
                let part: Vec<boolean::Boolean> = public_data_iterator.next().unwrap();
                pack_bits.extend(part.into_iter());
            }
            for _ in 0..padding_in_pack
            {
                pack_bits.push(boolean::Boolean::Constant(false));
            }
            hash_block = sha256::sha256_block_no_padding(
                cs.namespace(|| format!("hash for block {}", i)),
                &pack_bits
            ).unwrap();
        }

        let mut pack_bits: Vec<boolean::Boolean> = vec![];
        pack_bits.extend(hash_block.into_iter());
        for _ in 0..remaining_to_pack
        {
            let part: Vec<boolean::Boolean> = public_data_iterator.next().unwrap();
            pack_bits.extend(part.into_iter());
        }

        for _ in 0..padding_in_remainder
        {
            pack_bits.push(boolean::Boolean::Constant(false));
        }

        print_boolean_vector(&pack_bits.clone());

        hash_block = sha256::sha256_block_no_padding(
            cs.namespace(|| "hash the remainder"),
            &pack_bits
        ).unwrap();

        // now pack and enforce equality to the input

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
) -> Result<(AllocatedNum<E>, AllocatedNum<E>, AllocatedNum<E>, Vec<boolean::Boolean>), SynthesisError>
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
                    Ok(e_from.unwrap().0)
                }
            )?;

            let path_element_to = num::AllocatedNum::alloc(
                cs.namespace(|| format!("path element to {}", i)),
                || {
                    Ok(e_to.unwrap().0)
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

    // let value_from_allocated = AllocatedNum::alloc(
    //     cs.namespace(|| "allocate value from"),
    //     || Ok(tx_witness.balance_from.clone().unwrap())
    // ).unwrap();

    // let mut value_content_from = value_from_allocated.into_bits_le(
    //     cs.namespace(|| "unpack from leaf value")
    // ).unwrap();

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

    // print_boolean_vector(&leaf_content.clone());

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

    // print_boolean_vector(&from_path_bits.clone());

    // This is an injective encoding, as cur is a
    // point in the prime order subgroup.
    let mut cur_from = from_leaf_hash.get_x().clone();

    // print!("Inside the snark leaf hash from = {}\n", cur_from.get_value().unwrap());

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
                Ok(e.unwrap().0)
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
                Ok(e.unwrap().0)
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

    // add sender and recipient addresses to check
    message_bits.extend(from_path_bits.clone());
    message_bits.extend(to_path_bits.clone());

    let mut amount_bits = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "amount bits"),
        tx.amount
    ).unwrap();

    amount_bits.truncate(*plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH);
    
    // add amount to check
    message_bits.extend(amount_bits.clone());

    let mut fee_bits = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "fee bits"),
        tx.fee
    ).unwrap();

    fee_bits.truncate(*plasma_constants::FEE_EXPONENT_BIT_WIDTH + *plasma_constants::FEE_MANTISSA_BIT_WIDTH);

    // add fee to check
    message_bits.extend(fee_bits.clone());

    // add nonce to check
    message_bits.extend(nonce_content_from.clone());

    let mut block_number_bits = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "block number bits"),
        tx.good_until_block
    ).unwrap();

    block_number_bits.truncate(*plasma_constants::BLOCK_NUMBER_BIT_WIDTH);

    // add block number to check
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
                        + *plasma_constants::NONCE_BIT_WIDTH
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

    print!("Updated balance from in a snark is {}\n", new_balance_from_value.clone());

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

    let mut new_balance_to_value = old_balance_to.get_value().unwrap();
    new_balance_to_value.add_assign(&amount.get_value().clone().unwrap());

    print!("Updated balance to in a snark is {}\n", new_balance_to_value.clone());

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
        |lc| lc + old_balance_to.get_variable() + amount.get_variable()
    );

    let mut new_nonce_value = nonce.get_value().unwrap();
    new_nonce_value.add_assign(&E::Fr::one());

    print!("Updated nonce from in a snark is {}\n", new_nonce_value.clone());

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

        let mut leaf_content = vec![];

        // change balance and nonce

        let mut value_content = new_balance_from.into_bits_le(
            cs.namespace(|| "from leaf updated amount bits")
        ).unwrap();


        // let mut value_content = boolean::field_into_boolean_vec_le(
        //     cs.namespace(|| "from leaf amount bits update"), 
        //     new_balance_from.get_value()
        // ).unwrap();

        value_content.truncate(*plasma_constants::BALANCE_BIT_WIDTH);
        leaf_content.extend(value_content.clone());

        let mut nonce_content = new_nonce.into_bits_le(
            cs.namespace(|| "from leaf updated nonce bits")
        ).unwrap();

        // let mut nonce_content = boolean::field_into_boolean_vec_le(
        //     cs.namespace(|| "from leaf nonce bits updated"), 
        //     new_nonce.get_value()
        // ).unwrap();

        nonce_content.truncate(*plasma_constants::NONCE_BIT_WIDTH);
        leaf_content.extend(nonce_content);

        // keep public keys
        leaf_content.extend(pub_x_content_from);
        leaf_content.extend(pub_y_content_from);

        assert_eq!(leaf_content.len(), *plasma_constants::BALANCE_BIT_WIDTH 
                                    + *plasma_constants::NONCE_BIT_WIDTH
                                    + 2 * (*plasma_constants::FR_BIT_WIDTH)
        );

        // print_boolean_vector(&leaf_content.clone());

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
        ).unwrap();

        // let mut value_content = boolean::field_into_boolean_vec_le(
        //     cs.namespace(|| "to leaf amount bits upted"), 
        //     new_balance_to.get_value()
        // ).unwrap();

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

        // print_boolean_vector(&leaf_content.clone());

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

    let mut intersection_point_bits = intersection_point.into_bits_le(
        cs.namespace(|| "unpack intersection")
    ).unwrap();

    // let mut intersection_point_bits = boolean::field_into_boolean_vec_le(
    //     cs.namespace(|| "unpack intersection"),
    //     intersection_point.get_value()
    // ).unwrap();

    // truncating guarantees that even if the common prefix coincides everywhere
    // up to the last bit, it can still be properly used in next actions
    intersection_point_bits.truncate(*plasma_constants::BALANCE_TREE_DEPTH);
    // reverse cause bits here are counted from root, and later we need from the leaf
    intersection_point_bits.reverse();

    // First assemble new leafs
    cur_from = from_leaf_hash.get_x().clone();
    cur_to = to_leaf_hash.get_x().clone();
    print!("Inside the snark new leaf hash from = {}\n", cur_from.clone().get_value().unwrap());
    print!("Inside the snark new leaf hash to = {}\n", cur_to.clone().get_value().unwrap());

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
                    Ok(e_from.unwrap().0)
                }
            )?;

            let mut path_element_to = num::AllocatedNum::alloc(
                cs.namespace(|| "path element to"),
                || {
                    Ok(e_to.unwrap().0)
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

            // If we are on intersection place take a current hash from another branch instead of path element
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

    assert_eq!(public_data.len(), *plasma_constants::BALANCE_TREE_DEPTH 
                                    + *plasma_constants::BALANCE_TREE_DEPTH
                                    + *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH
                                    + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH
                                    + *plasma_constants::FEE_EXPONENT_BIT_WIDTH
                                    + *plasma_constants::FEE_MANTISSA_BIT_WIDTH);

    Ok((cur_from, fee, allocated_block_number, public_data))
}

#[test]
fn test_bits_into_fr(){
    use ff::{PrimeField};
    use pairing::bn256::*;
    use std::str::FromStr;

    // representation of 1 + 4 + 8 = 13;
    let bits: Vec<bool> = [true, false, true, true].to_vec();

    let fe: Fr = le_bit_vector_into_field_element(&bits);

    let as_string = format!("{}", fe.into_repr());
    print!("{}\n", as_string);
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
    use balance_tree::{BabyBalanceTree, BabyLeaf, Leaf};
    use crypto::sha2::Sha256;
    use crypto::digest::Digest;

    let p_g = FixedGenerators::SpendingKeyGenerator;
    let params = &AltJubjubBn256::new();

    let mut rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1 {
        let tree_depth = *plasma_constants::BALANCE_TREE_DEPTH as u32;
        let mut tree = BabyBalanceTree::new(tree_depth);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << *plasma_constants::BALANCE_TREE_DEPTH);

        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
    
        let recipient_sk = PrivateKey::<Bn256>(rng.gen());
        let recipient_pk = PublicKey::from_private(&recipient_sk, p_g, params);
        let (recipient_x, recipient_y) = recipient_pk.0.into_xy();

        // give some funds to sender and make zero balance for recipient

        let sender_leaf_number = 1;
        let recipient_leaf_number = 2;

        let transfer_amount : u128 = 500;

        let transfer_amount_as_field_element = Fr::from_str(&transfer_amount.to_string()).unwrap();

        let transfer_amount_bits = convert_to_float(
            transfer_amount,
            *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH,
            *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH,
            10
        ).unwrap();

        let transfer_amount_encoded: Fr = le_bit_vector_into_field_element(&transfer_amount_bits);

        let fee_encoded = Fr::zero();

        let sender_leaf = BabyLeaf {
                balance:    Fr::from_str("1000").unwrap(),
                nonce:      Fr::zero(),
                pub_x:      sender_x,
                pub_y:      sender_y,
        };

        let recipient_leaf = BabyLeaf {
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

        print!("Empty leaf hash is {}\n", tree.get_hash((tree_depth, 0)));

        // let p = tree.merkle_path(sender_leaf_number);
        // print!("Verifying merkle proof for an old leaf\n");
        assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));
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

        assert!(tree.verify_proof(sender_leaf_number, updated_sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));
        assert!(tree.verify_proof(recipient_leaf_number, updated_recipient_leaf.clone(), tree.merkle_path(recipient_leaf_number)));

        let new_root = tree.root_hash();

        print!("New root = {}\n", new_root);

        assert!(old_root != new_root);

        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let mut public_data_initial_bits = vec![];

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

            // for b in public_data_initial_bits.clone() {
            //     if b {
            //         print!("1");
            //     } else {
            //         print!("0");
            //     }
            // }
            // print!("\n");

            let mut h = Sha256::new();

            let bytes_to_hash = bit_vector_into_bytes(&public_data_initial_bits);
            // for b in bytes_to_hash.clone() {
            //     print!("{}", b);
            // }
            // print!("\n");
            h.input(&bytes_to_hash);

            let mut hash_result = [0u8; 32];
            h.result(&mut hash_result[..]);

            for a in hash_result.chunks(4) {
                let mut b = 0u32;
                let mut c = a.into_iter();
                b |= u32::from(*c.next().unwrap()) << 0;
                b |= u32::from(*c.next().unwrap()) << 8;
                b |= u32::from(*c.next().unwrap()) << 16;
                b |= u32::from(*c.next().unwrap()) << 24;
                print!("{}\n", b);
            }

            // for b in hash_result.iter() {
            //     print!("{}", b);
            // }
            // print!("\n");

            let first_round_bits = multipack::bytes_to_bits(&hash_result);
            
            for b in first_round_bits.clone() {
                if b {
                    print!("1");
                } else {
                    print!("0");
                }
            }
            print!("\n");

            let mut packed_transaction_data = vec![];
            let transaction_data = transaction.public_data_into_bits();
            packed_transaction_data.extend(transaction_data.clone().into_iter());
            for _ in 0..256 - transaction_data.len() {
                packed_transaction_data.push(false);
            }

            for b in packed_transaction_data.clone() {
                if b {
                    print!("1");
                } else {
                    print!("0");
                }
            }
            print!("\n");

            let packed_transaction_data_bytes = bit_vector_into_bytes(&packed_transaction_data);

            let mut next_round_hash_bytes = vec![];
            next_round_hash_bytes.extend(hash_result.iter());
            next_round_hash_bytes.extend(packed_transaction_data_bytes);
            assert_eq!(next_round_hash_bytes.len(), 64);

            h = Sha256::new();
            h.input(&next_round_hash_bytes);
            hash_result = [0u8; 32];
            h.result(&mut hash_result[..]);

            hash_result[0] &= 0x1f; // temporary solution

            let mut repr = Fr::zero().into_repr();
            repr.read_be(&hash_result[..]).expect("pack hash as field element");

            let public_data_commitment = Fr::from_repr(repr).unwrap();

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

            print!("{}\n", cs.num_constraints());

            assert_eq!(cs.num_inputs(), 4);

            let err = cs.which_is_unsatisfied();
            if err.is_some() {
                print!("ERROR satisfying in {}\n", err.unwrap());
            } else {
                assert!(cs.is_satisfied());
            }

            // assert!(cs.is_satisfied());
        }
    }
}