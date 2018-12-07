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

use super::baby_eddsa::EddsaSignature;

#[derive(Clone)]
pub struct BlockWitness<E: JubjubEngine> {
    pub root: Option<E::Fr>,
    pub proof: Vec<Option<(E::Fr, bool)>>,
}

#[derive(Clone)]
pub struct NonInclusion<'a, E: JubjubEngine> {
    pub params: &'a E::Params,

    // Number of blocks that this snark proves non-inclusion for
    pub number_of_blocks: usize,

    // Leaf hash length
    pub leaf_hash_length: usize,

    // Tree depth
    pub tree_depth: usize,

    // Non-inclusion level
    pub interval_length: Option<E::Fr>,

    // Index we prove non-inclusion for
    pub index: Option<E::Fr>,

    // Witnesses
    pub witness: Vec<BlockWitness<E>>,
}

// returns a bit vector with only one for the tree 
fn count_number_of_ones<E, CS>(
        mut cs: CS,
        a: &[boolean::Boolean]
    ) -> Result<AllocatedNum<E>, SynthesisError>
        where E: JubjubEngine,
        CS: ConstraintSystem<E>
{
    let mut counter = Num::zero();
    for bit in a.iter() {
        counter = counter.add_bool_with_coeff(CS::one(), &bit, E::Fr::one());
    }

    let result = AllocatedNum::alloc(
        cs.namespace(|| "number of zeroes number"), 
        || Ok(*counter.get_value().get()?)
    )?;

    cs.enforce(
        || "pack number of ones",
        |lc| lc + result.get_variable(),
        |lc| lc + CS::one(),
        |_| counter.lc(E::Fr::one())
    );

    Ok(result)
}

