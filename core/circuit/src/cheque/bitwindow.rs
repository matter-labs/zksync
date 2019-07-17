use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField};
use franklin_crypto::circuit::num::{AllocatedNum, Num};
use franklin_crypto::circuit::polynomial_lookup::{do_the_lookup, generate_powers};
use franklin_crypto::circuit::{boolean, Assignment};
use franklin_crypto::jubjub::JubjubEngine;

#[derive(Clone)]
pub struct BitWindowWitness<E: JubjubEngine> {
    // Bits in the current window
    pub bits: Option<E::Fr>,
    // Start of the current window
    pub start: Option<E::Fr>,
}

#[derive(Clone)]
pub struct BitNumber<E: JubjubEngine> {
    // Bit number to set
    pub number: Option<E::Fr>,
}

pub struct BitSet<'a, E: JubjubEngine> {
    pub params: &'a E::Params,

    pub action: (BitNumber<E>, BitWindowWitness<E>),
}

// fn print_boolean_vector(vector: &[boolean::Boolean]) {
//     for b in vector {
//         if b.get_value().unwrap() {
//             print!("1");
//         } else {
//             print!("0");
//         }
//     }
//     print!("\n");
// }

// generate a set of lookup polynomials
// - first one outputs a bitmask for a shift (shift by 1 bit -> mask is 0x000000....1 as Fr)
// - second one outputs just a bit 1 or 0 that tells that this distance is correct (outputs 1 or 0)

// make a polynomial that it's for a range of `i` [min_with_shift, max_with_shift] outputs `2^(min_with_shift-1)`,
// forcing to make a lookup in the highest bit, and for i in [min_no_shift, min_with_shift) outputs `2^i` - actual bit
fn generate_bit_lookup_polynomial<E: JubjubEngine>(
    min_no_shift: u128,
    min_with_shift: u128,
    max_with_shift: u128,
) -> Vec<E::Fr> {
    use franklin_crypto::interpolation::interpolate;

    let mut points: Vec<(E::Fr, E::Fr)> = vec![];
    let two = E::Fr::from_str("2").unwrap();
    let mut power = E::Fr::one();
    for i in min_no_shift..(min_with_shift - 1) {
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = power;
        points.push((x, y));

        power.mul_assign(&two);
    }

    let x = E::Fr::from_str(&(min_with_shift - 1).to_string()).unwrap();
    let y = power;
    points.push((x, y));

    for i in min_with_shift..=max_with_shift {
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = power;
        points.push((x, y));
    }
    let interpolation = interpolate::<E>(&points[..]).expect("must interpolate");
    assert_eq!(interpolation.len(), (max_with_shift + 1) as usize);

    interpolation
}

// make a polynomial that it's for a range of `i` in [min_with_shift, max_with_shift] outputs `2^(i+1-min_with_shift) - 1`,
// and for [min_no_shift, min_with_shift) outputs 0
fn generate_shift_bitmask_polynomial<E: JubjubEngine>(
    min_no_shift: u128,
    min_with_shift: u128,
    max_with_shift: u128,
) -> Vec<E::Fr> {
    use franklin_crypto::interpolation::interpolate;

    let mut points: Vec<(E::Fr, E::Fr)> = vec![];
    for i in min_no_shift..min_with_shift {
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::zero();
        points.push((x, y));
    }

    // the min_with_shift already requires shift by 1, so bitmask is 0x00000...0001
    let two = E::Fr::from_str("2").unwrap();
    let mut power = E::Fr::one();
    for i in min_with_shift..=max_with_shift {
        power.mul_assign(&two);

        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let mut y = power;
        y.sub_assign(&E::Fr::one());
        points.push((x, y));
    }
    let interpolation = interpolate::<E>(&points[..]).expect("must interpolate");
    assert_eq!(interpolation.len(), (max_with_shift + 1) as usize);

    interpolation
}

// make a lookup with 0 or 1 indicating if distance is correct in [min_valid, max_valid] and 0 in (max_valid, full_range]
fn generate_correctness_polynomial<E: JubjubEngine>(
    min_valid: u128,
    max_valid: u128,
    full_range: u128,
) -> Vec<E::Fr> {
    use franklin_crypto::interpolation::interpolate;

    let mut points: Vec<(E::Fr, E::Fr)> = vec![];
    for i in min_valid..=max_valid {
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::one();
        points.push((x, y));
    }

    for i in (max_valid + 1)..=full_range {
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::zero();
        points.push((x, y));
    }

    let interpolation = interpolate::<E>(&points[..]).expect("must interpolate");
    assert_eq!(interpolation.len(), (full_range + 1) as usize);

    interpolation
}

