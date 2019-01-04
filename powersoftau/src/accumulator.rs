//! This ceremony constructs the "powers of tau" for Jens Groth's 2016 zk-SNARK proving
//! system using the BLS12-381 pairing-friendly elliptic curve construction.
//!
//! # Overview
//!
//! Participants of the ceremony receive a "challenge" file containing:
//!
//! * the BLAKE2b hash of the last file entered into the transcript
//! * an `Accumulator` (with curve points encoded in uncompressed form for fast deserialization)
//!
//! The participant runs a tool which generates a random keypair (`PublicKey`, `PrivateKey`)
//! used for modifying the `Accumulator` from the "challenge" file. The keypair is then used to
//! transform the `Accumulator`, and a "response" file is generated containing:
//!
//! * the BLAKE2b hash of the "challenge" file (thus forming a hash chain over the entire transcript)
//! * an `Accumulator` (with curve points encoded in compressed form for fast uploading)
//! * the `PublicKey`
//!
//! This "challenge" file is entered into the protocol transcript. A given transcript is valid
//! if the transformations between consecutive `Accumulator`s verify with their respective
//! `PublicKey`s. Participants (and the public) can ensure that their contribution to the
//! `Accumulator` was accepted by ensuring the transcript contains their "response" file, ideally
//! by comparison of the BLAKE2b hash of the "response" file.
//!
//! After some time has elapsed for participants to contribute to the ceremony, a participant is
//! simulated with a randomness beacon. The resulting `Accumulator` contains partial zk-SNARK
//! public parameters for all circuits within a bounded size.

extern crate pairing;
extern crate rand;
extern crate crossbeam;
extern crate num_cpus;
extern crate blake2;
extern crate generic_array;
extern crate typenum;
extern crate byteorder;
extern crate ff;
extern crate memmap;

use memmap::{Mmap, MmapMut};
use self::ff::{Field, PrimeField};
use self::byteorder::{ReadBytesExt, BigEndian};
use self::rand::{SeedableRng, Rng, Rand};
use self::rand::chacha::ChaChaRng;
use self::pairing::bn256::{Bn256};
use self::pairing::*;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use self::generic_array::GenericArray;
use self::typenum::consts::U64;
use self::blake2::{Blake2b, Digest};
use std::fmt;

use super::keypair::*;
use super::utils::*;
use super::parameters::*;

/// The `Accumulator` is an object that participants of the ceremony contribute
/// randomness to. This object contains powers of trapdoor `tau` in G1 and in G2 over
/// fixed generators, and additionally in G1 over two other generators of exponents
/// `alpha` and `beta` over those fixed generators. In other words:
///
/// * (τ, τ<sup>2</sup>, ..., τ<sup>2<sup>22</sup> - 2</sup>, α, ατ, ατ<sup>2</sup>, ..., ατ<sup>2<sup>21</sup> - 1</sup>, β, βτ, βτ<sup>2</sup>, ..., βτ<sup>2<sup>21</sup> - 1</sup>)<sub>1</sub>
/// * (β, τ, τ<sup>2</sup>, ..., τ<sup>2<sup>21</sup> - 1</sup>)<sub>2</sub>
#[derive(Eq, Clone)]
pub struct Accumulator<E: Engine, P: PowersOfTauParameters> {
    /// tau^0, tau^1, tau^2, ..., tau^{TAU_POWERS_G1_LENGTH - 1}
    pub tau_powers_g1: Vec<E::G1Affine>,
    /// tau^0, tau^1, tau^2, ..., tau^{TAU_POWERS_LENGTH - 1}
    pub tau_powers_g2: Vec<E::G2Affine>,
    /// alpha * tau^0, alpha * tau^1, alpha * tau^2, ..., alpha * tau^{TAU_POWERS_LENGTH - 1}
    pub alpha_tau_powers_g1: Vec<E::G1Affine>,
    /// beta * tau^0, beta * tau^1, beta * tau^2, ..., beta * tau^{TAU_POWERS_LENGTH - 1}
    pub beta_tau_powers_g1: Vec<E::G1Affine>,
    /// beta
    pub beta_g2: E::G2Affine,
    /// Keep parameters here
    pub parameters: P
}

impl<E: Engine, P: PowersOfTauParameters> PartialEq for Accumulator<E, P> {
    fn eq(&self, other: &Accumulator<E, P>) -> bool {
        self.tau_powers_g1.eq(&other.tau_powers_g1) &&
        self.tau_powers_g2.eq(&other.tau_powers_g2) &&
        self.alpha_tau_powers_g1.eq(&other.alpha_tau_powers_g1) &&
        self.beta_tau_powers_g1.eq(&other.beta_tau_powers_g1) && 
        self.beta_g2 == other.beta_g2
    }
}

