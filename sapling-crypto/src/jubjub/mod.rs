//! Jubjub is a twisted Edwards curve defined over the BLS12-381 scalar
//! field, Fr. It takes the form `-x^2 + y^2 = 1 + dx^2y^2` with
//! `d = -(10240/10241)`. It is birationally equivalent to a Montgomery
//! curve of the form `y^2 = x^3 + Ax^2 + x` with `A = 40962`. This
//! value `A` is the smallest integer choice such that:
//!
//! * `(A - 2) / 4` is a small integer (`10240`).
//! * `A^2 - 4` is quadratic nonresidue.
//! * The group order of the curve and its quadratic twist has a large
//!   prime factor.
//!
//! Jubjub has `s = 0x0e7db4ea6533afa906673b0101343b00a6682093ccc81082d0970e5ed6f72cb7`
//! as the prime subgroup order, with cofactor 8. (The twist has
//! cofactor 4.)
//!
//! It is a complete twisted Edwards curve, so the equivalence with
//! the Montgomery curve forms a group isomorphism, allowing points
//! to be freely converted between the two forms.

use pairing::{
    Engine,
};

use ff::{
    Field,
    PrimeField,
    SqrtField
};

use group_hash::group_hash;

use constants;

use pairing::bls12_381::{
    Bls12,
    Fr
};

/// This is an implementation of the twisted Edwards Jubjub curve.
pub mod edwards;

/// This is an implementation of the birationally equivalent
/// Montgomery curve.
pub mod montgomery;

/// This is an implementation of the scalar field for Jubjub.
pub mod fs;

#[cfg(test)]
pub mod tests;

/// Point of unknown order.
pub enum Unknown { }

/// Point of prime order.
pub enum PrimeOrder { }

/// Fixed generators of the Jubjub curve of unknown
/// exponent.
#[derive(Copy, Clone)]
pub enum FixedGenerators {
    /// The prover will demonstrate knowledge of discrete log
    /// with respect to this base when they are constructing
    /// a proof, in order to authorize proof construction.
    ProofGenerationKey = 0,

    /// The note commitment is randomized over this generator.
    NoteCommitmentRandomness = 1,

    /// The node commitment is randomized again by the position
    /// in order to supply the nullifier computation with a
    /// unique input w.r.t. the note being spent, to prevent
    /// Faerie gold attacks.
    NullifierPosition = 2,

    /// The value commitment is used to check balance between
    /// inputs and outputs. The value is placed over this
    /// generator.
    ValueCommitmentValue = 3,
    /// The value commitment is randomized over this generator,
    /// for privacy.
    ValueCommitmentRandomness = 4,

    /// The spender proves discrete log with respect to this
    /// base at spend time.
    SpendingKeyGenerator = 5,

    Max = 6
}

pub trait ToUniform {
    fn to_uniform(digest: &[u8]) -> Self;
}

/// This is an extension to the pairing Engine trait which
/// offers a scalar field for the embedded curve (Jubjub)
/// and some pre-computed parameters.
pub trait JubjubEngine: Engine {
    /// The scalar field of the Jubjub curve
    type Fs: PrimeField + SqrtField + ToUniform;
    /// The parameters of Jubjub and the Sapling protocol
    type Params: JubjubParams<Self>;
}

/// The pre-computed parameters for Jubjub, including curve
/// constants and various limits and window tables.
pub trait JubjubParams<E: JubjubEngine>: Sized {
    /// The `d` constant of the twisted Edwards curve.
    fn edwards_d(&self) -> &E::Fr;
    /// The `A` constant of the birationally equivalent Montgomery curve.
    fn montgomery_a(&self) -> &E::Fr;
    /// The `A` constant, doubled.
    fn montgomery_2a(&self) -> &E::Fr;
    /// The scaling factor used for conversion from the Montgomery form.
    fn scale(&self) -> &E::Fr;
    /// Returns the generators (for each segment) used in all Pedersen commitments.
    fn pedersen_hash_generators(&self) -> &[edwards::Point<E, PrimeOrder>];
    /// Returns the exp table for Pedersen hashes.
    fn pedersen_hash_exp_table(&self) -> &[Vec<Vec<edwards::Point<E, PrimeOrder>>>];
    /// Returns the maximum number of chunks per segment of the Pedersen hash.
    fn pedersen_hash_chunks_per_generator(&self) -> usize;
    /// Returns the pre-computed window tables [-4, 3, 2, 1, 1, 2, 3, 4] of different
    /// magnitudes of the Pedersen hash segment generators.
    fn pedersen_circuit_generators(&self) -> &[Vec<Vec<(E::Fr, E::Fr)>>];