impl<'a, E: JubjubEngine> Circuit<E> for NonInclusion<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError>
    {
        // Check that transactions are in a right quantity
        assert!(self.number_of_blocks == self.witness.len());

        let mut zero_leaf = Vec::<boolean::Boolean>::with_capacity(self.leaf_hash_length);
        zero_leaf.resize(self.leaf_hash_length, boolean::Boolean::Constant(false));

        // Expose index
        let index = AllocatedNum::alloc(
            cs.namespace(|| "index"),
            || {
                let index_value = self.index;
                Ok(*index_value.get()?)
            }
        )?;
        index.inputize(cs.namespace(|| "index input"))?;

        // Index is expected to be just a coin number
        let mut index_input_bits = index.into_bits_le(
            cs.namespace(|| "index bits")
        )?;
        index_input_bits.truncate(self.tree_depth);

        // Expose level
        let interval = AllocatedNum::alloc(
            cs.namespace(|| "slice interval"),
            || {
                let interval_length_value = self.interval_length;
                Ok(*interval_length_value.get()?)
            }
        )?;
        interval.inputize(cs.namespace(|| "slice interval input"))?;

        // index is expected to be just a coin number
        let mut interval_bits = interval.into_bits_le(
            cs.namespace(|| "level bits")
        )?;
        interval_bits.truncate(self.tree_depth);

        let num_of_ones_in_level = count_number_of_ones(
            cs.namespace(|| "count number of ones"), 
            &interval_bits
        )?;
        
        cs.enforce(
            || "enforce number of ones",
            |lc| lc + CS::one(),
            |lc| lc + CS::one(),
            |lc| lc + num_of_ones_in_level.get_variable()
        );

        // now create a bitmask for higher levels
        let mut level_mask_lc = Num::zero();
        let mut coeff = E::Fr::one();
        for bit in interval_bits.clone().into_iter() {
            level_mask_lc = level_mask_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
            coeff.double();
        }
        // subtract one
        let mut minus_one = E::Fr::one();
        minus_one.negate();
        level_mask_lc = level_mask_lc.add_bool_with_coeff(CS::one(), &boolean::Boolean::Constant(true), minus_one);
        // make number

        let level_mask_allocated = AllocatedNum::alloc(
            cs.namespace(|| "allocate level bitmask"), 
            || Ok(*level_mask_lc.get_value().get()?)
        )?;

        cs.enforce(
            || "enforce bitmask for levels",
            |lc| lc + level_mask_allocated.get_variable(),
            |lc| lc + CS::one(),
            |_| level_mask_lc.lc(E::Fr::one())
        );

        // decompose again

        let mut level_bitmask = level_mask_allocated.into_bits_le(
            cs.namespace(|| "decompose bitmask again")
        )?;

        level_bitmask.truncate(self.tree_depth);

        // precompute empty nodes for a price of one tree depth of hashes

        let mut empty_levels = vec![];

        // Compute the hash of the from leaf
        let empty_leaf_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "empty leaf hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &zero_leaf,
            self.params
        )?;

        let mut cur = empty_leaf_hash.get_x().clone();

        empty_levels.push(cur.clone());

        for i in 0..self.tree_depth {
            let cs = &mut cs.namespace(|| format!("compute empty leafs merkle tree hash {}", i));

            let mut preimage = vec![];
            let cur_bits = cur.into_bits_le(cs.namespace(|| "current into bits"))?;
            preimage.extend(cur_bits.clone());
            preimage.extend(cur_bits);

            // Compute the new subtree value
            cur = pedersen_hash::pedersen_hash(
                cs.namespace(|| "computation of pedersen hash"),
                pedersen_hash::Personalization::MerkleTree(i),
                &preimage,
                self.params
            )?.get_x().clone(); // Injective encoding

            empty_levels.push(cur.clone());
        }

        for (i_witnesses, w) in self.witness.into_iter().enumerate() {
            let cs = &mut cs.namespace(|| format!("block proof number {}", i_witnesses));
            // allocate public input
            let mut proof_input = AllocatedNum::alloc(
                cs.namespace(|| "root"),
                || Ok(*w.root.get()?)
            )?;

            proof_input.inputize(
                cs.namespace(|| "input for root")
            )?;
            
            let audit_path = w.proof;
            assert!(self.tree_depth == audit_path.len());

            // at least at the bottom level there should be zero
            let mut cur = empty_leaf_hash.get_x().clone();

            // Ascend the merkle tree authentication path
            for (i_tree_levels, (e, level_bit)) in audit_path.clone().into_iter().zip(level_bitmask.clone().into_iter()).enumerate() {
                let cs_w = &mut cs.namespace(|| format!("merkle tree hash {}", i_tree_levels));

                // Determines if the current subtree is the "right" leaf at this
                // depth of the tree.

                let cur_is_right = boolean::Boolean::from(
                    boolean::AllocatedBit::alloc(
                    cs_w.namespace(|| "position bit"),
                    e.map(|e| e.1)
                )?);

                // Constraint this bit immediately
                boolean::Boolean::enforce_equal(
                    cs_w.namespace(|| "position bit is equal the start of the interval field bit"),
                    &cur_is_right, 
                    &index_input_bits[i_tree_levels]
                )?;

                cur = num::AllocatedNum::conditionally_select(
                    cs_w.namespace(|| "conditional select of empty or not leaf hash"),
                    &empty_levels[i_tree_levels], 
                    &cur,
                    &level_bit
                )?;

                // Witness the authentication path element adjacent
                // at this depth.
                let mut path_element = num::AllocatedNum::alloc(
                    cs_w.namespace(|| "path element"),
                    || {
                        Ok(e.get()?.0)
                    }
                )?;

                // Swap the two if the current subtree is on the right
                let (xl, xr) = num::AllocatedNum::conditionally_reverse(
                    cs_w.namespace(|| "conditional reversal of preimage"),
                    &cur,
                    &path_element,
                    &cur_is_right
                )?;

                // We don't need to be strict, because the function is
                // collision-resistant. If the prover witnesses a congruency,
                // they will be unable to find an authentication path in the
                // tree with high probability.
                let mut preimage = vec![];
                preimage.extend(xl.into_bits_le(cs_w.namespace(|| "xl into bits"))?);
                preimage.extend(xr.into_bits_le(cs_w.namespace(|| "xr into bits"))?);

                // Compute the new subtree value
                cur = pedersen_hash::pedersen_hash(
                    cs_w.namespace(|| "computation of pedersen hash"),
                    pedersen_hash::Personalization::MerkleTree(i_tree_levels),
                    &preimage,
                    self.params
                )?.get_x().clone(); // Injective encoding
            }

            // enforce old root before update
            cs.enforce(
                || "enforce correct root hash",
                |lc| lc + cur.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + proof_input.get_variable()
            );
        }

        Ok(())
    }
}

