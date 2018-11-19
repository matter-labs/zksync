extern crate sapling_crypto;
extern crate bellman;
extern crate rand;
extern crate pairing;

use std::time::{Duration, Instant};
use sapling_crypto::jubjub::{
    JubjubBls12,
    edwards,
    fs,
};
use sapling_crypto::circuit::sapling::{
    Spend
};
use sapling_crypto::primitives::{
    Diversifier,
    ProofGenerationKey,
    ValueCommitment
};
use bellman::groth16::*;
use rand::{XorShiftRng, SeedableRng, Rng};
use pairing::bls12_381::{Bls12, Fr};

const TREE_DEPTH: usize = 32;

fn main() {
    let jubjub_params = &JubjubBls12::new();
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    println!("Creating sample parameters...");
    let groth_params = generate_random_parameters::<Bls12, _, _>(
        Spend {
            params: jubjub_params,
            value_commitment: None,
            proof_generation_key: None,
            payment_address: None,
            commitment_randomness: None,
            ar: None,
            auth_path: vec![None; TREE_DEPTH],
            anchor: None
        },
        rng
    ).unwrap();

    const SAMPLES: u32 = 50;

    let mut total_time = Duration::new(0, 0);
    for _ in 0..SAMPLES {
        let value_commitment = ValueCommitment {
            value: 1,
            randomness: rng.gen()
        };

        let nsk: fs::Fs = rng.gen();
        let ak = edwards::Point::rand(rng, jubjub_params).mul_by_cofactor(jubjub_params);

        let proof_generation_key = ProofGenerationKey {
            ak: ak.clone(),
            nsk: nsk.clone()
        };

        let viewing_key = proof_generation_key.into_viewing_key(jubjub_params);

        let payment_address;

        loop {
            let diversifier = Diversifier(rng.gen());

            if let Some(p) = viewing_key.into_payment_address(
                diversifier,
                jubjub_params
            )
            {
                payment_address = p;
                break;
            }
        }

        let commitment_randomness: fs::Fs = rng.gen();
        let auth_path = vec![Some((rng.gen(), rng.gen())); TREE_DEPTH];
        let ar: fs::Fs = rng.gen();
        let anchor: Fr = rng.gen();

        let start = Instant::now();
        let _ = create_random_proof(Spend {
            params: jubjub_params,
            value_commitment: Some(value_commitment),
            proof_generation_key: Some(proof_generation_key),
            payment_address: Some(payment_address),
            commitment_randomness: Some(commitment_randomness),
            ar: Some(ar),
            auth_path: auth_path,
            anchor: Some(anchor)
        }, &groth_params, rng).unwrap();
        total_time += start.elapsed();
    }
    let avg = total_time / SAMPLES;
    let avg = avg.subsec_nanos() as f64 / 1_000_000_000f64
              + (avg.as_secs() as f64);

    println!("Average proving time (in seconds): {}", avg);
}
