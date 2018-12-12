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

pub trait PowersOfTauParameters {
    const REQUIRED_POWER: usize; 
    
    const G1_UNCOMPRESSED_BYTE_SIZE: usize;
    const G2_UNCOMPRESSED_BYTE_SIZE: usize;
    const G1_COMPRESSED_BYTE_SIZE: usize;
    const G2_COMPRESSED_BYTE_SIZE: usize;

    const TAU_POWERS_LENGTH: usize = (1 << Self::REQUIRED_POWER);

    const TAU_POWERS_G1_LENGTH: usize = (Self::TAU_POWERS_LENGTH << 1) - 1;

    const ACCUMULATOR_BYTE_SIZE: usize = (Self::TAU_POWERS_G1_LENGTH * Self::G1_UNCOMPRESSED_BYTE_SIZE) + // g1 tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G2_UNCOMPRESSED_BYTE_SIZE) + // g2 tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G1_UNCOMPRESSED_BYTE_SIZE) + // alpha tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G1_UNCOMPRESSED_BYTE_SIZE) // beta tau powers
                                            + Self::G2_UNCOMPRESSED_BYTE_SIZE // beta in g2
                                            + 64; // blake2b hash of previous contribution

    const PUBLIC_KEY_SIZE: usize = 3 * Self::G2_UNCOMPRESSED_BYTE_SIZE + // tau, alpha, and beta in g2
                                    6 * Self::G1_UNCOMPRESSED_BYTE_SIZE; // (s1, s1*tau), (s2, s2*alpha), (s3, s3*beta) in g1

    const CONTRIBUTION_BYTE_SIZE: usize = (Self::TAU_POWERS_G1_LENGTH * Self::G1_COMPRESSED_BYTE_SIZE) + // g1 tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G2_COMPRESSED_BYTE_SIZE) + // g2 tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G1_COMPRESSED_BYTE_SIZE) + // alpha tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G1_COMPRESSED_BYTE_SIZE) // beta tau powers
                                            + Self::G2_COMPRESSED_BYTE_SIZE // beta in g2
                                            + 64 // blake2b hash of input accumulator
                                            + Self::PUBLIC_KEY_SIZE; // public key
}

#[derive(Clone)]
pub struct Bn256CeremonyParameters {

}

impl PowersOfTauParameters for Bn256CeremonyParameters {
    const REQUIRED_POWER: usize = 22; // generate to have roughly 64 million constraints

    // This ceremony is based on the BN256 elliptic curve construction.
    const G1_UNCOMPRESSED_BYTE_SIZE: usize = 64;
    const G2_UNCOMPRESSED_BYTE_SIZE: usize = 128;
    const G1_COMPRESSED_BYTE_SIZE: usize = 32;
    const G2_COMPRESSED_BYTE_SIZE: usize = 64;

    // /// The accumulator supports circuits with 2^21 multiplication gates.
    // const TAU_POWERS_LENGTH: usize = (1 << REQUIRED_POWER);

    // /// More tau powers are needed in G1 because the Groth16 H query
    // /// includes terms of the form tau^i * (tau^m - 1) = tau^(i+m) - tau^i
    // /// where the largest i = m - 2, requiring the computation of tau^(2m - 2)
    // /// and thus giving us a vector length of 2^22 - 1.
    // const TAU_POWERS_G1_LENGTH: usize = (TAU_POWERS_LENGTH << 1) - 1;

    // /// The size of the accumulator on disk.
    // pub const ACCUMULATOR_BYTE_SIZE: usize = (TAU_POWERS_G1_LENGTH * G1_UNCOMPRESSED_BYTE_SIZE) + // g1 tau powers
    //                                         (TAU_POWERS_LENGTH * G2_UNCOMPRESSED_BYTE_SIZE) + // g2 tau powers
    //                                         (TAU_POWERS_LENGTH * G1_UNCOMPRESSED_BYTE_SIZE) + // alpha tau powers
    //                                         (TAU_POWERS_LENGTH * G1_UNCOMPRESSED_BYTE_SIZE) // beta tau powers
    //                                         + G2_UNCOMPRESSED_BYTE_SIZE // beta in g2
    //                                         + 64; // blake2b hash of previous contribution