    /// Returns the number of chunks needed to represent a full scalar during fixed-base
    /// exponentiation.
    fn fixed_base_chunks_per_generator(&self) -> usize;
    /// Returns a fixed generator.
    fn generator(&self, base: FixedGenerators) -> &edwards::Point<E, PrimeOrder>;
    /// Returns a window table [0, 1, ..., 8] for different magnitudes of some
    /// fixed generator.
    fn circuit_generators(&self, FixedGenerators) -> &[Vec<(E::Fr, E::Fr)>];
    /// Returns the window size for exponentiation of Pedersen hash generators
    /// outside the circuit
    fn pedersen_hash_exp_window_size() -> u32;
}

impl JubjubEngine for Bls12 {
    type Fs = self::fs::Fs;
    type Params = JubjubBls12;
}

pub struct JubjubBls12 {
    edwards_d: Fr,
    montgomery_a: Fr,
    montgomery_2a: Fr,
    scale: Fr,

    pedersen_hash_generators: Vec<edwards::Point<Bls12, PrimeOrder>>,
    pedersen_hash_exp: Vec<Vec<Vec<edwards::Point<Bls12, PrimeOrder>>>>,
    pedersen_circuit_generators: Vec<Vec<Vec<(Fr, Fr)>>>,

    fixed_base_generators: Vec<edwards::Point<Bls12, PrimeOrder>>,
    fixed_base_circuit_generators: Vec<Vec<Vec<(Fr, Fr)>>>,
}

impl JubjubParams<Bls12> for JubjubBls12 {
    fn edwards_d(&self) -> &Fr { &self.edwards_d }
    fn montgomery_a(&self) -> &Fr { &self.montgomery_a }
    fn montgomery_2a(&self) -> &Fr { &self.montgomery_2a }
    fn scale(&self) -> &Fr { &self.scale }
    fn pedersen_hash_generators(&self) -> &[edwards::Point<Bls12, PrimeOrder>] {
        &self.pedersen_hash_generators
    }
    fn pedersen_hash_exp_table(&self) -> &[Vec<Vec<edwards::Point<Bls12, PrimeOrder>>>] {
        &self.pedersen_hash_exp
    }
    fn pedersen_hash_chunks_per_generator(&self) -> usize {
        63
    }
    fn fixed_base_chunks_per_generator(&self) -> usize {
        84
    }
    fn pedersen_circuit_generators(&self) -> &[Vec<Vec<(Fr, Fr)>>] {
        &self.pedersen_circuit_generators
    }
    fn generator(&self, base: FixedGenerators) -> &edwards::Point<Bls12, PrimeOrder>
    {
        &self.fixed_base_generators[base as usize]
    }
    fn circuit_generators(&self, base: FixedGenerators) -> &[Vec<(Fr, Fr)>]
    {
        &self.fixed_base_circuit_generators[base as usize][..]
    }
    fn pedersen_hash_exp_window_size() -> u32 {
        8
    }
}

