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

use super::parameters::*;

/// Hashes to G2 using the first 32 bytes of `digest`. Panics if `digest` is less
/// than 32 bytes.
pub fn hash_to_g2<E:Engine>(mut digest: &[u8]) -> E::G2
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
pub fn power_pairs<E: Engine, G: CurveAffine<Engine = E, Scalar = E::Fr>>(v: &[G]) -> (G, G)
{
    merge_pairs::<E, _>(&v[0..(v.len()-1)], &v[1..])
}

/// Compute BLAKE2b("")
pub fn blank_hash() -> GenericArray<u8, U64> {
    Blake2b::new().result()
}

/// Checks if pairs have the same ratio.
/// Under the hood uses pairing to check
/// x1/x2 = y1/y2 => x1*y2 = x2*y1
pub fn same_ratio<E: Engine, G1: CurveAffine<Engine = E, Scalar = E::Fr>>(
    g1: (G1, G1),
    g2: (G1::Pair, G1::Pair)
) -> bool
{
    g1.0.pairing_with(&g2.1) == g1.1.pairing_with(&g2.0)
}

pub fn write_point<W, G>(
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

pub fn compute_g2_s<E: Engine> (
    digest: &[u8],
    g1_s: &E::G1Affine,
    g1_s_x: &E::G1Affine, 
    personalization: u8
) -> E::G2Affine  
{
    let mut h = Blake2b::default();
    h.input(&[personalization]);
    h.input(digest);
    h.input(g1_s.into_uncompressed().as_ref());
    h.input(g1_s_x.into_uncompressed().as_ref());

    hash_to_g2::<E>(h.result().as_ref()).into_affine()
}