    // /// The "public key" is used to verify a contribution was correctly
    // /// computed.
    // pub const PUBLIC_KEY_SIZE: usize = 3 * G2_UNCOMPRESSED_BYTE_SIZE + // tau, alpha, and beta in g2
    //                                 6 * G1_UNCOMPRESSED_BYTE_SIZE; // (s1, s1*tau), (s2, s2*alpha), (s3, s3*beta) in g1

    // /// The size of the contribution on disk.
    // pub const CONTRIBUTION_BYTE_SIZE: usize = (TAU_POWERS_G1_LENGTH * G1_COMPRESSED_BYTE_SIZE) + // g1 tau powers
    //                                         (TAU_POWERS_LENGTH * G2_COMPRESSED_BYTE_SIZE) + // g2 tau powers
    //                                         (TAU_POWERS_LENGTH * G1_COMPRESSED_BYTE_SIZE) + // alpha tau powers
    //                                         (TAU_POWERS_LENGTH * G1_COMPRESSED_BYTE_SIZE) // beta tau powers
    //                                         + G2_COMPRESSED_BYTE_SIZE // beta in g2
    //                                         + 64 // blake2b hash of input accumulator
    //                                         + PUBLIC_KEY_SIZE; // public key

}

// const REQUIRED_POWER: usize = 26; // generate to have roughly 64 million constraints

// // This ceremony is based on the BN256 elliptic curve construction.
// const G1_UNCOMPRESSED_BYTE_SIZE: usize = 64;
// const G2_UNCOMPRESSED_BYTE_SIZE: usize = 128;
// const G1_COMPRESSED_BYTE_SIZE: usize = 32;
// const G2_COMPRESSED_BYTE_SIZE: usize = 64;

// /// The accumulator supports circuits with 2^21 multiplication gates.
// const TAU_POWERS_LENGTH: usize = (1 << REQUIRED_POWER);

// /// More tau powers are needed in G1 because the Groth16 H query
// /// includes terms of the form tau^i * (tau^m - 1) = tau^(i+m) - tau^i
// /// where the largest i = m - 2, requiring the computation of tau^(2m - 2)
// /// and thus giving us a vector length of 2^22 - 1.
// const TAU_POWERS_G1_LENGTH: usize = (TAU_POWERS_LENGTH << 1) - 1;

// /// The size of the accumulator on disk.
// pub const ACCUMULATOR_BYTE_SIZE: usize = (TAU_POWERS_G1_LENGTH * G1_UNCOMPRESSED_BYTE_SIZE) + // g1 tau powers
//                                          (TAU_POWERS_LENGTH * G2_UNCOMPRESSED_BYTE_SIZE) + // g2 tau powers
//                                          (TAU_POWERS_LENGTH * G1_UNCOMPRESSED_BYTE_SIZE) + // alpha tau powers
//                                          (TAU_POWERS_LENGTH * G1_UNCOMPRESSED_BYTE_SIZE) // beta tau powers
//                                          + G2_UNCOMPRESSED_BYTE_SIZE // beta in g2
//                                          + 64; // blake2b hash of previous contribution

// /// The "public key" is used to verify a contribution was correctly
// /// computed.
// pub const PUBLIC_KEY_SIZE: usize = 3 * G2_UNCOMPRESSED_BYTE_SIZE + // tau, alpha, and beta in g2
//                                    6 * G1_UNCOMPRESSED_BYTE_SIZE; // (s1, s1*tau), (s2, s2*alpha), (s3, s3*beta) in g1

