use pairing::{
    CurveAffine,
    CurveProjective,
    Engine
};

use ff::{
    PrimeField,
    Field,
    PrimeFieldRepr,
    ScalarEngine};

use std::sync::Arc;
use std::io;
use bit_vec::{self, BitVec};
use std::iter;
use futures::{Future};
use super::multicore::Worker;

use super::SynthesisError;

/// An object that builds a source of bases.
pub trait SourceBuilder<G: CurveAffine>: Send + Sync + 'static + Clone {
    type Source: Source<G>;

    fn new(self) -> Self::Source;
}

/// A source of bases, like an iterator.
pub trait Source<G: CurveAffine> {
    /// Parses the element from the source. Fails if the point is at infinity.
    fn add_assign_mixed(&mut self, to: &mut <G as CurveAffine>::Projective) -> Result<(), SynthesisError>;

    /// Skips `amt` elements from the source, avoiding deserialization.
    fn skip(&mut self, amt: usize) -> Result<(), SynthesisError>;
}

impl<G: CurveAffine> SourceBuilder<G> for (Arc<Vec<G>>, usize) {
    type Source = (Arc<Vec<G>>, usize);

    fn new(self) -> (Arc<Vec<G>>, usize) {
        (self.0.clone(), self.1)
    }
}

impl<G: CurveAffine> Source<G> for (Arc<Vec<G>>, usize) {
    fn add_assign_mixed(&mut self, to: &mut <G as CurveAffine>::Projective) -> Result<(), SynthesisError> {
        if self.0.len() <= self.1 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "expected more bases when adding from source").into());
        }

        if self.0[self.1].is_zero() {
            return Err(SynthesisError::UnexpectedIdentity)
        }

        to.add_assign_mixed(&self.0[self.1]);

        self.1 += 1;

        Ok(())
    }

    fn skip(&mut self, amt: usize) -> Result<(), SynthesisError> {
        if self.0.len() <= self.1 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "expected more bases skipping from source").into());
        }

        self.1 += amt;

        Ok(())
    }
}

pub trait QueryDensity {
    /// Returns whether the base exists.
    type Iter: Iterator<Item=bool>;

    fn iter(self) -> Self::Iter;
    fn get_query_size(self) -> Option<usize>;
}

#[derive(Clone)]
pub struct FullDensity;

impl AsRef<FullDensity> for FullDensity {
    fn as_ref(&self) -> &FullDensity {
        self
    }
}

impl<'a> QueryDensity for &'a FullDensity {
    type Iter = iter::Repeat<bool>;

    fn iter(self) -> Self::Iter {
        iter::repeat(true)
    }

    fn get_query_size(self) -> Option<usize> {
        None
    }
}

#[derive(Clone)]
pub struct DensityTracker {
    bv: BitVec,
    total_density: usize
}

impl<'a> QueryDensity for &'a DensityTracker {
    type Iter = bit_vec::Iter<'a>;

    fn iter(self) -> Self::Iter {
        self.bv.iter()
    }

    fn get_query_size(self) -> Option<usize> {
        Some(self.bv.len())
    }
}

impl DensityTracker {
    pub fn new() -> DensityTracker {
        DensityTracker {
            bv: BitVec::new(),
            total_density: 0
        }
    }

    pub fn add_element(&mut self) {
        self.bv.push(false);
    }

    pub fn inc(&mut self, idx: usize) {
        if !self.bv.get(idx).unwrap() {
            self.bv.set(idx, true);
            self.total_density += 1;
        }
    }

    pub fn get_total_density(&self) -> usize {
        self.total_density
    }
}

