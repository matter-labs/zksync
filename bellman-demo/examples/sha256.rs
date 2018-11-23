extern crate bellman;
extern crate pairing;
extern crate rand;
extern crate ff;
extern crate num_bigint;
extern crate sapling_crypto;
extern crate bellman_demo;

use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{PrimeField};
use pairing::{Engine};
use pairing::bn256::{Bn256, Fr};

use sapling_crypto::circuit::sha256::{sha256};
use sapling_crypto::circuit::num::{AllocatedNum};
use sapling_crypto::circuit::multipack::{pack_into_inputs};

use num_bigint::BigUint;

struct Sha256Circuit<E: Engine> {
    preimage: Option<E::Fr>,
}

// Implementation of our circuit:
// Given a `hash`, prove that we know a 5 byte string `preimage` such that `sha256(preimage) == hash`
impl<E: Engine> Circuit<E> for Sha256Circuit<E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {

        let get_preimage = || self.preimage.ok_or(SynthesisError::AssignmentMissing);

        let preimage = AllocatedNum::alloc(cs.namespace(|| "a"), get_preimage).unwrap();
        let mut preimage_bits = preimage.into_bits_le(cs.namespace(|| "bits")).unwrap();

        // we only take the lowest 5 bytes (note that this makes using into_bits_le() inefficient,
        // because there we allocate 253 bits which we don't need; but you'd need them for a full hash)
        preimage_bits.truncate(5 * 8);

        // convert from little to big endian
        preimage_bits.reverse();

        // get sha256 bits
        let mut hash_bits = sha256(cs.namespace(|| "sha256"), &preimage_bits).unwrap();

        // convert from big to little endian
        hash_bits.reverse();

        // shorten by a few bits to fit the hash into a single field element; this doesn't significantly affect security
        hash_bits.truncate(E::Fr::CAPACITY as usize);

        // allocate hash bits as an input field element
        pack_into_inputs(cs.namespace(|| "hash"), hash_bits.as_slice()).unwrap();

        Ok(())
    }
}

fn from_big_uint_truncated(n: &BigUint) -> Fr {
    // truncate to Fr::CAPACITY bits
    let one = &BigUint::new(vec![1]);
    let mut full_capacity = BigUint::new(vec![1]);
    for _ in 1..Fr::CAPACITY {
        full_capacity <<= 1;
        full_capacity = full_capacity | one;
    }
    let truncated = n & full_capacity;

    Fr::from_str(&truncated.to_str_radix(10)).unwrap()
}

fn fr_from_str_truncated(s: &str) -> Fr {
    let parsed = BigUint::from_bytes_be(s.as_bytes());
    from_big_uint_truncated(&parsed)
}

fn fr_from_hex_truncated(hex: &str) -> Fr {
    let parsed = BigUint::parse_bytes(hex.as_bytes(), 16).unwrap();
    from_big_uint_truncated(&parsed)
}

// Create some parameters, create a proof, and verify the proof.
fn main() {
    use rand::thread_rng;

    use bellman::groth16::{
        create_random_proof, generate_random_parameters, prepare_verifying_key, verify_proof,
    };

    let rng = &mut thread_rng();

    let params = {
        let c = Sha256Circuit::<Bn256> {
            preimage: None,
        };
        generate_random_parameters(c, rng).unwrap()
    };

    let pvk = prepare_verifying_key(&params.vk);

    let c = Sha256Circuit {
        preimage: Some(fr_from_str_truncated("hello")),
    };

    let proof = create_random_proof(c, &params, rng).unwrap();

    let inputs = &[
        // not that we truncate the hash below to 253 bits, in order to fit it into one single input
        fr_from_hex_truncated("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
    ];
    let success = verify_proof(&pvk, &proof, inputs).unwrap();
    assert!(success);

    let inputs_extra = r#"assert(inputs[0] == uint(sha256("hello")) & 0x1fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);"#;
    println!("{}", bellman_demo::generate_demo_contract(&params.vk, &proof, inputs, inputs_extra));
}