// /// The size of the contribution on disk.
// pub const CONTRIBUTION_BYTE_SIZE: usize = (TAU_POWERS_G1_LENGTH * G1_COMPRESSED_BYTE_SIZE) + // g1 tau powers
//                                           (TAU_POWERS_LENGTH * G2_COMPRESSED_BYTE_SIZE) + // g2 tau powers
//                                           (TAU_POWERS_LENGTH * G1_COMPRESSED_BYTE_SIZE) + // alpha tau powers
//                                           (TAU_POWERS_LENGTH * G1_COMPRESSED_BYTE_SIZE) // beta tau powers
//                                           + G2_COMPRESSED_BYTE_SIZE // beta in g2
//                                           + 64 // blake2b hash of input accumulator
//                                           + PUBLIC_KEY_SIZE; // public key



/// Hashes to G2 using the first 32 bytes of `digest`. Panics if `digest` is less
/// than 32 bytes.
fn hash_to_g2<E:Engine>(mut digest: &[u8]) -> E::G2
{
    assert!(digest.len() >= 32);

    let mut seed = Vec::with_capacity(8);

    for _ in 0..8 {
        seed.push(digest.read_u32::<BigEndian>().expect("assertion above guarantees this to work"));
    }

    ChaChaRng::from_seed(&seed).gen()
}

#[test]
fn test_hash_to_g2() {
    assert!(
        hash_to_g2::<Bn256>(&[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33])
        ==
        hash_to_g2::<Bn256>(&[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,34])
    );

    assert!(
        hash_to_g2::<Bn256>(&[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32])
        !=
        hash_to_g2::<Bn256>(&[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,33])
    );
}

/// Contains terms of the form (s<sub>1</sub>, s<sub>1</sub><sup>x</sup>, H(s<sub>1</sub><sup>x</sup>)<sub>2</sub>, H(s<sub>1</sub><sup>x</sup>)<sub>2</sub><sup>x</sup>)
/// for all x in τ, α and β, and some s chosen randomly by its creator. The function H "hashes into" the group G2. No points in the public key may be the identity.
///
/// The elements in G2 are used to verify transformations of the accumulator. By its nature, the public key proves
/// knowledge of τ, α and β.
///
/// It is necessary to verify `same_ratio`((s<sub>1</sub>, s<sub>1</sub><sup>x</sup>), (H(s<sub>1</sub><sup>x</sup>)<sub>2</sub>, H(s<sub>1</sub><sup>x</sup>)<sub>2</sub><sup>x</sup>)).
#[derive(Eq)]
pub struct PublicKey<E: Engine> {
    tau_g1: (E::G1Affine, E::G1Affine),
    alpha_g1: (E::G1Affine, E::G1Affine),
    beta_g1: (E::G1Affine, E::G1Affine),
    tau_g2: E::G2Affine,
    alpha_g2: E::G2Affine,
    beta_g2: E::G2Affine
}

impl<E: Engine> PartialEq for PublicKey<E> {
    fn eq(&self, other: &PublicKey<E>) -> bool {
        self.tau_g1.0 == other.tau_g1.0 &&
        self.tau_g1.1 == other.tau_g1.1 &&
        self.alpha_g1.0 == other.alpha_g1.0 &&
        self.alpha_g1.1 == other.alpha_g1.1 &&
        self.beta_g1.0 == other.beta_g1.0 &&
        self.beta_g1.1 == other.beta_g1.1 &&
        self.tau_g2 == other.tau_g2 &&
        self.alpha_g2 == other.alpha_g2 &&
        self.beta_g2 == other.beta_g2
    }
}

/// Contains the secrets τ, α and β that the participant of the ceremony must destroy.
pub struct PrivateKey<E: Engine> {
    tau: E::Fr,
    alpha: E::Fr,
    beta: E::Fr
}

