extern crate bellman;
extern crate pairing;
extern crate rand;
extern crate ff;
extern crate sapling_crypto;
extern crate time;

use time::PreciseTime;
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use pairing::{Engine};
use pairing::bn256::Bn256;
use ff::{Field};
use sapling_crypto::circuit::sha256::{sha256};
use sapling_crypto::circuit::num::{AllocatedNum};

use std::marker::PhantomData;

struct BenchCircuit<E: Engine> {
    phantom: PhantomData<E>,
    num_constraints: usize,
}

impl<E: Engine> Circuit<E> for BenchCircuit<E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let preimage = AllocatedNum::alloc(cs.namespace(|| "a"), || Ok(E::Fr::zero())).unwrap();
        let mut cur = preimage.into_bits_le(cs.namespace(|| "bits")).unwrap();
        cur.truncate(160);
        for _ in 0..(self.num_constraints / 24_600) {
            cur = sha256(cs.namespace(|| "sha256"), &cur).unwrap();
        }
        Ok(())
    }
}

use bellman::groth16::{
    create_random_proof, generate_random_parameters, prepare_verifying_key, verify_proof,
};

use std::env;

fn main() {

    let num_constraints: usize = match env::args().nth(1) {
        Some(n) => n.parse().unwrap(),
        None => 100_000,
    };

    println!("bench test for ~{} constraints", num_constraints);

    let bench_circuit = || BenchCircuit::<Bn256> {
        phantom: PhantomData,
        num_constraints: num_constraints,
    };

    let rng = &mut rand::thread_rng();

    println!("generating setup...");
    let start = PreciseTime::now();
    let params = generate_random_parameters(bench_circuit(), rng).unwrap();
    println!("done in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0);

    let pvk = prepare_verifying_key(&params.vk);

    println!("creating proof...");
    let start = PreciseTime::now();
    let proof = create_random_proof(bench_circuit(), &params, rng).unwrap();
    println!("done in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0);

    let success = verify_proof(&pvk, &proof, &[]).unwrap();
    assert!(success);
}