fn multiexp_inner<Q, D, G, S>(
    pool: &Worker,
    bases: S,
    density_map: D,
    exponents: Arc<Vec<<G::Scalar as PrimeField>::Repr>>,
    // exponents: Arc<Vec<<<G::Engine as Engine>::Fr as PrimeField>::Repr>>,
    mut skip: u32,
    c: u32,
    handle_trivial: bool
) -> Box<Future<Item=<G as CurveAffine>::Projective, Error=SynthesisError>>
    where for<'a> &'a Q: QueryDensity,
          D: Send + Sync + 'static + Clone + AsRef<Q>,
          G: CurveAffine,
          S: SourceBuilder<G>
{
    // Perform this region of the multiexp
    let this = {
        let bases = bases.clone();
        let exponents = exponents.clone();
        let density_map = density_map.clone();

        // This looks like a Pippenger’s algorithm
        pool.compute(move || {
            // Accumulate the result
            let mut acc = G::Projective::zero();

            // Build a source for the bases
            let mut bases = bases.new();

            // Create buckets to place remainders s mod 2^c,
            // it will be 2^c - 1 buckets (no bucket for zeroes)

            // Create space for the buckets
            let mut buckets = vec![<G as CurveAffine>::Projective::zero(); (1 << c) - 1];

            let zero = <G::Engine as ScalarEngine>::Fr::zero().into_repr();
            let one = <G::Engine as ScalarEngine>::Fr::one().into_repr();

            // Sort the bases into buckets
            for (&exp, density) in exponents.iter().zip(density_map.as_ref().iter()) {
                // Go over density and exponents
                if density {
                    if exp == zero {
                        bases.skip(1)?;
                    } else if exp == one {
                        if handle_trivial {
                            bases.add_assign_mixed(&mut acc)?;
                        } else {
                            bases.skip(1)?;
                        }
                    } else {
                        // Place multiplication into the bucket: Separate s * P as 
                        // (s/2^c) * P + (s mod 2^c) P
                        // First multiplication is c bits less, do one can do it,
                        // sum results from different buckets and double it c times,
                        // then add with (s mod 2^c) P parts
                        let mut exp = exp;
                        exp.shr(skip);
                        let exp = exp.as_ref()[0] % (1 << c);

                        if exp != 0 {
                            bases.add_assign_mixed(&mut buckets[(exp - 1) as usize])?;
                        } else {
                            bases.skip(1)?;
                        }
                    }
                }
            }

            // Summation by parts
            // e.g. 3a + 2b + 1c = a +
            //                    (a) + b +
            //                    ((a) + b) + c
            let mut running_sum = G::Projective::zero();
            for exp in buckets.into_iter().rev() {
                running_sum.add_assign(&exp);
                acc.add_assign(&running_sum);
            }

            Ok(acc)
        })
    };

    skip += c;

    if skip >= <G::Engine as ScalarEngine>::Fr::NUM_BITS {
        // There isn't another region.
        Box::new(this)
    } else {
        // There's another region more significant. Calculate and join it with
        // this region recursively.
        Box::new(
            this.join(multiexp_inner(pool, bases, density_map, exponents, skip, c, false))
                .map(move |(this, mut higher)| {
                    for _ in 0..c {
                        higher.double();
                    }

                    higher.add_assign(&this);

                    higher
                })
        )
    }
}

/// Perform multi-exponentiation. The caller is responsible for ensuring the
/// query size is the same as the number of exponents.
pub fn multiexp<Q, D, G, S>(
    pool: &Worker,
    bases: S,
    density_map: D,
    exponents: Arc<Vec<<<G::Engine as ScalarEngine>::Fr as PrimeField>::Repr>>
) -> Box<Future<Item=<G as CurveAffine>::Projective, Error=SynthesisError>>
    where for<'a> &'a Q: QueryDensity,
          D: Send + Sync + 'static + Clone + AsRef<Q>,
          G: CurveAffine,
          S: SourceBuilder<G>
{
    let c = if exponents.len() < 32 {
        3u32
    } else {
        (f64::from(exponents.len() as u32)).ln().ceil() as u32
    };

    if let Some(query_size) = density_map.as_ref().get_query_size() {
        // If the density map has a known query size, it should not be
        // inconsistent with the number of exponents.

        assert!(query_size == exponents.len());
    }

    multiexp_inner(pool, bases, density_map, exponents, 0, c, true)
}

#[test]
fn test_with_bls12() {
    fn naive_multiexp<G: CurveAffine>(
        bases: Arc<Vec<G>>,
        exponents: Arc<Vec<<G::Scalar as PrimeField>::Repr>>
    ) -> G::Projective
    {
        assert_eq!(bases.len(), exponents.len());

        let mut acc = G::Projective::zero();

        for (base, exp) in bases.iter().zip(exponents.iter()) {
            acc.add_assign(&base.mul(*exp));
        }

        acc
    }

    use rand::{self, Rand};
    use pairing::bls12_381::Bls12;

    const SAMPLES: usize = 1 << 14;

    let rng = &mut rand::thread_rng();
    let v = Arc::new((0..SAMPLES).map(|_| <Bls12 as ScalarEngine>::Fr::rand(rng).into_repr()).collect::<Vec<_>>());
    let g = Arc::new((0..SAMPLES).map(|_| <Bls12 as Engine>::G1::rand(rng).into_affine()).collect::<Vec<_>>());

    let naive = naive_multiexp(g.clone(), v.clone());

    let pool = Worker::new();

    let fast = multiexp(
        &pool,
        (g, 0),
        FullDensity,
        v
    ).wait().unwrap();

    assert_eq!(naive, fast);
}
