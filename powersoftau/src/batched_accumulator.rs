/// Memory constrained accumulator that checks parts of the initial information in parts that fit to memory
/// and then contributes to entropy in parts as well

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

use super::keypair::*;
use super::utils::*;
use super::parameters::*;

pub enum AccumulatorState{
    Empty,
    NonEmpty,
    Transformed,
}

/// The `Accumulator` is an object that participants of the ceremony contribute
/// randomness to. This object contains powers of trapdoor `tau` in G1 and in G2 over
/// fixed generators, and additionally in G1 over two other generators of exponents
/// `alpha` and `beta` over those fixed generators. In other words:
///
/// * (τ, τ<sup>2</sup>, ..., τ<sup>2<sup>22</sup> - 2</sup>, α, ατ, ατ<sup>2</sup>, ..., ατ<sup>2<sup>21</sup> - 1</sup>, β, βτ, βτ<sup>2</sup>, ..., βτ<sup>2<sup>21</sup> - 1</sup>)<sub>1</sub>
/// * (β, τ, τ<sup>2</sup>, ..., τ<sup>2<sup>21</sup> - 1</sup>)<sub>2</sub>
pub struct BachedAccumulator<E: Engine, P: PowersOfTauParameters> {
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
    pub parameters: P,
    /// Keep the last point read from file
    pub state: AccumulatorState,
    /// Hash chain hash
    pub hash: GenericArray<u8, U64>
}

impl<E:Engine, P: PowersOfTauParameters> BachedAccumulator<E, P> {
    pub fn calculate_hash(input_map: &Mmap) -> GenericArray<u8, U64> {
        let chunk_size = 1 << 30; // read by 1GB from map
        let mut hasher = Blake2b::default();
        for chunk in input_map.chunks(chunk_size) {
            hasher.input(&chunk);
        }

        hasher.result()
    }
}

impl<E:Engine, P: PowersOfTauParameters> BachedAccumulator<E, P> {
    pub fn empty(parameters: P) -> Self {
        Self {
            tau_powers_g1: vec![],
            tau_powers_g2: vec![],
            alpha_tau_powers_g1: vec![],
            beta_tau_powers_g1: vec![],
            beta_g2: E::G2Affine::zero(),
            parameters: parameters,
            state: AccumulatorState::Empty,
            hash: blank_hash(),
        }
    }
}

impl<E:Engine, P: PowersOfTauParameters> BachedAccumulator<E, P> {
    fn g1_size(compression: UseCompression) -> usize {
        match compression {
            UseCompression::Yes => {
                return P::G1_COMPRESSED_BYTE_SIZE;
            },
            UseCompression::No => {
                return P::G1_UNCOMPRESSED_BYTE_SIZE;
            }
        }
    } 

    fn g2_size(compression: UseCompression) -> usize {
        match compression {
            UseCompression::Yes => {
                return P::G2_COMPRESSED_BYTE_SIZE;
            },
            UseCompression::No => {
                return P::G2_UNCOMPRESSED_BYTE_SIZE;
            }
        }
    } 

    fn get_size(element_type: ElementType, compression: UseCompression) -> usize {
        let size = match element_type {
            ElementType::AlphaG1 | ElementType::BetaG1 | ElementType::TauG1 => { Self::g1_size(compression) },
            ElementType::BetaG2 | ElementType::TauG2 => { Self::g2_size(compression) }
        };

        size
    }

    /// File expected structure
    /// TAU_POWERS_G1_LENGTH of G1 points
    /// TAU_POWERS_LENGTH of G2 points
    /// TAU_POWERS_LENGTH of G1 points for alpha
    /// TAU_POWERS_LENGTH of G1 points for beta
    /// One G2 point for beta
    