/// Constructs a keypair given an RNG and a 64-byte transcript `digest`.
pub fn keypair<R: Rng, E: Engine>(rng: &mut R, digest: &[u8]) -> (PublicKey<E>, PrivateKey<E>)
{
    assert_eq!(digest.len(), 64);

    let tau = E::Fr::rand(rng);
    let alpha = E::Fr::rand(rng);
    let beta = E::Fr::rand(rng);

    let mut op = |x: E::Fr, personalization: u8| {
        // Sample random g^s
        let g1_s = E::G1::rand(rng).into_affine();
        // Compute g^{s*x}
        let g1_s_x = g1_s.mul(x).into_affine();
        // Compute BLAKE2b(personalization | transcript | g^s | g^{s*x})
        let h: generic_array::GenericArray<u8, U64> = {
            let mut h = Blake2b::default();
            h.input(&[personalization]);
            h.input(digest);
            h.input(g1_s.into_uncompressed().as_ref());
            h.input(g1_s_x.into_uncompressed().as_ref());
            h.result()
        };
        // Hash into G2 as g^{s'}
        let g2_s: E::G2Affine = hash_to_g2::<E>(h.as_ref()).into_affine();
        // Compute g^{s'*x}
        let g2_s_x = g2_s.mul(x).into_affine();

        ((g1_s, g1_s_x), g2_s_x)
    };

    let pk_tau = op(tau, 0);
    let pk_alpha = op(alpha, 1);
    let pk_beta = op(beta, 2);

    (
        PublicKey {
            tau_g1: pk_tau.0,
            alpha_g1: pk_alpha.0,
            beta_g1: pk_beta.0,
            tau_g2: pk_tau.1,
            alpha_g2: pk_alpha.1,
            beta_g2: pk_beta.1,
        },
        PrivateKey {
            tau: tau,
            alpha: alpha,
            beta: beta
        }
    )
}

/// Determines if point compression should be used.
#[derive(Copy, Clone)]
pub enum UseCompression {
    Yes,
    No
}

/// Determines if points should be checked for correctness during deserialization.
/// This is not necessary for participants, because a transcript verifier can
/// check this theirself.
#[derive(Copy, Clone)]
pub enum CheckForCorrectness {
    Yes,
    No
}

fn write_point<W, G>(
    writer: &mut W,
    p: &G,
    compression: UseCompression
) -> io::Result<()>
    where W: Write,
          G: CurveAffine
{
    match compression {
        UseCompression::Yes => writer.write_all(p.into_compressed().as_ref()),
        UseCompression::No => writer.write_all(p.into_uncompressed().as_ref()),
    }
}

/// Errors that might occur during deserialization.
#[derive(Debug)]
pub enum DeserializationError {
    IoError(io::Error),
    DecodingError(GroupDecodingError),
    PointAtInfinity
}

impl fmt::Display for DeserializationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DeserializationError::IoError(ref e) => write!(f, "Disk IO error: {}", e),
            DeserializationError::DecodingError(ref e) => write!(f, "Decoding error: {}", e),
            DeserializationError::PointAtInfinity => write!(f, "Point at infinity found")
        }
    }
}

impl From<io::Error> for DeserializationError {
    fn from(err: io::Error) -> DeserializationError {
        DeserializationError::IoError(err)
    }
}

impl From<GroupDecodingError> for DeserializationError {
    fn from(err: GroupDecodingError) -> DeserializationError {
        DeserializationError::DecodingError(err)
    }
}

impl<E: Engine> PublicKey<E> {
    /// Serialize the public key. Points are always in uncompressed form.
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()>
    {
        write_point(writer, &self.tau_g1.0, UseCompression::No)?;
        write_point(writer, &self.tau_g1.1, UseCompression::No)?;

        write_point(writer, &self.alpha_g1.0, UseCompression::No)?;
        write_point(writer, &self.alpha_g1.1, UseCompression::No)?;

        write_point(writer, &self.beta_g1.0, UseCompression::No)?;
        write_point(writer, &self.beta_g1.1, UseCompression::No)?;

        write_point(writer, &self.tau_g2, UseCompression::No)?;
        write_point(writer, &self.alpha_g2, UseCompression::No)?;
        write_point(writer, &self.beta_g2, UseCompression::No)?;

        Ok(())
    }

