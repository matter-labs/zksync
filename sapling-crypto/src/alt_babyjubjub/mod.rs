//! Alternative Baby Jubjub is a twisted Edwards curve defined over the BN256 scalar
//! field, Fr. 
//! Fr modulus = 21888242871839275222246405745257275088548364400416034343698204186575808495617
//! It takes the form `-x^2 + y^2 = 1 + dx^2y^2` with
//! `d = -(168696/168700)` using the isomorphism from usual Baby Jubjub 
//! with a requirement that a' = -1, a = 168696, that results in 
//! scaling = 1911982854305225074381251344103329931637610209014896889891168275855466657090 
//! a' = 21888242871839275222246405745257275088548364400416034343698204186575808495616 == -1 = a*scale^2 mod P
//! d' = 12181644023421730124874158521699555681764249180949974110617291017600649128846 == -(168696/168700) = d*scale^2
//! 
//! It is birationally equivalent to a Montgomery
//! curve of the form `y^2 = x^3 + Ax^2 + x` with `A = 168698`. This
//! value `A` is the smallest integer choice such that:
//!
//! * `(A - 2) / 4` is a small integer (`10240`).
//! * `A^2 - 4` is quadratic nonresidue.
//! * The group order of the curve and its quadratic twist has a large
//!   prime factor.
//!
//! Jubjub has `s = 2736030358979909402780800718157159386076813972158567259200215660948447373041`
//! as the prime subgroup order, with cofactor 8. (The twist has
//! cofactor 4.)
//!
//! It is a complete twisted Edwards curve, so the equivalence with
//! the Montgomery curve forms a group isomorphism, allowing points
//! to be freely converted between the two forms.

use pairing::{
    Engine,
};

use ::jubjub::{
    Unknown,
    PrimeOrder,
    FixedGenerators,
    ToUniform,
    JubjubEngine,
    JubjubParams,
    edwards,
    montgomery
};

use ff::{
    Field,
    PrimeField,
    SqrtField
};

use group_hash::baby_group_hash;

use constants;

use pairing::bn256::{
    Bn256,
    Fr
};

// /// This is an implementation of the twisted Edwards Jubjub curve.
// pub mod edwards;

// /// This is an implementation of the birationally equivalent
// /// Montgomery curve.
// pub mod montgomery;

/// This is an implementation of the scalar field for Jubjub.
pub mod fs;

#[cfg(test)]
pub mod tests;

impl JubjubEngine for Bn256 {
    type Fs = self::fs::Fs;
    type Params = AltJubjubBn256;
}

pub struct AltJubjubBn256 {
    edwards_d: Fr,
    montgomery_a: Fr,
    montgomery_2a: Fr,
    scale: Fr,

    pedersen_hash_generators: Vec<edwards::Point<Bn256, PrimeOrder>>,
    pedersen_hash_exp: Vec<Vec<Vec<edwards::Point<Bn256, PrimeOrder>>>>,
    pedersen_circuit_generators: Vec<Vec<Vec<(Fr, Fr)>>>,

    fixed_base_generators: Vec<edwards::Point<Bn256, PrimeOrder>>,
    fixed_base_circuit_generators: Vec<Vec<Vec<(Fr, Fr)>>>,
}

impl JubjubParams<Bn256> for AltJubjubBn256 {
    fn edwards_d(&self) -> &Fr { &self.edwards_d }
    fn montgomery_a(&self) -> &Fr { &self.montgomery_a }
    fn montgomery_2a(&self) -> &Fr { &self.montgomery_2a }
    fn scale(&self) -> &Fr { &self.scale }
    fn pedersen_hash_generators(&self) -> &[edwards::Point<Bn256, PrimeOrder>] {
        &self.pedersen_hash_generators
    }
    fn pedersen_hash_exp_table(&self) -> &[Vec<Vec<edwards::Point<Bn256, PrimeOrder>>>] {
        &self.pedersen_hash_exp
    }
    fn pedersen_hash_chunks_per_generator(&self) -> usize {
        62
    }
    fn fixed_base_chunks_per_generator(&self) -> usize {
        84
    }
    fn pedersen_circuit_generators(&self) -> &[Vec<Vec<(Fr, Fr)>>] {
        &self.pedersen_circuit_generators
    }
    fn generator(&self, base: FixedGenerators) -> &edwards::Point<Bn256, PrimeOrder>
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

impl AltJubjubBn256 {
    pub fn new() -> Self {
        let montgomery_a = Fr::from_str("168698").unwrap();
        let mut montgomery_2a = montgomery_a;
        montgomery_2a.double();

        let mut tmp_params = AltJubjubBn256 {
            // d = -(168696/168700)
            edwards_d: Fr::from_str("12181644023421730124874158521699555681764249180949974110617291017600649128846").unwrap(),
            // A = 168698
            montgomery_a: montgomery_a,
            // 2A = 2.A
            montgomery_2a: montgomery_2a,
            // scaling factor = sqrt(4 / (a - d))
            scale: Fr::from_str("6360561867910373094066688120553762416144456282423235903351243436111059670888").unwrap(),

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
                let gh = baby_group_hash(
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

                let window = AltJubjubBn256::pedersen_hash_exp_window_size();

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
fn test_jubjub_altbn256() {
    let params = AltJubjubBn256::new();

    tests::test_suite::<Bn256>(&params);

    // let test_repr = hex!("9d12b88b08dcbef8a11ee0712d94cb236ee2f4ca17317075bfafc82ce3139d31");
    // let p = edwards::Point::<Bn256, _>::read(&test_repr[..], &params).unwrap();
    // let q = edwards::Point::<Bn256, _>::get_for_y(
    //     Fr::from_str("22440861827555040311190986994816762244378363690614952020532787748720529117853").unwrap(),
    //     false,
    //     &params
    // ).unwrap();

    // assert!(p == q);

    // // Same thing, but sign bit set
    // let test_repr = hex!("9d12b88b08dcbef8a11ee0712d94cb236ee2f4ca17317075bfafc82ce3139db1");
    // let p = edwards::Point::<Bn256, _>::read(&test_repr[..], &params).unwrap();
    // let q = edwards::Point::<Bn256, _>::get_for_y(
    //     Fr::from_str("22440861827555040311190986994816762244378363690614952020532787748720529117853").unwrap(),
    //     true,
    //     &params
    // ).unwrap();

    // assert!(p == q);
}
