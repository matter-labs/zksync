use super::*;
use super::ecc::{
    MontgomeryPoint,
    EdwardsPoint
};
use super::boolean::Boolean;
use ::jubjub::*;
use bellman::{
    ConstraintSystem
};
use super::lookup::*;
pub use pedersen_hash::Personalization;

impl Personalization {
    fn get_constant_bools(&self) -> Vec<Boolean> {
        self.get_bits()
        .into_iter()
        .map(|e| Boolean::constant(e))
        .collect()
    }
}

pub fn pedersen_hash<E: JubjubEngine, CS>(
    mut cs: CS,
    personalization: Personalization,
    bits: &[Boolean],
    params: &E::Params
) -> Result<EdwardsPoint<E>, SynthesisError>
    where CS: ConstraintSystem<E>
{
    let personalization = personalization.get_constant_bools();
    assert_eq!(personalization.len(), 6);

    let mut edwards_result = None;
    let mut bits = personalization.iter().chain(bits.iter());
    let mut segment_generators = params.pedersen_circuit_generators().iter();
    let boolean_false = Boolean::constant(false);

    let mut segment_i = 0;
    loop {
        let mut segment_result = None;
        let mut segment_windows = &segment_generators.next()
                                                     .expect("enough segments")[..];

        let mut window_i = 0;
        while let Some(a) = bits.next() {
            let b = bits.next().unwrap_or(&boolean_false);
            let c = bits.next().unwrap_or(&boolean_false);

            let tmp = lookup3_xy_with_conditional_negation(
                cs.namespace(|| format!("segment {}, window {}", segment_i, window_i)),
                &[a.clone(), b.clone(), c.clone()],
                &segment_windows[0]
            )?;

            let tmp = MontgomeryPoint::interpret_unchecked(tmp.0, tmp.1);

            match segment_result {
                None => {
                    segment_result = Some(tmp);
                },
                Some(ref mut segment_result) => {
                    *segment_result = tmp.add(
                        cs.namespace(|| format!("addition of segment {}, window {}", segment_i, window_i)),
                        segment_result,
                        params
                    )?;
                }
            }

            segment_windows = &segment_windows[1..];

            if segment_windows.len() == 0 {
                break;
            }

            window_i += 1;
        }

        match segment_result {
            Some(segment_result) => {
                // Convert this segment into twisted Edwards form.
                let segment_result = segment_result.into_edwards(
                    cs.namespace(|| format!("conversion of segment {} into edwards", segment_i)),
                    params
                )?;

                match edwards_result {
                    Some(ref mut edwards_result) => {
                        *edwards_result = segment_result.add(
                            cs.namespace(|| format!("addition of segment {} to accumulator", segment_i)),
                            edwards_result,
                            params
                        )?;
                    },
                    None => {
                        edwards_result = Some(segment_result);
                    }
                }
            },
            None => {
                // We didn't process any new bits.
                break;
            }
        }

        segment_i += 1;
    }

    Ok(edwards_result.unwrap())
}

#[cfg(test)]
mod test {
    use rand::{SeedableRng, Rng, XorShiftRng};
    use super::*;
    use ::circuit::test::*;
    use ::circuit::boolean::{Boolean, AllocatedBit};
    use pairing::bls12_381::{Bls12, Fr};
    use ff::PrimeField;