impl<E:Engine, P: PowersOfTauParameters> Accumulator<E, P> {
    /// Constructs an "initial" accumulator with τ = 1, α = 1, β = 1.
    pub fn new(parameters: P) -> Self {
        Accumulator {
            tau_powers_g1: vec![E::G1Affine::one(); P::TAU_POWERS_G1_LENGTH],
            tau_powers_g2: vec![E::G2Affine::one(); P::TAU_POWERS_LENGTH],
            alpha_tau_powers_g1: vec![E::G1Affine::one(); P::TAU_POWERS_LENGTH],
            beta_tau_powers_g1: vec![E::G1Affine::one(); P::TAU_POWERS_LENGTH],
            beta_g2: E::G2Affine::one(),
            parameters: parameters
        }
    }

    /// Write the accumulator with some compression behavior.
    pub fn serialize<W: Write>(
        &self,
        writer: &mut W,
        compression: UseCompression
    ) -> io::Result<()>
    {
        fn write_all<W: Write, C: CurveAffine>(
            writer: &mut W,
            c: &[C],
            compression: UseCompression
        ) -> io::Result<()>
        {
            for c in c {
                write_point(writer, c, compression)?;
            }

            Ok(())
        }

        write_all(writer, &self.tau_powers_g1, compression)?;
        write_all(writer, &self.tau_powers_g2, compression)?;
        write_all(writer, &self.alpha_tau_powers_g1, compression)?;
        write_all(writer, &self.beta_tau_powers_g1, compression)?;
        write_all(writer, &[self.beta_g2], compression)?;

        Ok(())
    }

    /// Read the accumulator from disk with some compression behavior. `checked`
    /// indicates whether we should check it's a valid element of the group and
    /// not the point at infinity.
    pub fn deserialize<R: Read>(
        reader: &mut R,
        compression: UseCompression,
        checked: CheckForCorrectness,
        parameters: P
    ) -> Result<Self, DeserializationError>
    {
        fn read_all<EE: Engine, R: Read, C: CurveAffine<Engine = EE, Scalar = EE::Fr> > (
            reader: &mut R,
            size: usize,
            compression: UseCompression,
            checked: CheckForCorrectness
        ) -> Result<Vec<C>, DeserializationError>
        {
            fn decompress_all<R: Read, ENC: EncodedPoint>(
                reader: &mut R,
                size: usize,
                checked: CheckForCorrectness
            ) -> Result<Vec<ENC::Affine>, DeserializationError>
            {
                // Read the encoded elements
                let mut res = vec![ENC::empty(); size];

                for encoded in &mut res {
                    reader.read_exact(encoded.as_mut())?;
                }

                // Allocate space for the deserialized elements
                let mut res_affine = vec![ENC::Affine::zero(); size];

                let mut chunk_size = res.len() / num_cpus::get();
                if chunk_size == 0 {
                    chunk_size = 1;
                }

                // If any of our threads encounter a deserialization/IO error, catch
                // it with this.
                let decoding_error = Arc::new(Mutex::new(None));

                crossbeam::scope(|scope| {
                    for (source, target) in res.chunks(chunk_size).zip(res_affine.chunks_mut(chunk_size)) {
                        let decoding_error = decoding_error.clone();

                        scope.spawn(move || {
                            for (source, target) in source.iter().zip(target.iter_mut()) {
                                match {
                                    // If we're a participant, we don't need to check all of the
                                    // elements in the accumulator, which saves a lot of time.
                                    // The hash chain prevents this from being a problem: the
                                    // transcript guarantees that the accumulator was properly
                                    // formed.
                                    match checked {
                                        CheckForCorrectness::Yes => {
                                            // Points at infinity are never expected in the accumulator
                                            source.into_affine().map_err(|e| e.into()).and_then(|source| {
                                                if source.is_zero() {
                                                    Err(DeserializationError::PointAtInfinity)
                                                } else {
                                                    Ok(source)
                                                }
                                            })
                                        },
                                        CheckForCorrectness::No => source.into_affine_unchecked().map_err(|e| e.into())
                                    }
                                }
                                {
                                    Ok(source) => {
                                        *target = source;
                                    },
                                    Err(e) => {
                                        *decoding_error.lock().unwrap() = Some(e);
                                    }
                                }
                            }
                        });
                    }
                });

                match Arc::try_unwrap(decoding_error).unwrap().into_inner().unwrap() {
                    Some(e) => {
                        Err(e)
                    },
                    None => {
                        Ok(res_affine)
                    }
                }
            }

            match compression {
                UseCompression::Yes => decompress_all::<_, C::Compressed>(reader, size, checked),
                UseCompression::No => decompress_all::<_, C::Uncompressed>(reader, size, checked)
            }
        }

        let tau_powers_g1 = read_all::<E, _, _>(reader, P::TAU_POWERS_G1_LENGTH, compression, checked)?;
        let tau_powers_g2 = read_all::<E, _, _>(reader, P::TAU_POWERS_LENGTH, compression, checked)?;
        let alpha_tau_powers_g1 = read_all::<E, _, _>(reader, P::TAU_POWERS_LENGTH, compression, checked)?;
        let beta_tau_powers_g1 = read_all::<E, _, _>(reader, P::TAU_POWERS_LENGTH, compression, checked)?;
        let beta_g2 = read_all::<E, _, _>(reader, 1, compression, checked)?[0];

        Ok(Accumulator {
            tau_powers_g1: tau_powers_g1,
            tau_powers_g2: tau_powers_g2,
            alpha_tau_powers_g1: alpha_tau_powers_g1,
            beta_tau_powers_g1: beta_tau_powers_g1,
            beta_g2: beta_g2,
            parameters: parameters
        })
    }