    fn calculate_mmap_position(index: usize, element_type: ElementType, compression: UseCompression) -> usize {
        let g1_size = Self::g1_size(compression);
        let g2_size = Self::g2_size(compression);
        let required_tau_g1_power = P::TAU_POWERS_G1_LENGTH;
        let required_power = P::TAU_POWERS_LENGTH;
        let position = match element_type {
            ElementType::TauG1 => {
                let mut position = 0;
                position += g1_size * index;

                position
            },
            ElementType::TauG2 => {
                let mut position = 0;
                position += g1_size * required_tau_g1_power;
                if index > P::TAU_POWERS_LENGTH {
                    position += g2_size * required_power
                } else {
                    position += g2_size * index;
                }
                
                position
            },
            ElementType::AlphaG1 => {
                let mut position = 0;
                position += g1_size * required_tau_g1_power;
                position += g2_size * required_power;
                if index > P::TAU_POWERS_LENGTH {
                    position += g1_size * required_power
                } else {
                    position += g1_size * index;
                }

                position   
            },
            ElementType::BetaG1 => {
                let mut position = 0;
                position += g1_size * required_tau_g1_power;
                position += g2_size * required_power;
                position += g1_size * required_power;
                if index > P::TAU_POWERS_LENGTH {
                    position += g1_size * required_power
                } else {
                    position += g1_size * index;
                }

                position 
            },
            ElementType::BetaG2 => {
                let mut position = 0;
                position += g1_size * required_tau_g1_power;
                position += g2_size * required_power;
                position += g1_size * required_power;
                position += g1_size * required_power;

                position
            }
        };

        position + P::HASH_SIZE
    }
}

impl<E:Engine, P: PowersOfTauParameters> BachedAccumulator<E, P> {
    /// Verifies a transformation of the `Accumulator` with the `PublicKey`, given a 64-byte transcript `digest`.
    pub fn verify_transformation(
        input_map: &Mmap,
        output_map: &Mmap,
        parameters: P,
        key: &PublicKey<E>, 
        digest: &[u8]
    ) -> bool
    {
        use itertools::MinMaxResult::{NoElements, OneElement, MinMax};
        assert_eq!(digest.len(), 64);

        let tau_g2_s = compute_g2_s::<E>(&digest, &key.tau_g1.0, &key.tau_g1.1, 0);
        let alpha_g2_s = compute_g2_s::<E>(&digest, &key.alpha_g1.0, &key.alpha_g1.1, 1);
        let beta_g2_s = compute_g2_s::<E>(&digest, &key.beta_g1.0, &key.beta_g1.1, 2);

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

        // Load accumulators AND perform computations

        let mut before = Self::empty(parameters.clone());
        let mut after = Self::empty(parameters.clone());

        // these checks only touch a part of the accumulator, so read one element in principle

        {
            let chunk_size = 1;
            before.read_chunk(0, chunk_size, UseCompression::No, CheckForCorrectness::No, &input_map).expect("must read a first chunk");
            after.read_chunk(0, chunk_size, UseCompression::No, CheckForCorrectness::No, &output_map).expect("must read a first chunk");

            match before.state {
                AccumulatorState::Empty => {panic!("Accumulator is empty")},
                _ => {}
            };

            match after.state {
                AccumulatorState::Empty => {panic!("Accumulator is empty")},
                _ => {}
            };

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

        }

        let tau_powers_g2_0 = after.tau_powers_g2[0].clone();
        let tau_powers_g2_1 = after.tau_powers_g2[1].clone();
        let tau_powers_g1_0 = after.tau_powers_g1[0].clone();
        let tau_powers_g1_1 = after.tau_powers_g1[1].clone();


        // Read by parts and just verify same ratios. Cause of two fixed variables above with tau_powers_g2_1 = tau_powers_g2_0 ^ s
        // one does not need to care about some overlapping
        
        for chunk in &(0..P::TAU_POWERS_LENGTH).into_iter().chunks(P::EMPIRICAL_BATCH_SIZE) {
            if let MinMax(start, end) = chunk.minmax() {
                let chunk_size = end - start;
                before.read_chunk(start, chunk_size, UseCompression::No, CheckForCorrectness::No, &input_map).expect("must read a first chunk");
                after.read_chunk(start, chunk_size, UseCompression::No, CheckForCorrectness::No, &output_map).expect("must read a first chunk");

                assert_eq!(before.tau_powers_g2.len(), 0, "during rest of tau g1 generation tau g2 must be empty");
                assert_eq!(after.tau_powers_g2.len(), 0, "during rest of tau g1 generation tau g2 must be empty");

                // Are the powers of tau correct?
                if !same_ratio(power_pairs(&after.tau_powers_g1), (tau_powers_g2_0, tau_powers_g2_1)) {
                    return false;
                }
                if !same_ratio(power_pairs(&after.tau_powers_g2), (tau_powers_g1_0, tau_powers_g1_1)) {
                    return false;
                }
                if !same_ratio(power_pairs(&after.alpha_tau_powers_g1), (tau_powers_g2_0, tau_powers_g2_1)) {
                    return false;
                }
                if !same_ratio(power_pairs(&after.beta_tau_powers_g1), (tau_powers_g2_0, tau_powers_g2_1)) {
                    return false;
                }
            } else {
                panic!("Chunk does not have a min and max");
            }
        }

        for chunk in &(P::TAU_POWERS_LENGTH..P::TAU_POWERS_G1_LENGTH).into_iter().chunks(P::EMPIRICAL_BATCH_SIZE) {
            if let MinMax(start, end) = chunk.minmax() {
                let chunk_size = end - start;
                before.read_chunk(start, chunk_size, UseCompression::No, CheckForCorrectness::No, &input_map).expect("must read a first chunk");
                after.read_chunk(start, chunk_size, UseCompression::No, CheckForCorrectness::No, &output_map).expect("must read a first chunk");

                // Are the powers of tau correct?
                if !same_ratio(power_pairs(&after.tau_powers_g1), (tau_powers_g2_0, tau_powers_g2_1)) {
                    return false;
                }
            } else {
                panic!("Chunk does not have a min and max");
            }
        }

        true
    }
}

