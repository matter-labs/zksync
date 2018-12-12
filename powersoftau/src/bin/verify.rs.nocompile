extern crate pairing;
extern crate powersoftau;
extern crate rand;
extern crate blake2;
extern crate byteorder;
extern crate bellman;

use pairing::{CurveAffine, CurveProjective};
use pairing::bls12_381::{G1, G2};
use powersoftau::*;

use bellman::multicore::Worker;
use bellman::domain::{EvaluationDomain, Point};

use std::fs::OpenOptions;
use std::io::{self, BufReader, BufWriter, Write};

fn into_hex(h: &[u8]) -> String {
    let mut f = String::new();

    for byte in &h[..] {
        f += &format!("{:02x}", byte);
    }

    f
}

// Computes the hash of the challenge file for the player,
// given the current state of the accumulator and the last
// response file hash.
fn get_challenge_file_hash(
    acc: &Accumulator,
    last_response_file_hash: &[u8; 64]
) -> [u8; 64]
{
    let sink = io::sink();
    let mut sink = HashWriter::new(sink);

    sink.write_all(last_response_file_hash)
        .unwrap();

    acc.serialize(
        &mut sink,
        UseCompression::No
    ).unwrap();

    let mut tmp = [0; 64];
    tmp.copy_from_slice(sink.into_hash().as_slice());

    tmp
}

// Computes the hash of the response file, given the new
// accumulator, the player's public key, and the challenge
// file's hash.
fn get_response_file_hash(
    acc: &Accumulator,
    pubkey: &PublicKey,
    last_challenge_file_hash: &[u8; 64]
) -> [u8; 64]
{
    let sink = io::sink();
    let mut sink = HashWriter::new(sink);

    sink.write_all(last_challenge_file_hash)
        .unwrap();

    acc.serialize(
        &mut sink,
        UseCompression::Yes
    ).unwrap();

    pubkey.serialize(&mut sink).unwrap();

    let mut tmp = [0; 64];
    tmp.copy_from_slice(sink.into_hash().as_slice());

    tmp
}

