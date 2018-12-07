extern crate ff;
extern crate pairing;
extern crate rand;
extern crate sapling_crypto;
extern crate plasma_cash_bonus;
extern crate bellman;

use ff::{PrimeField};
use pairing::bn256::*;
use bellman::Circuit;
use rand::{SeedableRng, Rng, XorShiftRng};
use sapling_crypto::circuit::test::*;
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};
use plasma_cash_bonus::transaction_tree::{BabyTransactionTree, BabyTransactionLeaf};
use plasma_cash_bonus::circuit::non_inclusion::{NonInclusion, BlockWitness};

const TREE_DEPTH: u32 = 24;
const NUMBER_OF_BLOCKS_TO_PROVE: u32 = 128;

fn main() {
    let params = &AltJubjubBn256::new();

    let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let non_inclusion_level = 2;
    // println!("Proving for intersection level = {}", non_inclusion_level);

    let interval_length = Fr::from_str(&(1 << non_inclusion_level).to_string()).unwrap();
    // println!("Interval length = {}", interval_length);

    let mut witnesses = vec![];

    let start_of_slice = 0u32;
    let index_as_field_element = Fr::from_str(&start_of_slice.to_string()).unwrap();

    for _ in 0..NUMBER_OF_BLOCKS_TO_PROVE {
        // create an empty tree

        let mut tree = BabyTransactionTree::new(TREE_DEPTH);

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

        // println!("Inserting a non-empty leaf");

        let slice_len = 1 << non_inclusion_level;

        tree.insert(slice_len, non_empty_leaf.clone());

        let root = tree.root_hash();
        // println!("Root = {}", root);

        // println!("Checking reference proofs");

        assert!(tree.verify_proof(slice_len, non_empty_leaf.clone(), tree.merkle_path(slice_len)));
        assert!(tree.verify_proof(start_of_slice, empty_leaf.clone(), tree.merkle_path(start_of_slice)));

        {
            let proof = tree.merkle_path(start_of_slice);
            let proof_as_some: Vec<Option<(Fr, bool)>> = proof.into_iter().map(|e| Some(e)).collect();

            let block_witness: BlockWitness<Bn256> = BlockWitness {
                root: Some(root),
                proof: proof_as_some
            };

            witnesses.push(block_witness);
        }
    }

    {
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