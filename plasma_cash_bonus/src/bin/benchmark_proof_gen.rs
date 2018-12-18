extern crate ff;
extern crate pairing;
extern crate rand;
extern crate sapling_crypto;
extern crate bellman;
extern crate plasma_cash_history_snark;
extern crate time;

use time::PreciseTime;
use ff::{PrimeField};
use pairing::bn256::*;
use bellman::{Circuit};
use rand::{SeedableRng, Rng, XorShiftRng};
use sapling_crypto::circuit::test::*;
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};
use plasma_cash_history_snark::transaction_tree::{BabyTransactionTree, BabyTransactionLeaf};
use plasma_cash_history_snark::circuit::non_inclusion::{NonInclusion, BlockWitness};

use bellman::groth16::{
    create_random_proof, 
    generate_random_parameters, 
    prepare_verifying_key, 
    verify_proof,
};

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
            let proof_as_some: Vec<Option<Fr>> = proof.into_iter().map(|e| Some(e.0)).collect();

            let block_witness: BlockWitness<Bn256> = BlockWitness {
                root: Some(root),
                proof: proof_as_some
            };

            witnesses.push(block_witness);
        }
    }

    println!("Using test constraint system to check the satisfiability");

    {
        let mut cs = TestConstraintSystem::<Bn256>::new();

        let instance = NonInclusion {
            params: params,
            number_of_blocks: NUMBER_OF_BLOCKS_TO_PROVE as usize,
            leaf_hash_length: 256,
            tree_depth: TREE_DEPTH as usize,
            interval_length: Some(interval_length),
            index: Some(index_as_field_element),
            witness: witnesses.clone(),
        };

        println!("Synthsizing a snark for {} block for {} tree depth", NUMBER_OF_BLOCKS_TO_PROVE, TREE_DEPTH);

        instance.synthesize(&mut cs).unwrap();

        println!("Looking for unconstrained variabled: {}", cs.find_unconstrained());

        println!("Number of constraints = {}", cs.num_constraints());
        // inputs are ONE, starting index, slice length + root * number of blocks 
        // assert_eq!(cs.num_inputs(), (1 + 1 + 1 + NUMBER_OF_BLOCKS_TO_PROVE) as usize);

        let err = cs.which_is_unsatisfied();
        if err.is_some() {
            panic!("ERROR satisfying in {}\n", err.unwrap());
        }
    }
    let empty_witness: BlockWitness<Bn256> = BlockWitness {
            root: None,
            proof: vec![None; TREE_DEPTH as usize]
        };

    let instance_for_generation = NonInclusion {
        params: params,
        number_of_blocks: NUMBER_OF_BLOCKS_TO_PROVE as usize,
        leaf_hash_length: 256,
        tree_depth: TREE_DEPTH as usize,
        interval_length: None,
        index: None,
        witness: vec![empty_witness; NUMBER_OF_BLOCKS_TO_PROVE as usize],
    };

    println!("generating setup...");
    let start = PreciseTime::now();
    let circuit_params = generate_random_parameters(instance_for_generation, rng).unwrap();
    println!("setup generated in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0);

    let instance_for_proof = NonInclusion {
        params: params,
        number_of_blocks: NUMBER_OF_BLOCKS_TO_PROVE as usize,
        leaf_hash_length: 256,
        tree_depth: TREE_DEPTH as usize,
        interval_length: Some(interval_length),
        index: Some(index_as_field_element),
        witness: witnesses.clone(),
    };

    let pvk = prepare_verifying_key(&circuit_params.vk);

    println!("creating proof...");
    let start = PreciseTime::now();
    let proof = create_random_proof(instance_for_proof, &circuit_params, rng).unwrap();
    println!("proof created in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0);

    let mut public_inputs = vec![];
    public_inputs.push(index_as_field_element);
    public_inputs.push(interval_length);
    public_inputs.extend(witnesses.into_iter().map(|e| e.root.clone().unwrap()));

    let success = verify_proof(&pvk, &proof, &public_inputs).unwrap();
    assert!(success);
    println!("Proof is valid");
}