    /// Deserialize the public key. Points are always in uncompressed form, and
    /// always checked, since there aren't very many of them. Does not allow any
    /// points at infinity.
    pub fn deserialize<R: Read>(reader: &mut R) -> Result<PublicKey<E>, DeserializationError>
    {
        fn read_uncompressed<EE: Engine, C: CurveAffine<Engine = EE, Scalar = EE::Fr>, R: Read>(reader: &mut R) -> Result<C, DeserializationError> {
            let mut repr = C::Uncompressed::empty();
            reader.read_exact(repr.as_mut())?;
            let v = repr.into_affine()?;

            if v.is_zero() {
                Err(DeserializationError::PointAtInfinity)
            } else {
                Ok(v)
            }
        }

        let tau_g1_s = read_uncompressed::<E, _, _>(reader)?;
        let tau_g1_s_tau = read_uncompressed::<E, _, _>(reader)?;

        let alpha_g1_s = read_uncompressed::<E, _, _>(reader)?;
        let alpha_g1_s_alpha = read_uncompressed::<E, _, _>(reader)?;

        let beta_g1_s = read_uncompressed::<E, _, _>(reader)?;
        let beta_g1_s_beta = read_uncompressed::<E, _, _>(reader)?;

        let tau_g2 = read_uncompressed::<E, _, _>(reader)?;
        let alpha_g2 = read_uncompressed::<E, _, _>(reader)?;
        let beta_g2 = read_uncompressed::<E, _, _>(reader)?;

        Ok(PublicKey {
            tau_g1: (tau_g1_s, tau_g1_s_tau),
            alpha_g1: (alpha_g1_s, alpha_g1_s_alpha),
            beta_g1: (beta_g1_s, beta_g1_s_beta),
            tau_g2: tau_g2,
            alpha_g2: alpha_g2,
            beta_g2: beta_g2
        })
    }
}

#[test]
fn test_pubkey_serialization() {
    use self::rand::thread_rng;
    
    let rng = &mut thread_rng();
    let digest = (0..64).map(|_| rng.gen()).collect::<Vec<_>>();
    let (pk, _) = keypair::<_, Bn256>(rng, &digest);
    let mut v = vec![];
    pk.serialize(&mut v).unwrap();
    assert_eq!(v.len(), Bn256CeremonyParameters::PUBLIC_KEY_SIZE);
    let deserialized = PublicKey::<Bn256>::deserialize(&mut &v[..]).unwrap();
    assert!(pk == deserialized);
}

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

/// Computes a random linear combination over v1/v2.
///
/// Checking that many pairs of elements are exponentiated by
/// the same `x` can be achieved (with high probability) with
/// the following technique:
///
/// Given v1 = [a, b, c] and v2 = [as, bs, cs], compute
/// (a*r1 + b*r2 + c*r3, (as)*r1 + (bs)*r2 + (cs)*r3) for some
/// random r1, r2, r3. Given (g, g^s)...
///
/// e(g, (as)*r1 + (bs)*r2 + (cs)*r3) = e(g^s, a*r1 + b*r2 + c*r3)
///
/// ... with high probability.
fn merge_pairs<E: Engine, G: CurveAffine<Engine = E, Scalar = E::Fr>>(v1: &[G], v2: &[G]) -> (G, G)
{
    use std::sync::{Arc, Mutex};
    use self::rand::{thread_rng};

    assert_eq!(v1.len(), v2.len());

    let chunk = (v1.len() / num_cpus::get()) + 1;

    let s = Arc::new(Mutex::new(G::Projective::zero()));
    let sx = Arc::new(Mutex::new(G::Projective::zero()));

    crossbeam::scope(|scope| {
        for (v1, v2) in v1.chunks(chunk).zip(v2.chunks(chunk)) {
            let s = s.clone();
            let sx = sx.clone();

            scope.spawn(move || {
                // We do not need to be overly cautious of the RNG
                // used for this check.
                let rng = &mut thread_rng();

                let mut wnaf = Wnaf::new();
                let mut local_s = G::Projective::zero();
                let mut local_sx = G::Projective::zero();

                for (v1, v2) in v1.iter().zip(v2.iter()) {
                    let rho = G::Scalar::rand(rng);
                    let mut wnaf = wnaf.scalar(rho.into_repr());
                    let v1 = wnaf.base(v1.into_projective());
                    let v2 = wnaf.base(v2.into_projective());

                    local_s.add_assign(&v1);
                    local_sx.add_assign(&v2);
                }

                s.lock().unwrap().add_assign(&local_s);
                sx.lock().unwrap().add_assign(&local_sx);
            });
        }
    });

    let s = s.lock().unwrap().into_affine();
    let sx = sx.lock().unwrap().into_affine();

    (s, sx)
}