impl JubjubBls12 {
    pub fn new() -> Self {
        let montgomery_a = Fr::from_str("40962").unwrap();
        let mut montgomery_2a = montgomery_a;
        montgomery_2a.double();

        let mut tmp_params = JubjubBls12 {
            // d = -(10240/10241)
            edwards_d: Fr::from_str("19257038036680949359750312669786877991949435402254120286184196891950884077233").unwrap(),
            // A = 40962
            montgomery_a: montgomery_a,
            // 2A = 2.A
            montgomery_2a: montgomery_2a,
            // scaling factor = sqrt(4 / (a - d))
            scale: Fr::from_str("17814886934372412843466061268024708274627479829237077604635722030778476050649").unwrap(),

            // We'll initialize these below
            pedersen_hash_generators: vec![],
            pedersen_hash_exp: vec![],
            pedersen_circuit_generators: vec![],
            fixed_base_generators: vec![],
            fixed_base_circuit_generators: vec![],
        };

        fn find_group_hash<E: JubjubEngine>(
            m: &[u8],
            personalization: &[u8; 8],
            params: &E::Params
        ) -> edwards::Point<E, PrimeOrder>
        {
            let mut tag = m.to_vec();
            let i = tag.len();
            tag.push(0u8);

            loop {
                let gh = group_hash(
                    &tag,
                    personalization,
                    params
                );

                // We don't want to overflow and start reusing generators
                assert!(tag[i] != u8::max_value());
                tag[i] += 1;

                if let Some(gh) = gh {
                    break gh;
                }
            }
        }

        // Create the bases for the Pedersen hashes
        {
            let mut pedersen_hash_generators = vec![];

            for m in 0..5 {
                use byteorder::{WriteBytesExt, LittleEndian};

                let mut segment_number = [0u8; 4];
                (&mut segment_number[0..4]).write_u32::<LittleEndian>(m).unwrap();

                pedersen_hash_generators.push(
                    find_group_hash(
                        &segment_number,
                        constants::PEDERSEN_HASH_GENERATORS_PERSONALIZATION,
                        &tmp_params
                    )
                );
            }

            // Check for duplicates, far worse than spec inconsistencies!
            for (i, p1) in pedersen_hash_generators.iter().enumerate() {
                if p1 == &edwards::Point::zero() {
                    panic!("Neutral element!");
                }

                for p2 in pedersen_hash_generators.iter().skip(i+1) {
                    if p1 == p2 {
                        panic!("Duplicate generator!");
                    }
                }
            }

            tmp_params.pedersen_hash_generators = pedersen_hash_generators;
        }

        // Create the exp table for the Pedersen hash generators
        {
            let mut pedersen_hash_exp = vec![];

            for g in &tmp_params.pedersen_hash_generators {
                let mut g = g.clone();

                let window = JubjubBls12::pedersen_hash_exp_window_size();

                let mut tables = vec![];

                let mut num_bits = 0;
                while num_bits <= fs::Fs::NUM_BITS {
                    let mut table = Vec::with_capacity(1 << window);

                    let mut base = edwards::Point::zero();

                    for _ in 0..(1 << window) {
                        table.push(base.clone());
                        base = base.add(&g, &tmp_params);
                    }

                    tables.push(table);
                    num_bits += window;

                    for _ in 0..window {
                        g = g.double(&tmp_params);
                    }
                }

                pedersen_hash_exp.push(tables);
            }

            tmp_params.pedersen_hash_exp = pedersen_hash_exp;
        }

        // Create the bases for other parts of the protocol
        {
            let mut fixed_base_generators = vec![edwards::Point::zero(); FixedGenerators::Max as usize];

            fixed_base_generators[FixedGenerators::ProofGenerationKey as usize] =
                find_group_hash(&[], constants::PROOF_GENERATION_KEY_BASE_GENERATOR_PERSONALIZATION, &tmp_params);

            fixed_base_generators[FixedGenerators::NoteCommitmentRandomness as usize] =
                find_group_hash(b"r", constants::PEDERSEN_HASH_GENERATORS_PERSONALIZATION, &tmp_params);

            fixed_base_generators[FixedGenerators::NullifierPosition as usize] =
                find_group_hash(&[], constants::NULLIFIER_POSITION_IN_TREE_GENERATOR_PERSONALIZATION, &tmp_params);

            fixed_base_generators[FixedGenerators::ValueCommitmentValue as usize] =
                find_group_hash(b"v", constants::VALUE_COMMITMENT_GENERATOR_PERSONALIZATION, &tmp_params);

            fixed_base_generators[FixedGenerators::ValueCommitmentRandomness as usize] =
                find_group_hash(b"r", constants::VALUE_COMMITMENT_GENERATOR_PERSONALIZATION, &tmp_params);

            fixed_base_generators[FixedGenerators::SpendingKeyGenerator as usize] =
                find_group_hash(&[], constants::SPENDING_KEY_GENERATOR_PERSONALIZATION, &tmp_params);

            // Check for duplicates, far worse than spec inconsistencies!
            for (i, p1) in fixed_base_generators.iter().enumerate() {
                if p1 == &edwards::Point::zero() {
                    panic!("Neutral element!");
                }

                for p2 in fixed_base_generators.iter().skip(i+1) {
                    if p1 == p2 {
                        panic!("Duplicate generator!");
                    }
                }
            }

            tmp_params.fixed_base_generators = fixed_base_generators;
        }

        // Create the 2-bit window table lookups for each 4-bit
        // "chunk" in each segment of the Pedersen hash
        {
            let mut pedersen_circuit_generators = vec![];

            // Process each segment
            for mut gen in tmp_params.pedersen_hash_generators.iter().cloned() {
                let mut gen = montgomery::Point::from_edwards(&gen, &tmp_params);
                let mut windows = vec![];
                for _ in 0..tmp_params.pedersen_hash_chunks_per_generator() {
                    // Create (x, y) coeffs for this chunk
                    let mut coeffs = vec![];
                    let mut g = gen.clone();

                    // coeffs = g, g*2, g*3, g*4
                    for _ in 0..4 {
                        coeffs.push(g.into_xy().expect("cannot produce O"));
                        g = g.add(&gen, &tmp_params);
                    }
                    windows.push(coeffs);

                    // Our chunks are separated by 2 bits to prevent overlap.
                    for _ in 0..4 {
                        gen = gen.double(&tmp_params);
                    }
                }
                pedersen_circuit_generators.push(windows);
            }

            tmp_params.pedersen_circuit_generators = pedersen_circuit_generators;
        }

        // Create the 3-bit window table lookups for fixed-base
        // exp of each base in the protocol.
        {
            let mut fixed_base_circuit_generators = vec![];

            for mut gen in tmp_params.fixed_base_generators.iter().cloned() {
                let mut windows = vec![];
                for _ in 0..tmp_params.fixed_base_chunks_per_generator() {
                    let mut coeffs = vec![(Fr::zero(), Fr::one())];
                    let mut g = gen.clone();
                    for _ in 0..7 {
                        coeffs.push(g.into_xy());
                        g = g.add(&gen, &tmp_params);
                    }
                    windows.push(coeffs);

                    // gen = gen * 8
                    gen = g;
                }
                fixed_base_circuit_generators.push(windows);
            }

            tmp_params.fixed_base_circuit_generators = fixed_base_circuit_generators;
        }

        tmp_params
    }
}

#[test]
fn test_jubjub_bls12() {
    let params = JubjubBls12::new();

    tests::test_suite::<Bls12>(&params);

    let test_repr = hex!("9d12b88b08dcbef8a11ee0712d94cb236ee2f4ca17317075bfafc82ce3139d31");
    let p = edwards::Point::<Bls12, _>::read(&test_repr[..], &params).unwrap();
    let q = edwards::Point::<Bls12, _>::get_for_y(
        Fr::from_str("22440861827555040311190986994816762244378363690614952020532787748720529117853").unwrap(),
        false,
        &params
    ).unwrap();

    assert!(p == q);

    // Same thing, but sign bit set
    let test_repr = hex!("9d12b88b08dcbef8a11ee0712d94cb236ee2f4ca17317075bfafc82ce3139db1");
    let p = edwards::Point::<Bls12, _>::read(&test_repr[..], &params).unwrap();
    let q = edwards::Point::<Bls12, _>::get_for_y(
        Fr::from_str("22440861827555040311190986994816762244378363690614952020532787748720529117853").unwrap(),
        true,
        &params
    ).unwrap();

    assert!(p == q);
}