const TREE_DEPTH: u32 = 24;
const NUMBER_OF_BLOCKS_TO_PROVE: u32 = 1;

#[test]
fn test_non_inclusion_proof() {
    use ff::{Field};
    use pairing::bn256::*;
    use rand::{SeedableRng, Rng, XorShiftRng, Rand};
    use sapling_crypto::circuit::test::*;
    use sapling_crypto::alt_babyjubjub::{AltJubjubBn256, fs, edwards, PrimeOrder};
    use transaction_tree::{BabyTransactionTree, BabyTransactionLeaf, Leaf};

    extern crate hex;

    let p_g = FixedGenerators::SpendingKeyGenerator;
    let params = &AltJubjubBn256::new();

    let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let non_inclusion_level = 2;
    println!("Proving for intersection level = {}", non_inclusion_level);

    let interval_length = Fr::from_str(&(1 << non_inclusion_level).to_string()).unwrap();
    println!("Interval length = {}", interval_length);

    let mut witnesses = vec![];

    let start_of_slice = 0u32;
    let index_as_field_element = Fr::from_str(&start_of_slice.to_string()).unwrap();

    for _ in 0..NUMBER_OF_BLOCKS_TO_PROVE {
        // create an empty tree

        let mut tree = BabyTransactionTree::new(TREE_DEPTH);

        let empty_tree_root = tree.root_hash();
        println!("Empty root hash = {}", empty_tree_root);

        // test will prove the large [0, 3] (length 4), 
        // so we need to enter non-zero element at the leaf number 4

        let mut random_bools = vec![];
        for _ in 0..256 {
            let bit: bool = rng.gen::<bool>();
            random_bools.push(bit);
        }

        let empty_leaf = BabyTransactionLeaf::default();

        let non_empty_leaf = BabyTransactionLeaf {
                hash:    random_bools,
                phantom: std::marker::PhantomData
        };

        println!("Inserting a non-empty leaf");

        let slice_len = 1 << non_inclusion_level;

        tree.insert(slice_len, non_empty_leaf.clone());

        let root = tree.root_hash();
        println!("Root = {}", root);

        println!("Checking reference proofs");

        // assert!(tree.verify_proof(slice_len, non_empty_leaf.clone(), tree.merkle_path(slice_len)));
        assert!(tree.verify_proof(start_of_slice, empty_leaf.clone(), tree.merkle_path(start_of_slice)));

        let proof = tree.merkle_path(start_of_slice);
        let proof_as_some: Vec<Option<(Fr, bool)>> = proof.into_iter().map(|e| Some(e)).collect();

        let block_witness: BlockWitness<Bn256> = BlockWitness {
            root: Some(root),
            proof: proof_as_some
        };

        witnesses.push(block_witness);
    }

    {
        assert_eq!(witnesses.len(), NUMBER_OF_BLOCKS_TO_PROVE as usize);

        let mut cs = TestConstraintSystem::<Bn256>::new();

        let instance = NonInclusion {
            params: params,
            number_of_blocks: NUMBER_OF_BLOCKS_TO_PROVE as usize,
            leaf_hash_length: 256,
            tree_depth: TREE_DEPTH as usize,
            interval_length: Some(interval_length),
            index: Some(index_as_field_element),
            witness: witnesses,
        };

        println!("Synthsizing a snark for {} block for {} tree depth", NUMBER_OF_BLOCKS_TO_PROVE, TREE_DEPTH);

        instance.synthesize(&mut cs).unwrap();

        println!("{}", cs.find_unconstrained());

        println!("Number of constraints = {}", cs.num_constraints());
        // inputs are ONE, starting index, slice length + root * number of blocks 
        assert_eq!(cs.num_inputs(), (1 + 1 + 1 + NUMBER_OF_BLOCKS_TO_PROVE) as usize);

        let err = cs.which_is_unsatisfied();
        if err.is_some() {
            panic!("ERROR satisfying in {}\n", err.unwrap());
        }
    }
}