impl<E:Engine, P: PowersOfTauParameters> BachedAccumulator<E, P> {
        pub fn read_chunk (
        &mut self,
        from: usize,
        size: usize,
        compression: UseCompression,
        checked: CheckForCorrectness,
        input_map: &Mmap,
    ) -> Result<(), DeserializationError>
    {
        self.tau_powers_g1 = match compression {
            UseCompression::Yes => {
                self.read_points_chunk::<<E::G1Affine as CurveAffine>::Compressed>(from, size, ElementType::TauG1, compression, checked, &input_map)?
            },
            UseCompression::No => {
                self.read_points_chunk::<<E::G1Affine as CurveAffine>::Uncompressed>(from, size, ElementType::TauG1, compression, checked, &input_map)?
            },

        };

        self.tau_powers_g2 = match compression {
            UseCompression::Yes => {
                self.read_points_chunk::<<E::G2Affine as CurveAffine>::Compressed>(from, size, ElementType::TauG2, compression, checked, &input_map)?
            },
            UseCompression::No => {
                self.read_points_chunk::<<E::G2Affine as CurveAffine>::Uncompressed>(from, size, ElementType::TauG2, compression, checked, &input_map)?
            },

        };

        self.alpha_tau_powers_g1 = match compression {
            UseCompression::Yes => {
                self.read_points_chunk::<<E::G1Affine as CurveAffine>::Compressed>(from, size, ElementType::AlphaG1, compression, checked, &input_map)?
            },
            UseCompression::No => {
                self.read_points_chunk::<<E::G1Affine as CurveAffine>::Uncompressed>(from, size, ElementType::AlphaG1, compression, checked, &input_map)?
            },

        };

        self.beta_tau_powers_g1 = match compression {
            UseCompression::Yes => {
                self.read_points_chunk::<<E::G1Affine as CurveAffine>::Compressed>(from, size, ElementType::BetaG1, compression, checked, &input_map)?
            },
            UseCompression::No => {
                self.read_points_chunk::<<E::G1Affine as CurveAffine>::Uncompressed>(from, size, ElementType::BetaG1, compression, checked, &input_map)?
            },
        };

        self.beta_g2 = match compression {
            UseCompression::Yes => {
                let points = self.read_points_chunk::<<E::G2Affine as CurveAffine>::Compressed>(0, 1, ElementType::BetaG2, compression, checked, &input_map)?;
                
                points[0]
            },
            UseCompression::No => {
                let points = self.read_points_chunk::<<E::G2Affine as CurveAffine>::Uncompressed>(0, 1, ElementType::BetaG2, compression, checked, &input_map)?;

                points[0]
            },
        };

        match self.state {
            AccumulatorState::Empty => {
                self.state = AccumulatorState::NonEmpty;
            },
            _ => {},
        };

        Ok(())
    }

