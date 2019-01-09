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
extern crate itertools;

use itertools::Itertools;
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

use super::utils::*;
use super::parameters::*;

/// Contains terms of the form (s<sub>1</sub>, s<sub>1</sub><sup>x</sup>, H(s<sub>1</sub><sup>x</sup>)<sub>2</sub>, H(s<sub>1</sub><sup>x</sup>)<sub>2</sub><sup>x</sup>)
/// for all x in τ, α and β, and some s chosen randomly by its creator. The function H "hashes into" the group G2. No points in the public key may be the identity.
///
/// The elements in G2 are used to verify transformations of the accumulator. By its nature, the public key proves
/// knowledge of τ, α and β.
///
/// It is necessary to verify `same_ratio`((s<sub>1</sub>, s<sub>1</sub><sup>x</sup>), (H(s<sub>1</sub><sup>x</sup>)<sub>2</sub>, H(s<sub>1</sub><sup>x</sup>)<sub>2</sub><sup>x</sup>)).
#[derive(Eq)]
pub struct PublicKey<E: Engine> {
    pub tau_g1: (E::G1Affine, E::G1Affine),
    pub alpha_g1: (E::G1Affine, E::G1Affine),
    pub beta_g1: (E::G1Affine, E::G1Affine),
    pub tau_g2: E::G2Affine,
    pub alpha_g2: E::G2Affine,
    pub beta_g2: E::G2Affine
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
    pub tau: E::Fr,
    pub alpha: E::Fr,
    pub beta: E::Fr
}

/// Constructs a keypair given an RNG and a 64-byte transcript `digest`.
pub fn keypair<R: Rng, E: Engine>(rng: &mut R, digest: &[u8]) -> (PublicKey<E>, PrivateKey<E>)
{
    assert_eq!(digest.len(), 64);

    // tau is a conribution to the "powers of tau", in a set of points of the form "tau^i * G"
    let tau = E::Fr::rand(rng);
    // alpha and beta are a set of conrtibuitons in a form "alpha * tau^i * G" and that are required
    // for construction of the polynomials
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

    // these "public keys" are requried for for next participants to check that points are in fact
    // sequential powers
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

impl<E: Engine> PublicKey<E> {

    /// This function is intended to write the key to the memory map and calculates
    /// a position for writing into the file itself based on information whether
    /// contribution was output in compressed on uncompressed form
    pub fn write<P>(
        &self,
        output_map: &mut MmapMut,
        accumulator_was_compressed: UseCompression
    )
    -> io::Result<()>
        where P: PowersOfTauParameters
    {
        let mut position = match accumulator_was_compressed {
            UseCompression::Yes => {
                P::CONTRIBUTION_BYTE_SIZE - P::PUBLIC_KEY_SIZE
            },
            UseCompression::No => {
                P::ACCUMULATOR_BYTE_SIZE
            }
        };

        (&mut output_map[position..]).write(&self.tau_g1.0.into_uncompressed().as_ref())?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        (&mut output_map[position..]).write(&self.tau_g1.1.into_uncompressed().as_ref())?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        (&mut output_map[position..]).write(&self.alpha_g1.0.into_uncompressed().as_ref())?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        (&mut output_map[position..]).write(&self.alpha_g1.1.into_uncompressed().as_ref())?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        (&mut output_map[position..]).write(&self.beta_g1.0.into_uncompressed().as_ref())?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        (&mut output_map[position..]).write(&self.beta_g1.1.into_uncompressed().as_ref())?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        (&mut output_map[position..]).write(&self.tau_g2.into_uncompressed().as_ref())?;
        position += P::G2_UNCOMPRESSED_BYTE_SIZE;

        (&mut output_map[position..]).write(&self.alpha_g2.into_uncompressed().as_ref())?;
        position += P::G2_UNCOMPRESSED_BYTE_SIZE;

        (&mut output_map[position..]).write(&self.beta_g2.into_uncompressed().as_ref())?;

        output_map.flush()?;

        Ok(())
    }

    /// Deserialize the public key. Points are always in uncompressed form, and
    /// always checked, since there aren't very many of them. Does not allow any
    /// points at infinity.
    pub fn read<P>(
        input_map: &Mmap,
        accumulator_was_compressed: UseCompression
    ) -> Result<Self, DeserializationError>
        where P: PowersOfTauParameters
    {
        fn read_uncompressed<EE: Engine, C: CurveAffine<Engine = EE, Scalar = EE::Fr>>(input_map: &Mmap, position: usize) -> Result<C, DeserializationError> {
            let mut repr = C::Uncompressed::empty();
            let element_size = C::Uncompressed::size();
            let memory_slice = input_map.get(position..position+element_size).expect("must read point data from file");
            memory_slice.clone().read_exact(repr.as_mut())?;
            let v = repr.into_affine()?;

            if v.is_zero() {
                Err(DeserializationError::PointAtInfinity)
            } else {
                Ok(v)
            }
        }

        let mut position = match accumulator_was_compressed {
            UseCompression::Yes => {
                P::CONTRIBUTION_BYTE_SIZE - P::PUBLIC_KEY_SIZE
            },
            UseCompression::No => {
                P::ACCUMULATOR_BYTE_SIZE
            }
        };

        let tau_g1_s = read_uncompressed::<E, _>(input_map, position)?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        let tau_g1_s_tau = read_uncompressed::<E, _>(input_map, position)?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        let alpha_g1_s = read_uncompressed::<E, _>(input_map, position)?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        let alpha_g1_s_alpha = read_uncompressed::<E, _>(input_map, position)?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        let beta_g1_s = read_uncompressed::<E, _>(input_map, position)?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        let beta_g1_s_beta = read_uncompressed::<E, _>(input_map, position)?;
        position += P::G1_UNCOMPRESSED_BYTE_SIZE;

        let tau_g2 = read_uncompressed::<E, _>(input_map, position)?;
        position += P::G2_UNCOMPRESSED_BYTE_SIZE;

        let alpha_g2 = read_uncompressed::<E, _>(input_map, position)?;
        position += P::G2_UNCOMPRESSED_BYTE_SIZE;

        let beta_g2 = read_uncompressed::<E, _>(input_map, position)?;

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
