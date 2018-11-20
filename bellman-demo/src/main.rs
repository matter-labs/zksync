// https://github.com/ebfull/bellman-demo

#![allow(unused_imports)]
#![allow(unused_variables)]
extern crate bellman;
extern crate pairing;
extern crate rand;

#[macro_use]
extern crate ff;

use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField};
use pairing::{Engine};

trait OptionExt<T> {
    fn grab(&self) -> Result<T, SynthesisError>;
}

impl<T: Copy> OptionExt<T> for Option<T> {
    fn grab(&self) -> Result<T, SynthesisError> {
        self.ok_or(SynthesisError::AssignmentMissing)
    }
}

struct DemoCircuit<E: Engine> {
    a: Option<E::Fr>,
    b: Option<E::Fr>,
    c: Option<E::Fr>,
}

// Implementation of our circuit.
impl<E: Engine> Circuit<E> for DemoCircuit<E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        // variables: a, b
        // public input: c
        // constraint system:
        // a * a = a
        // b * b = b
        // 2a * b = a + b - c

        let a = cs.alloc(|| "a", || self.a.grab())?;

        // a * a = a
        cs.enforce(|| "a is a boolean", |lc| lc + a, |lc| lc + a, |lc| lc + a);

        let b = cs.alloc(|| "b", || self.b.grab())?;

        // b * b = b
        cs.enforce(|| "b is a boolean", |lc| lc + b, |lc| lc + b, |lc| lc + b);

        // c = a xor b
        let c = cs.alloc_input(|| "c", || self.c.grab())?;

        // 2a * b = a + b - c
        cs.enforce(
            || "xor constraint",
            |lc| lc + (E::Fr::from_str("2").unwrap(), a),
            |lc| lc + b,
            |lc| lc + a + b - c,
        );

        Ok(())
    }
}

// Create some parameters, create a proof, and verify the proof.
fn main() {
    use pairing::bls12_381::{Bls12, Fr};
    use rand::thread_rng;
    use std::marker::PhantomData;

    use bellman::groth16::{
        create_random_proof, generate_random_parameters, prepare_verifying_key, verify_proof, Proof,
    };

    let rng = &mut thread_rng();

    let params = {
        let c = DemoCircuit::<Bls12> {
            a: None,
            b: None,
            c: None,
        };

        generate_random_parameters(c, rng).unwrap()
    };

    let pvk = prepare_verifying_key(&params.vk);

    let c = DemoCircuit {
        a: Some(Fr::one()),
        b: Some(Fr::zero()),
        c: Some(Fr::one()),
    };

    // Create a groth16 proof with our parameters.
    let proof = create_random_proof(c, &params, rng).unwrap();

    assert!(verify_proof(&pvk, &proof, &[Fr::one()]).unwrap());

    println!("success!");
}