/// Construct a single pair (s, s^x) for a vector of
/// the form [1, x, x^2, x^3, ...].
fn power_pairs<E: Engine, G: CurveAffine<Engine = E, Scalar = E::Fr>>(v: &[G]) -> (G, G)
{
    merge_pairs::<E, _>(&v[0..(v.len()-1)], &v[1..])
}

#[test]
fn test_power_pairs() {
    use self::rand::thread_rng;
    use self::pairing::bn256::{Fr, G1Affine, G2Affine};
    let rng = &mut thread_rng();

    let mut v = vec![];
    let x = Fr::rand(rng);
    let mut acc = Fr::one();
    for _ in 0..100 {
        v.push(G1Affine::one().mul(acc).into_affine());
        acc.mul_assign(&x);
    }

    let gx = G2Affine::one().mul(x).into_affine();

    assert!(same_ratio(power_pairs(&v), (G2Affine::one(), gx)));

    v[1] = v[1].mul(Fr::rand(rng)).into_affine();

    assert!(!same_ratio(power_pairs(&v), (G2Affine::one(), gx)));
}

/// Checks if pairs have the same ratio.
fn same_ratio<E: Engine, G1: CurveAffine<Engine = E, Scalar = E::Fr>>(
    g1: (G1, G1),
    g2: (G1::Pair, G1::Pair)
) -> bool
{
    g1.0.pairing_with(&g2.1) == g1.1.pairing_with(&g2.0)
}

#[test]
fn test_same_ratio() {
    use self::rand::thread_rng;
    use self::pairing::bn256::{Fr, G1Affine, G2Affine};

    let rng = &mut thread_rng();

    let s = Fr::rand(rng);
    let g1 = G1Affine::one();
    let g2 = G2Affine::one();
    let g1_s = g1.mul(s).into_affine();
    let g2_s = g2.mul(s).into_affine();

    assert!(same_ratio((g1, g1_s), (g2, g2_s)));
    assert!(!same_ratio((g1_s, g1), (g2, g2_s)));
}

#[test]
fn test_accumulator_serialization() {
    use self::rand::thread_rng;
    use self::pairing::bn256::{Bn256, Fr, G1Affine, G2Affine};
    use self::PowersOfTauParameters;

    let rng = &mut thread_rng();
    let mut digest = (0..64).map(|_| rng.gen()).collect::<Vec<_>>();
    let params = Bn256CeremonyParameters{};
    let mut acc = Accumulator::<Bn256, _>::new(params.clone());
    let before = acc.clone();
    let (pk, sk) = keypair::<_, Bn256>(rng, &digest);
    acc.transform(&sk);
    assert!(verify_transform(&before, &acc, &pk, &digest));
    digest[0] = !digest[0];
    assert!(!verify_transform(&before, &acc, &pk, &digest));
    let mut v = Vec::with_capacity(Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE - 64);
    acc.serialize(&mut v, UseCompression::No).unwrap();
    assert_eq!(v.len(), Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE - 64);
    let deserialized = Accumulator::deserialize(&mut &v[..], UseCompression::No, CheckForCorrectness::No, params).unwrap();
    assert!(acc == deserialized);
}

/// Compute BLAKE2b("")
pub fn blank_hash() -> GenericArray<u8, U64> {
    Blake2b::new().result()
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
