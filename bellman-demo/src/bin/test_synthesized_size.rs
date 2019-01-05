extern crate bellman;
extern crate pairing;
extern crate rand;
extern crate ff;
extern crate sapling_crypto;
extern crate time;
extern crate heapsize;

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
    prepare_prover,
};

fn main() {
    let num_constraints = (1 << 28) - 1000000;

    println!("check size test for ~{} constraints", num_constraints);

    let bench_circuit = || BenchCircuit::<Bn256> {
        phantom: PhantomData,
        num_constraints: num_constraints,
    };

    let circuit = bench_circuit();

    println!("Synthesizing for {} constraints", circuit.num_constraints);

    let kind_of_circuit = prepare_prover(circuit);

    // let s = unsafe {heapsize::heap_size_of(&kind_of_circuit)};
    // println!("Heap size = {}", s);

    println!("Please measure a memory");

    use std::{thread, time};
    let sleep_period = time::Duration::from_secs(60);
    thread::sleep(sleep_period);

    if kind_of_circuit.is_err() {
        println!("Error synthesizing");
    } else {
        println!("Synthesized well");
    }

}
