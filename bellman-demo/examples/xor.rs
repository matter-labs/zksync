extern crate bellman;
extern crate pairing;
extern crate rand;
extern crate ff;
extern crate bellman_demo;

use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField};
use pairing::{Engine};
use pairing::bn256::{Bn256, Fr};

trait OptionExt<T> {
    fn grab(&self) -> Result<T, SynthesisError>;
}

impl<T: Copy> OptionExt<T> for Option<T> {
    fn grab(&self) -> Result<T, SynthesisError> {
        self.ok_or(SynthesisError::AssignmentMissing)
    }
}

struct XorCircuit<E: Engine> {
    a: Option<E::Fr>,
    b: Option<E::Fr>,
    c: Option<E::Fr>,
}

// Implementation of our circuit:
// Given a bit `c`, prove that we know bits `a` and `b` such that `c = a xor b`
impl<E: Engine> Circuit<E> for XorCircuit<E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {

        // public input: c
        // variables (witness): a, b

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
    use rand::thread_rng;

    use bellman::groth16::{
        create_random_proof, generate_random_parameters, prepare_verifying_key, verify_proof
    };

    let rng = &mut thread_rng();

    let params = {
        let c = XorCircuit::<Bn256> {
            a: None,
            b: None,
            c: None,
        };
        generate_random_parameters(c, rng).unwrap()
    };

    let pvk = prepare_verifying_key(&params.vk);

    // here we allocate actual variables
    let c = XorCircuit {
        a: Some(Fr::one()),
        b: Some(Fr::zero()),
        c: Some(Fr::one()),
    };

    // Create a groth16 proof with our parameters.
    let proof = create_random_proof(c, &params, rng).unwrap();

    // `inputs` slice contains public parameters encoded as field elements Fr

    // incorrect input tests
    let inputs = &[Fr::from_str("5").unwrap()];
    let success = verify_proof(&pvk, &proof, inputs).unwrap();
    assert!(!success); // fails, because 5 is not 1 or 0

    let inputs = &[Fr::zero()];
    let success = verify_proof(&pvk, &proof, inputs).unwrap();
    assert!(!success); // fails because 0 != 0 xor 1

    // correct input test
    let inputs = &[Fr::one()];
    let success = verify_proof(&pvk, &proof, inputs).unwrap();
    assert!(success);
    println!("{}", bellman_demo::verifier_contract::generate_demo_contract(&params.vk, &proof, inputs, ""));
}