    /// Transforms the accumulator with a private key.
    pub fn transform(&mut self, key: &PrivateKey<E>)
    {
        // Construct the powers of tau
        let mut taupowers = vec![E::Fr::zero(); P::TAU_POWERS_G1_LENGTH];
        let chunk_size = P::TAU_POWERS_G1_LENGTH / num_cpus::get();

        // Construct exponents in parallel
        crossbeam::scope(|scope| {
            for (i, taupowers) in taupowers.chunks_mut(chunk_size).enumerate() {
                scope.spawn(move || {
                    let mut acc = key.tau.pow(&[(i * chunk_size) as u64]);

                    for t in taupowers {
                        *t = acc;
                        acc.mul_assign(&key.tau);
                    }
                });
            }
        });

        /// Exponentiate a large number of points, with an optional coefficient to be applied to the
        /// exponent.
        fn batch_exp<EE: Engine, C: CurveAffine<Engine = EE, Scalar = EE::Fr> >(bases: &mut [C], exp: &[C::Scalar], coeff: Option<&C::Scalar>) {
            assert_eq!(bases.len(), exp.len());
            let mut projective = vec![C::Projective::zero(); bases.len()];
            let chunk_size = bases.len() / num_cpus::get();

            // Perform wNAF over multiple cores, placing results into `projective`.
            crossbeam::scope(|scope| {
                for ((bases, exp), projective) in bases.chunks_mut(chunk_size)
                                                       .zip(exp.chunks(chunk_size))
                                                       .zip(projective.chunks_mut(chunk_size))
                {
                    scope.spawn(move || {
                        let mut wnaf = Wnaf::new();

                        for ((base, exp), projective) in bases.iter_mut()
                                                              .zip(exp.iter())
                                                              .zip(projective.iter_mut())
                        {
                            let mut exp = *exp;
                            if let Some(coeff) = coeff {
                                exp.mul_assign(coeff);
                            }

                            *projective = wnaf.base(base.into_projective(), 1).scalar(exp.into_repr());
                        }
                    });
                }
            });

            // Perform batch normalization
            crossbeam::scope(|scope| {
                for projective in projective.chunks_mut(chunk_size)
                {
                    scope.spawn(move || {
                        C::Projective::batch_normalization(projective);
                    });
                }
            });

            // Turn it all back into affine points
            for (projective, affine) in projective.iter().zip(bases.iter_mut()) {
                *affine = projective.into_affine();
            }
        }

        batch_exp::<E, _>(&mut self.tau_powers_g1, &taupowers[0..], None);
        batch_exp::<E, _>(&mut self.tau_powers_g2, &taupowers[0..P::TAU_POWERS_LENGTH], None);
        batch_exp::<E, _>(&mut self.alpha_tau_powers_g1, &taupowers[0..P::TAU_POWERS_LENGTH], Some(&key.alpha));
        batch_exp::<E, _>(&mut self.beta_tau_powers_g1, &taupowers[0..P::TAU_POWERS_LENGTH], Some(&key.beta));
        self.beta_g2 = self.beta_g2.mul(key.beta).into_affine();
    }
}