    #[test]
    fn test_pedersen_hash_constraints() {
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let params = &JubjubBls12::new();
        let mut cs = TestConstraintSystem::<Bls12>::new();

        let input: Vec<bool> = (0..(Fr::NUM_BITS * 2)).map(|_| rng.gen()).collect();

        let input_bools: Vec<Boolean> = input.iter().enumerate().map(|(i, b)| {
            Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| format!("input {}", i)), Some(*b)).unwrap()
            )
        }).collect();

        pedersen_hash(
            cs.namespace(|| "pedersen hash"),
            Personalization::NoteCommitment,
            &input_bools,
            params
        ).unwrap();

        assert!(cs.is_satisfied());
        assert_eq!(cs.num_constraints(), 1377);
    }

    #[test]
    fn test_pedersen_hash() {
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let params = &JubjubBls12::new();

        for length in 0..751 {
            for _ in 0..5 {
                let mut input: Vec<bool> = (0..length).map(|_| rng.gen()).collect();

                let mut cs = TestConstraintSystem::<Bls12>::new();

                let input_bools: Vec<Boolean> = input.iter().enumerate().map(|(i, b)| {
                    Boolean::from(
                        AllocatedBit::alloc(cs.namespace(|| format!("input {}", i)), Some(*b)).unwrap()
                    )
                }).collect();

                let res = pedersen_hash(
                    cs.namespace(|| "pedersen hash"),
                    Personalization::MerkleTree(1),
                    &input_bools,
                    params
                ).unwrap();

                assert!(cs.is_satisfied());

                let expected = ::pedersen_hash::pedersen_hash::<Bls12, _>(
                    Personalization::MerkleTree(1),
                    input.clone().into_iter(),
                    params
                ).into_xy();

                assert_eq!(res.get_x().get_value().unwrap(), expected.0);
                assert_eq!(res.get_y().get_value().unwrap(), expected.1);

                // Test against the output of a different personalization
                let unexpected = ::pedersen_hash::pedersen_hash::<Bls12, _>(
                    Personalization::MerkleTree(0),
                    input.into_iter(),
                    params
                ).into_xy();

                assert!(res.get_x().get_value().unwrap() != unexpected.0);
                assert!(res.get_y().get_value().unwrap() != unexpected.1);
            }
        }
    }
}

#[cfg(test)]
mod baby_test {
    use rand::{SeedableRng, Rng, XorShiftRng};
    use super::*;
    use ::circuit::test::*;
    use ::circuit::boolean::{Boolean, AllocatedBit};
    use pairing::bn256::{Bn256, Fr};
    use ff::PrimeField;
    use ::alt_babyjubjub::{AltJubjubBn256};

    #[test]
    fn test_pedersen_hash_constraints() {
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let params = &AltJubjubBn256::new();
        let mut cs = TestConstraintSystem::<Bn256>::new();

        let input: Vec<bool> = (0..(Fr::NUM_BITS * 2)).map(|_| rng.gen()).collect();

        let input_bools: Vec<Boolean> = input.iter().enumerate().map(|(i, b)| {
            Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| format!("input {}", i)), Some(*b)).unwrap()
            )
        }).collect();

        pedersen_hash(
            cs.namespace(|| "pedersen hash"),
            Personalization::NoteCommitment,
            &input_bools,
            params
        ).unwrap();

        assert!(cs.is_satisfied());
        assert_eq!(cs.num_constraints(), 1374);
    }

    #[test]
    fn test_pedersen_hash() {
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let params = &AltJubjubBn256::new();

        for length in 0..751 {
            for _ in 0..5 {
                let mut input: Vec<bool> = (0..length).map(|_| rng.gen()).collect();

                let mut cs = TestConstraintSystem::<Bn256>::new();

                let input_bools: Vec<Boolean> = input.iter().enumerate().map(|(i, b)| {
                    Boolean::from(
                        AllocatedBit::alloc(cs.namespace(|| format!("input {}", i)), Some(*b)).unwrap()
                    )
                }).collect();

                let res = pedersen_hash(
                    cs.namespace(|| "pedersen hash"),
                    Personalization::MerkleTree(1),
                    &input_bools,
                    params
                ).unwrap();

                assert!(cs.is_satisfied());

                let expected = ::pedersen_hash::pedersen_hash::<Bn256, _>(
                    Personalization::MerkleTree(1),
                    input.clone().into_iter(),
                    params
                ).into_xy();

                assert_eq!(res.get_x().get_value().unwrap(), expected.0);
                assert_eq!(res.get_y().get_value().unwrap(), expected.1);

                // Test against the output of a different personalization
                let unexpected = ::pedersen_hash::pedersen_hash::<Bn256, _>(
                    Personalization::MerkleTree(0),
                    input.into_iter(),
                    params
                ).into_xy();

                assert!(res.get_x().get_value().unwrap() != unexpected.0);
                assert!(res.get_y().get_value().unwrap() != unexpected.1);
            }
        }
    }
}