// make a polynomial that it's for a range of `i` in [min_with_shift, max_with_shift] outputs `2^(i+1-min_with_shift) - 1`,
// and for [min_no_shift, min_with_shift) outputs 0
fn generate_start_adjustment_polynomial<E: JubjubEngine>(
    min_no_shift: u128,
    min_with_shift: u128,
    max_with_shift: u128,
) -> Vec<E::Fr> {
    use franklin_crypto::interpolation::interpolate;

    let mut points: Vec<(E::Fr, E::Fr)> = vec![];
    for i in min_no_shift..min_with_shift {
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = E::Fr::zero();
        points.push((x, y));
    }

    let mut shift = E::Fr::one();
    for i in min_with_shift..=max_with_shift {
        let x = E::Fr::from_str(&i.to_string()).unwrap();
        let y = shift;
        points.push((x, y));

        shift.add_assign(&E::Fr::one());
    }
    let interpolation = interpolate::<E>(&points[..]).expect("must interpolate");
    assert_eq!(interpolation.len(), (max_with_shift + 1) as usize);

    interpolation
}

impl<'a, E: JubjubEngine> Circuit<E> for BitSet<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let bitfield_length = 128u128;
        let shift_length = 64u128;
        // let log_shift_length = 6;

        // distance in range [0, 127] is without shift
        // distance [128, 191] is with shift
        let max_valid_distance = bitfield_length + shift_length - 1u128;
        let lookup_polynomial_length = bitfield_length * 2;
        let lookup_argument_bit_width = 8;

        let (bit_number, witness) = self.action;
        let current_bits_fe =
            AllocatedNum::alloc(cs.namespace(|| "allocate bits witness"), || {
                Ok(*witness.bits.get()?)
            })?;

        // current bits is in the field and is of predefined bit length
        current_bits_fe.limit_number_of_bits(
            cs.namespace(|| "limit number of bits of the bitfield"),
            bitfield_length as usize,
        )?;

        let start = AllocatedNum::alloc(cs.namespace(|| "allocate window start witness"), || {
            Ok(*witness.start.get()?)
        })?;

        let two_inverted = E::Fr::from_str("2").unwrap().inverse().unwrap();

        let current_bits = current_bits_fe.into_bits_le(cs.namespace(|| "get current bits"))?;

        let bit_number = AllocatedNum::alloc(cs.namespace(|| "allocate bit number"), || {
            Ok(*bit_number.number.get()?)
        })?;

        start.limit_number_of_bits(cs.namespace(|| "limit start as 2^32"), 32)?;

        let distance = AllocatedNum::alloc(cs.namespace(|| "allocate distance"), || {
            let mut num = *bit_number.get_value().get()?;
            let start = *start.get_value().get()?;
            num.sub_assign(&start);

            Ok(num)
        })?;

        cs.enforce(
            || "enforce distance calculation",
            |lc| lc + distance.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + bit_number.get_variable() - start.get_variable(),
        );

        // distance must be smaller than 128*2 as a first limit for further use of polynomial tricks
        distance.limit_number_of_bits(
            cs.namespace(|| "limit distance as 2^8"),
            lookup_argument_bit_width,
        )?;

        let distance_powers = generate_powers(
            cs.namespace(|| "generate powers of distance variable"),
            &distance,
            lookup_polynomial_length as usize,
        )?;

        assert_eq!(distance_powers.len(), lookup_polynomial_length as usize);

        let bit_lookup_coeffs =
            generate_bit_lookup_polynomial::<E>(0u128, bitfield_length, max_valid_distance);
        assert_eq!(
            bit_lookup_coeffs.len(),
            (bitfield_length + shift_length) as usize
        );

        let mask_lookup_coeffs =
            generate_shift_bitmask_polynomial::<E>(0u128, bitfield_length, max_valid_distance);
        assert_eq!(
            mask_lookup_coeffs.len(),
            (bitfield_length + shift_length) as usize
        );

        let valid_distance_lookup_coeffs = generate_correctness_polynomial::<E>(
            0u128,
            bitfield_length + shift_length - 1u128,
            (1u128 << lookup_argument_bit_width) - 1u128,
        );
        assert_eq!(
            valid_distance_lookup_coeffs.len(),
            lookup_polynomial_length as usize
        );

        let start_adjustment_lookup_coeffs =
            generate_start_adjustment_polynomial::<E>(0u128, bitfield_length, max_valid_distance);
        assert_eq!(
            start_adjustment_lookup_coeffs.len(),
            (bitfield_length + shift_length) as usize
        );

        let is_valid = do_the_lookup(
            cs.namespace(|| "lookup distance validity"),
            &valid_distance_lookup_coeffs,
            &distance_powers,
        )?;

        let mask_fe = do_the_lookup(
            cs.namespace(|| "lookup shift mask"),
            &mask_lookup_coeffs,
            &distance_powers[0..((bitfield_length + shift_length) as usize)],
        )?;

        let bit_position_fe = do_the_lookup(
            cs.namespace(|| "lookup bit position"),
            &bit_lookup_coeffs,
            &distance_powers[0..((bitfield_length + shift_length) as usize)],
        )?;

        cs.enforce(
            || "enforce distance is valid",
            |lc| lc + is_valid.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + CS::one(),
        );

        let mask_bits = mask_fe.into_bits_le(cs.namespace(|| "bitshift mask bit decomposition"))?;

        // current_bits.truncate(bitfield_length as usize);
        // mask_bits.truncate(bitfield_length as usize);

        let mut masked_bits: Vec<boolean::Boolean> = vec![];
        assert_eq!(current_bits.len(), mask_bits.len());

        // mask the field bits to further make a shift. This is basically bitfield mod 2^shift
        for (i, (mask_bit, field_bit)) in mask_bits.iter().zip(current_bits.iter()).enumerate() {
            let bit = boolean::Boolean::and(
                cs.namespace(|| format!("mask the field bit {}", i)),
                mask_bit,
                field_bit,
            )?;

            masked_bits.push(bit);
        }

        // repack remainder before shifting
        let mut masked_lc = Num::<E>::zero();
        let mut coeff = E::Fr::one();
        for bit in &masked_bits {
            masked_lc = masked_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
            coeff.double();
        }

        let remainder = AllocatedNum::alloc(
            cs.namespace(|| "allocate the remainder after bitmask"),
            || Ok(*masked_lc.get_value().get()?),
        )?;

        cs.enforce(
            || "pack the remainder after bitmasking",
            |lc| lc + remainder.get_variable(),
            |lc| lc + CS::one(),
            |_| masked_lc.lc(E::Fr::one()),
        );

        let quotient = AllocatedNum::alloc(
            cs.namespace(|| "allocate top field bits after masking"),
            || {
                let mut initial = *current_bits_fe.get_value().get()?;
                let masked = *remainder.get_value().get()?;
                initial.sub_assign(&masked);

                Ok(initial)
            },
        )?;

        cs.enforce(
            || "enforce top bits after masking",
            |lc| lc + quotient.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + current_bits_fe.get_variable() - remainder.get_variable(),
        );

        let mut shifted_quotient = quotient.clone();

        // do the bitshifting
        for (i, mask_bit) in mask_bits.iter().enumerate() {
            let multiplier = AllocatedNum::alloc(
                cs.namespace(|| format!("allocate bitshift multiplier {}", i)),
                || {
                    let bval = *mask_bit.get_value().get()?;
                    if !bval {
                        Ok(E::Fr::one())
                    } else {
                        Ok(two_inverted)
                    }
                },
            )?;

            // b*two_inv + (1 - b) * 1 = b(two_inv - i) - 1

            let mut c = two_inverted;
            c.sub_assign(&E::Fr::one());

            cs.enforce(
                || format!("enforce multiplier selection {}", i),
                |lc| lc + multiplier.get_variable(),
                |lc| lc + CS::one(),
                |_| mask_bit.lc::<E>(CS::one(), c) + CS::one(),
            );

            shifted_quotient = shifted_quotient
                .mul(cs.namespace(|| format!("do the shift {}", i)), &multiplier)?;
        }

        // decompose the resulting register state

        let shifted_bits =
            shifted_quotient.into_bits_le(cs.namespace(|| "get shifted register bits"))?;

        let bit_position_bits =
            bit_position_fe.into_bits_le(cs.namespace(|| "get bit of interest mask bits"))?;

        for (i, (reg_bit, position_bit)) in shifted_bits
            .iter()
            .zip(bit_position_bits.iter())
            .enumerate()
        {
            // enforce that bit is not set
            // reg_bit * lookup_bit = 0
            // reg_bit = 1, lookup_bit = 0 -> valid
            // reg_bit = 0, lookup_bit = 0 -> valid
            // reg_bit = 0, lookup_bit = 1 -> valid, bit is not set
            // reg_bit = 1, lookup_bit = 1 -> invalid, bit is already set
            cs.enforce(
                || format!("enforce shifted register bit is not set, iteraction {}", i),
                |_| reg_bit.lc::<E>(CS::one(), E::Fr::one()),
                |_| position_bit.lc::<E>(CS::one(), E::Fr::one()),
                |lc| lc,
            );
        }

        // make a final register state

        let new_register = AllocatedNum::alloc(cs.namespace(|| "allocate new register"), || {
            let mut new_val = *shifted_quotient.get_value().get()?;
            let position = *bit_position_fe.get_value().get()?;
            new_val.add_assign(&position);

            Ok(new_val)
        })?;

        cs.enforce(
            || "enforce new register",
            |lc| lc + new_register.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + shifted_quotient.get_variable() + bit_position_fe.get_variable(),
        );

        let start_adjustment = do_the_lookup(
            cs.namespace(|| "create start adjustment"),
            &start_adjustment_lookup_coeffs,
            &distance_powers[0..((bitfield_length + shift_length) as usize)],
        )?;

        // start_adjustment.limit_number_of_bits(
        //     cs.namespace(|| "limit number of bits in the start change"),
        //     log_shift_length + 1
        // )?;

        let new_start = AllocatedNum::alloc(cs.namespace(|| "allocate new start"), || {
            let mut new_val = *start.get_value().get()?;
            let shift = *start_adjustment.get_value().get()?;
            new_val.add_assign(&shift);

            Ok(new_val)
        })?;

        cs.enforce(
            || "enforce new start",
            |lc| lc + new_start.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + start.get_variable() + start_adjustment.get_variable(),
        );

        new_start
            .limit_number_of_bits(cs.namespace(|| "limit number of bits for a new start"), 32)?;

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use super::*;

    use bellman::groth16::{
        create_random_proof, generate_random_parameters, prepare_verifying_key, verify_proof,
    };
    use ff::{BitIterator, Field, PrimeField};
    use franklin_crypto::{
        alt_babyjubjub::AltJubjubBn256, circuit::test::*, interpolation::evaluate_at_x,
    };
    use pairing::bn256::*;
    use rand::{Rng, SeedableRng, XorShiftRng};

    #[test]
    fn test_redeem() {
        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);

        let params = &AltJubjubBn256::new();

        let bitfield_length = 128u128;
        let shift_length = 64u128;
        let max_valid_distance = bitfield_length + shift_length - 1u128;

        let start = 0u128;

        for bit_of_interest in 0u128..=max_valid_distance {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let bottom_bits: u64 = rng.gen();
            let top_bits: u64 = rng.gen();

            let mut existing_field: u128 = (u128::from(top_bits) << 64) + u128::from(bottom_bits);
            if bit_of_interest < bitfield_length {
                let mask = 1u128 << bit_of_interest;

                if existing_field & mask > 0 {
                    existing_field -= mask;
                }
                assert!(existing_field & mask == 0u128);
            }

            let witness = BitWindowWitness {
                bits: Fr::from_str(&existing_field.to_string()),

                start: Fr::from_str(&start.to_string()),
            };

            let number = BitNumber {
                number: Fr::from_str(&bit_of_interest.to_string()),
            };

            let instance = BitSet {
                params,
                action: (number, witness),
            };

            instance.synthesize(&mut cs).expect("must synthesize");

            println!("{}", cs.find_unconstrained());

            println!("{}", cs.num_constraints());

            assert_eq!(cs.num_inputs(), 1);

            let err = cs.which_is_unsatisfied();
            if err.is_some() {
                println!(
                    "Error for bitfield = {:#b}, bit of interest = {}",
                    existing_field, bit_of_interest
                );
                panic!("ERROR satisfying in {}", err.unwrap());
            } else {
                println!("Satisfied for bit = {}", bit_of_interest);
            }
        }
    }

    #[test]
    fn test_proof_generation() {
        let mut rng =
            &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);

        let params = &AltJubjubBn256::new();

        let bitfield_length = 128u128;
        let shift_length = 64u128;
        let _max_valid_distance = bitfield_length + shift_length - 1u128;

        let start = 0u128;

        let bit_of_interest = 129u128;

        let bottom_bits: u64 = rng.gen();
        let top_bits: u64 = rng.gen();

        let mut existing_field: u128 = (u128::from(top_bits) << 64) + u128::from(bottom_bits);
        if bit_of_interest < bitfield_length {
            let mask = 1u128 << bit_of_interest;

            if existing_field & mask > 0 {
                existing_field -= mask;
            }
            assert!(existing_field & mask == 0u128);
        }

        let witness = BitWindowWitness {
            bits: Fr::from_str(&existing_field.to_string()),

            start: Fr::from_str(&start.to_string()),
        };

        let number = BitNumber {
            number: Fr::from_str(&bit_of_interest.to_string()),
        };

        let instance = BitSet {
            params,

            action: (number, witness),
        };

        let parameters = {
            let w = BitWindowWitness::<Bn256> {
                bits: None,
                start: None,
            };

            let n = BitNumber::<Bn256> { number: None };

            let inst = BitSet::<Bn256> {
                params,

                action: (n, w),
            };

            generate_random_parameters::<Bn256, _, _>(inst, &mut rng)
                .expect("must generate parameters")
        };

        let proof = create_random_proof::<Bn256, _, _, _>(instance, &parameters, &mut rng)
            .expect("must generate proof");

        let pvk = prepare_verifying_key(&parameters.vk);

        let inputs: Vec<Fr> = vec![];

        let valid = verify_proof(&pvk, &proof, &inputs).expect("must verify proof");

        assert!(valid);
    }

    #[test]
    fn test_bit_shifts() {
        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let mut bitmask: Fr = rng.gen();
        let power_of_two = 8;
        let mut pow = Fr::one();
        let two = Fr::from_str("2").unwrap();
        for _ in 0..power_of_two {
            pow.mul_assign(&two);
        }

        let pow_bits: Vec<bool> = BitIterator::new(pow.into_repr()).collect();
        for b in pow_bits {
            if b {
                print!("1");
            } else {
                print!("0");
            }
        }
        println!();

        let bits_before: Vec<bool> = BitIterator::new(bitmask.into_repr()).collect();
        for b in bits_before {
            if b {
                print!("1");
            } else {
                print!("0");
            }
        }
        println!();

        pow = pow.inverse().unwrap();

        bitmask.mul_assign(&pow);

        let bits_after: Vec<bool> = BitIterator::new(bitmask.into_repr()).collect();
        for b in bits_after {
            if b {
                print!("1");
            } else {
                print!("0");
            }
        }
        println!();
    }

    #[test]
    fn test_bitmask_lookups() {
        let min_no_shift = 0u128;
        let min_with_shift = 128u128;
        let max_with_shift = 255u128;

        let interpolation = generate_shift_bitmask_polynomial::<Bn256>(
            min_no_shift,
            min_with_shift,
            max_with_shift,
        );

        for i in 0..=max_with_shift {
            let x = Fr::from_str(&i.to_string()).unwrap();
            let val = evaluate_at_x::<Bn256>(&interpolation[..], &x);
            println!("X = {}, Y = {}", x, val);
        }
    }

    #[test]
    fn test_check_bit_lookups() {
        let min_no_shift = 0u128;
        let min_with_shift = 128u128;
        let max_with_shift = 255u128;

        let interpolation =
            generate_bit_lookup_polynomial::<Bn256>(min_no_shift, min_with_shift, max_with_shift);

        for i in 0..=max_with_shift {
            let x = Fr::from_str(&i.to_string()).unwrap();
            let val = evaluate_at_x::<Bn256>(&interpolation[..], &x);
            println!("X = {}, Y = {}", x, val);
        }
    }

    #[test]
    fn test_validity_lookups() {
        let min_valid = 0u128;
        let max_valid = 128u128 + 64u128;
        let full_range = 255u128;

        let interpolation =
            generate_correctness_polynomial::<Bn256>(min_valid, max_valid, full_range);

        for i in 0..=full_range {
            let x = Fr::from_str(&i.to_string()).unwrap();
            let val = evaluate_at_x::<Bn256>(&interpolation[..], &x);
            println!("X = {}, Y = {}", x, val);
        }
    }
}