/// Verifies a transformation of the `Accumulator` with the `PublicKey`, given a 64-byte transcript `digest`.
pub fn verify_transform<E: Engine, P: PowersOfTauParameters>(before: &Accumulator<E, P>, after: &Accumulator<E, P>, key: &PublicKey<E>, digest: &[u8]) -> bool
{
    assert_eq!(digest.len(), 64);

    let compute_g2_s = |g1_s: E::G1Affine, g1_s_x: E::G1Affine, personalization: u8| {
        let mut h = Blake2b::default();
        h.input(&[personalization]);
        h.input(digest);
        h.input(g1_s.into_uncompressed().as_ref());
        h.input(g1_s_x.into_uncompressed().as_ref());
        hash_to_g2::<E>(h.result().as_ref()).into_affine()
    };

    let tau_g2_s = compute_g2_s(key.tau_g1.0, key.tau_g1.1, 0);
    let alpha_g2_s = compute_g2_s(key.alpha_g1.0, key.alpha_g1.1, 1);
    let beta_g2_s = compute_g2_s(key.beta_g1.0, key.beta_g1.1, 2);

    // Check the proofs-of-knowledge for tau/alpha/beta
    
    // g1^s / g1^(s*x) = g2^s / g2^(s*x)
    if !same_ratio(key.tau_g1, (tau_g2_s, key.tau_g2)) {
        return false;
    }
    if !same_ratio(key.alpha_g1, (alpha_g2_s, key.alpha_g2)) {
        return false;
    }
    if !same_ratio(key.beta_g1, (beta_g2_s, key.beta_g2)) {
        return false;
    }

    // Check the correctness of the generators for tau powers
    if after.tau_powers_g1[0] != E::G1Affine::one() {
        return false;
    }
    if after.tau_powers_g2[0] != E::G2Affine::one() {
        return false;
    }

    // Did the participant multiply the previous tau by the new one?
    if !same_ratio((before.tau_powers_g1[1], after.tau_powers_g1[1]), (tau_g2_s, key.tau_g2)) {
        return false;
    }

    // Did the participant multiply the previous alpha by the new one?
    if !same_ratio((before.alpha_tau_powers_g1[0], after.alpha_tau_powers_g1[0]), (alpha_g2_s, key.alpha_g2)) {
        return false;
    }

    // Did the participant multiply the previous beta by the new one?
    if !same_ratio((before.beta_tau_powers_g1[0], after.beta_tau_powers_g1[0]), (beta_g2_s, key.beta_g2)) {
        return false;
    }
    if !same_ratio((before.beta_tau_powers_g1[0], after.beta_tau_powers_g1[0]), (before.beta_g2, after.beta_g2)) {
        return false;
    }

    // Are the powers of tau correct?
    if !same_ratio(power_pairs(&after.tau_powers_g1), (after.tau_powers_g2[0], after.tau_powers_g2[1])) {
        return false;
    }
    if !same_ratio(power_pairs(&after.tau_powers_g2), (after.tau_powers_g1[0], after.tau_powers_g1[1])) {
        return false;
    }
    if !same_ratio(power_pairs(&after.alpha_tau_powers_g1), (after.tau_powers_g2[0], after.tau_powers_g2[1])) {
        return false;
    }
    if !same_ratio(power_pairs(&after.beta_tau_powers_g1), (after.tau_powers_g2[0], after.tau_powers_g2[1])) {
        return false;
    }

    true
}



/// Abstraction over a reader which hashes the data being read.
pub struct HashReader<R: Read> {
    reader: R,
    hasher: Blake2b
}

impl<R: Read> HashReader<R> {
    /// Construct a new `HashReader` given an existing `reader` by value.
    pub fn new(reader: R) -> Self {
        HashReader {
            reader: reader,
            hasher: Blake2b::default()
        }
    }

    /// Destroy this reader and return the hash of what was read.
    pub fn into_hash(self) -> GenericArray<u8, U64> {
        self.hasher.result()
    }
}

impl<R: Read> Read for HashReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes = self.reader.read(buf)?;

        if bytes > 0 {
            self.hasher.input(&buf[0..bytes]);
        }

        Ok(bytes)
    }
}

/// Abstraction over a writer which hashes the data being written.
pub struct HashWriter<W: Write> {
    writer: W,
    hasher: Blake2b
}

impl<W: Write> HashWriter<W> {
    /// Construct a new `HashWriter` given an existing `writer` by value.
    pub fn new(writer: W) -> Self {
        HashWriter {
            writer: writer,
            hasher: Blake2b::default()
        }
    }

    /// Destroy this writer and return the hash of what was written.
    pub fn into_hash(self) -> GenericArray<u8, U64> {
        self.hasher.result()
    }
}

impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes = self.writer.write(buf)?;

        if bytes > 0 {
            self.hasher.input(&buf[0..bytes]);
        }

        Ok(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