fn main() {
    // Try to load `./transcript` from disk.
    let reader = OpenOptions::new()
                            .read(true)
                            .open("transcript")
                            .expect("unable open `./transcript` in this directory");

    let mut reader = BufReader::with_capacity(1024 * 1024, reader);

    // Initialize the accumulator
    let mut current_accumulator = Accumulator::new();

    // The "last response file hash" is just a blank BLAKE2b hash
    // at the beginning of the hash chain.
    let mut last_response_file_hash = [0; 64];
    last_response_file_hash.copy_from_slice(blank_hash().as_slice());

    // There were 89 rounds.
    for _ in 0..89 {
        // Compute the hash of the challenge file that the player
        // should have received.
        let last_challenge_file_hash = get_challenge_file_hash(
            &current_accumulator,
            &last_response_file_hash
        );

        // Deserialize the accumulator provided by the player in
        // their response file. It's stored in the transcript in
        // uncompressed form so that we can more efficiently
        // deserialize it.
        let response_file_accumulator = Accumulator::deserialize(
            &mut reader,
            UseCompression::No,
            CheckForCorrectness::Yes
        ).expect("unable to read uncompressed accumulator");

        // Deserialize the public key provided by the player.
        let response_file_pubkey = PublicKey::deserialize(&mut reader)
            .expect("wasn't able to deserialize the response file's public key");

        // Compute the hash of the response file. (we had it in uncompressed
        // form in the transcript, but the response file is compressed to save
        // participants bandwidth.)
        last_response_file_hash = get_response_file_hash(
            &response_file_accumulator,
            &response_file_pubkey,
            &last_challenge_file_hash
        );

        print!("{}", into_hex(&last_response_file_hash));

        // Verify the transformation from the previous accumulator to the new
        // one. This also verifies the correctness of the accumulators and the
        // public keys, with respect to the transcript so far.
        if !verify_transform(
            &current_accumulator,
            &response_file_accumulator,
            &response_file_pubkey,
            &last_challenge_file_hash
        )
        {
            println!(" ... FAILED");
            panic!("INVALID RESPONSE FILE!");
        } else {
            println!("");
        }

        current_accumulator = response_file_accumulator;
    }

    println!("Transcript OK!");

    let worker = &Worker::new();

    // Create the parameters for various 2^m circuit depths.
    for m in 0..22 {
        let paramname = format!("phase1radix2m{}", m);
        println!("Creating {}", paramname);

        let degree = 1 << m;

        let mut g1_coeffs = EvaluationDomain::from_coeffs(
            current_accumulator.tau_powers_g1[0..degree].iter()
                .map(|e| Point(e.into_projective()))
                .collect()
        ).unwrap();

        let mut g2_coeffs = EvaluationDomain::from_coeffs(
            current_accumulator.tau_powers_g2[0..degree].iter()
                .map(|e| Point(e.into_projective()))
                .collect()
        ).unwrap();

        let mut g1_alpha_coeffs = EvaluationDomain::from_coeffs(
            current_accumulator.alpha_tau_powers_g1[0..degree].iter()
                .map(|e| Point(e.into_projective()))
                .collect()
        ).unwrap();
        
        let mut g1_beta_coeffs = EvaluationDomain::from_coeffs(
            current_accumulator.beta_tau_powers_g1[0..degree].iter()
                .map(|e| Point(e.into_projective()))
                .collect()
        ).unwrap();

        // This converts all of the elements into Lagrange coefficients
        // for later construction of interpolation polynomials
        g1_coeffs.ifft(&worker);
        g2_coeffs.ifft(&worker);
        g1_alpha_coeffs.ifft(&worker);
        g1_beta_coeffs.ifft(&worker);

        let g1_coeffs = g1_coeffs.into_coeffs();
        let g2_coeffs = g2_coeffs.into_coeffs();
        let g1_alpha_coeffs = g1_alpha_coeffs.into_coeffs();
        let g1_beta_coeffs = g1_beta_coeffs.into_coeffs();

        assert_eq!(g1_coeffs.len(), degree);
        assert_eq!(g2_coeffs.len(), degree);
        assert_eq!(g1_alpha_coeffs.len(), degree);
        assert_eq!(g1_beta_coeffs.len(), degree);

        // Remove the Point() wrappers

        let mut g1_coeffs = g1_coeffs.into_iter()
            .map(|e| e.0)
            .collect::<Vec<_>>();

        let mut g2_coeffs = g2_coeffs.into_iter()
            .map(|e| e.0)
            .collect::<Vec<_>>();

        let mut g1_alpha_coeffs = g1_alpha_coeffs.into_iter()
            .map(|e| e.0)
            .collect::<Vec<_>>();

        let mut g1_beta_coeffs = g1_beta_coeffs.into_iter()
            .map(|e| e.0)
            .collect::<Vec<_>>();

        // Batch normalize
        G1::batch_normalization(&mut g1_coeffs);
        G2::batch_normalization(&mut g2_coeffs);
        G1::batch_normalization(&mut g1_alpha_coeffs);
        G1::batch_normalization(&mut g1_beta_coeffs);

        // H query of Groth16 needs...
        // x^i * (x^m - 1) for i in 0..=(m-2) a.k.a.
        // x^(i + m) - x^i for i in 0..=(m-2)
        // for radix2 evaluation domains
        let mut h = Vec::with_capacity(degree - 1);
        for i in 0..(degree-1) {
            let mut tmp = current_accumulator.tau_powers_g1[i + degree].into_projective();
            let mut tmp2 = current_accumulator.tau_powers_g1[i].into_projective();
            tmp2.negate();
            tmp.add_assign(&tmp2);

            h.push(tmp);
        }

        // Batch normalize this as well
        G1::batch_normalization(&mut h);

        // Create the parameter file
        let writer = OpenOptions::new()
                            .read(false)
                            .write(true)
                            .create_new(true)
                            .open(paramname)
                            .expect("unable to create parameter file in this directory");

        let mut writer = BufWriter::new(writer);

        // Write alpha (in g1)
        // Needed by verifier for e(alpha, beta)
        // Needed by prover for A and C elements of proof
        writer.write_all(
            current_accumulator.alpha_tau_powers_g1[0]
                .into_uncompressed()
                .as_ref()
        ).unwrap();

        // Write beta (in g1)
        // Needed by prover for C element of proof
        writer.write_all(
            current_accumulator.beta_tau_powers_g1[0]
                .into_uncompressed()
                .as_ref()
        ).unwrap();

        // Write beta (in g2)
        // Needed by verifier for e(alpha, beta)
        // Needed by prover for B element of proof
        writer.write_all(
            current_accumulator.beta_g2
                .into_uncompressed()
                .as_ref()
        ).unwrap();

        // Lagrange coefficients in G1 (for constructing
        // LC/IC queries and precomputing polynomials for A)
        for coeff in g1_coeffs {
            // Was normalized earlier in parallel
            let coeff = coeff.into_affine();

            writer.write_all(
                coeff.into_uncompressed()
                    .as_ref()
            ).unwrap();
        }

        // Lagrange coefficients in G2 (for precomputing
        // polynomials for B)
        for coeff in g2_coeffs {
            // Was normalized earlier in parallel
            let coeff = coeff.into_affine();

            writer.write_all(
                coeff.into_uncompressed()
                    .as_ref()
            ).unwrap();
        }

        // Lagrange coefficients in G1 with alpha (for
        // LC/IC queries)
        for coeff in g1_alpha_coeffs {
            // Was normalized earlier in parallel
            let coeff = coeff.into_affine();

            writer.write_all(
                coeff.into_uncompressed()
                    .as_ref()
            ).unwrap();
        }

        // Lagrange coefficients in G1 with beta (for
        // LC/IC queries)
        for coeff in g1_beta_coeffs {
            // Was normalized earlier in parallel
            let coeff = coeff.into_affine();

            writer.write_all(
                coeff.into_uncompressed()
                    .as_ref()
            ).unwrap();
        }

        // Bases for H polynomial computation
        for coeff in h {
            // Was normalized earlier in parallel
            let coeff = coeff.into_affine();

            writer.write_all(
                coeff.into_uncompressed()
                    .as_ref()
            ).unwrap();
        }
    }
}