    // fn read_point<ENC: EncodedPoint>(

    // ) -> 

    fn read_points_chunk<ENC: EncodedPoint>(
        &mut self,
        from: usize,
        size: usize,
        element_type: ElementType,
        compression: UseCompression,
        checked: CheckForCorrectness,
        input_map: &Mmap,
    ) -> Result<Vec<ENC::Affine>, DeserializationError>
    {
        // Read the encoded elements
        let mut res = vec![ENC::empty(); size];

        for (i, encoded) in res.iter_mut().enumerate() {
            let index = from + i;
            match element_type {
                ElementType::TauG1 => {
                    if index > P::TAU_POWERS_G1_LENGTH {
                        return Ok(vec![]);
                    }
                },
                ElementType::AlphaG1 | ElementType::BetaG1 | ElementType::BetaG2 | ElementType::TauG2 => { 
                    if index > P::TAU_POWERS_LENGTH {
                        return Ok(vec![]);
                    }
                }
            };
            let position = Self::calculate_mmap_position(index, element_type.clone(), compression);
            let element_size = Self::get_size(element_type.clone(), compression);
            let memory_slice = input_map.get(position..position+element_size).expect("must read point data from file");
            // memory_slice.write();
            // encoded.as_mut() = self.input_map[position+element_size];
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
}

impl<E:Engine, P: PowersOfTauParameters> BachedAccumulator<E, P> {
    fn write_all(
        &mut self,
        chunk_start: usize,
        compression: UseCompression,
        element_type: ElementType,
        output_map: &mut MmapMut,
    ) -> io::Result<()>
    {
        match element_type {
            ElementType::TauG1 => {
                for (i, c) in self.tau_powers_g1.clone().iter().enumerate() {
                    let index = chunk_start + i;
                    self.write_point(index, c, compression, element_type.clone(), output_map)?;
                }
            },
            ElementType::TauG2 => {
                for (i, c) in self.tau_powers_g2.clone().iter().enumerate() {
                    let index = chunk_start + i;
                    self.write_point(index, c, compression, element_type.clone(), output_map)?;
                }
            },
            ElementType::AlphaG1 => {
                for (i, c) in self.alpha_tau_powers_g1.clone().iter().enumerate() {
                    let index = chunk_start + i;
                    self.write_point(index, c, compression, element_type.clone(), output_map)?;
                }
            },
            ElementType::BetaG1 => {
                for (i, c) in self.beta_tau_powers_g1.clone().iter().enumerate() {
                    let index = chunk_start + i;
                    self.write_point(index, c, compression, element_type.clone(), output_map)?;
                }
            },
            ElementType::BetaG2 => {
                self.write_point(0, &self.beta_g2.clone(), compression, element_type.clone(), output_map)?
            }
        };

        output_map.flush_async()?;

        Ok(())
    }

    fn write_point<C>(
        &mut self,
        index: usize,
        p: &C,
        compression: UseCompression,
        element_type: ElementType,
        output_map: &mut MmapMut,
    ) -> io::Result<()>
        where C: CurveAffine<Engine = E, Scalar = E::Fr>
    {
        match element_type {
            ElementType::TauG1 => {
                if index > P::TAU_POWERS_G1_LENGTH {
                    return Ok(());
                }
            },
            ElementType::AlphaG1 | ElementType::BetaG1 | ElementType::BetaG2 | ElementType::TauG2 => { 
                if index > P::TAU_POWERS_LENGTH {
                    return Ok(());
                }
            }
        };

        match compression {
            UseCompression::Yes => {
                let position = Self::calculate_mmap_position(index, element_type, compression);
                // let size = self.get_size(element_type, compression);
                (&mut output_map[position..]).write(p.into_compressed().as_ref())?;
            },
            UseCompression::No => {
                let position = Self::calculate_mmap_position(index, element_type, compression);
                // let size = self.get_size(element_type, compression);
                (&mut output_map[position..]).write(p.into_uncompressed().as_ref())?;
            },
        };

        Ok(())
    }

    /// Write the accumulator with some compression behavior.
    pub fn write_chunk(
        &mut self,
        chunk_start: usize,
        compression: UseCompression,
        output_map: &mut MmapMut
    ) -> io::Result<()>
    {
        self.write_all(chunk_start, compression, ElementType::TauG1, output_map)?;
        self.write_all(chunk_start, compression, ElementType::TauG2, output_map)?;
        self.write_all(chunk_start, compression, ElementType::AlphaG1, output_map)?;
        self.write_all(chunk_start, compression, ElementType::BetaG1, output_map)?;
        self.write_all(chunk_start, compression, ElementType::BetaG2, output_map)?;

        Ok(())
    }

}

impl<E:Engine, P: PowersOfTauParameters> BachedAccumulator<E, P> {
    /// Transforms the accumulator with a private key.
    pub fn transform(
        input_map: &Mmap,
        output_map: &mut MmapMut,
        parameters: P,
        key: &PrivateKey<E>
    ) -> io::Result<()>
    {
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

        let mut accumulator = Self::empty(parameters.clone());

        use itertools::MinMaxResult::{NoElements, OneElement, MinMax};

        for chunk in &(0..P::TAU_POWERS_LENGTH).into_iter().chunks(P::EMPIRICAL_BATCH_SIZE) {
            if let MinMax(start, end) = chunk.minmax() {
                let size = end - start;
                accumulator.read_chunk(start, size, UseCompression::No, CheckForCorrectness::No, &input_map).expect("must read a first chunk");

                // Construct the powers of tau
                let mut taupowers = vec![E::Fr::zero(); size];
                let chunk_size = size / num_cpus::get();

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

                batch_exp::<E, _>(&mut accumulator.tau_powers_g1, &taupowers[0..], None);
                batch_exp::<E, _>(&mut accumulator.tau_powers_g2, &taupowers[0..], None);
                batch_exp::<E, _>(&mut accumulator.alpha_tau_powers_g1, &taupowers[0..], Some(&key.alpha));
                batch_exp::<E, _>(&mut accumulator.beta_tau_powers_g1, &taupowers[0..], Some(&key.beta));
                accumulator.beta_g2 = accumulator.beta_g2.mul(key.beta).into_affine();
                accumulator.write_chunk(start, UseCompression::Yes, output_map)?;
            } else {
                panic!("Chunk does not have a min and max");
            }
        }

        for chunk in &(P::TAU_POWERS_LENGTH..P::TAU_POWERS_G1_LENGTH).into_iter().chunks(P::EMPIRICAL_BATCH_SIZE) {
            if let MinMax(start, end) = chunk.minmax() {
                let size = end - start;
                accumulator.read_chunk(start, size, UseCompression::No, CheckForCorrectness::No, &input_map).expect("must read a first chunk");
                assert_eq!(accumulator.tau_powers_g2.len(), 0, "during rest of tau g1 generation tau g2 must be empty");

                // Construct the powers of tau
                let mut taupowers = vec![E::Fr::zero(); size];
                let chunk_size = size / num_cpus::get();

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

                batch_exp::<E, _>(&mut accumulator.tau_powers_g1, &taupowers[0..], None);
                accumulator.beta_g2 = accumulator.beta_g2.mul(key.beta).into_affine();
                accumulator.write_chunk(start, UseCompression::Yes, output_map)?;
            } else {
                panic!("Chunk does not have a min and max");
            }
        }

        Ok(())
    }
}
