use pairing::{Engine,};
use ff::{Field};
use super::*;
use super::num::{
    AllocatedNum,
    Num
};
use super::boolean::Boolean;
use bellman::{
    ConstraintSystem
};

// Synthesize the constants for each base pattern.
fn synth<'a, E: Engine, I>(
    window_size: usize,
    constants: I,
    assignment: &mut [E::Fr]
)
    where I: IntoIterator<Item=&'a E::Fr>
{
    assert_eq!(assignment.len(), 1 << window_size);

    for (i, constant) in constants.into_iter().enumerate() {
        let mut cur = assignment[i];
        cur.negate();
        cur.add_assign(constant);
        assignment[i] = cur;
        for (j, eval) in assignment.iter_mut().enumerate().skip(i + 1) {
            if j & i == i {
                eval.add_assign(&cur);
            }
        }
    }
}

/// Performs a 3-bit window table lookup. `bits` is in
/// little-endian order.
pub fn lookup3_xy<E: Engine, CS>(
    mut cs: CS,
    bits: &[Boolean],
    coords: &[(E::Fr, E::Fr)]
) -> Result<(AllocatedNum<E>, AllocatedNum<E>), SynthesisError>
    where CS: ConstraintSystem<E>
{
    assert_eq!(bits.len(), 3);
    assert_eq!(coords.len(), 8);

    // Calculate the index into `coords`
    let i =
    match (bits[0].get_value(), bits[1].get_value(), bits[2].get_value()) {
        (Some(a_value), Some(b_value), Some(c_value)) => {
            let mut tmp = 0;
            if a_value {
                tmp += 1;
            }
            if b_value {
                tmp += 2;
            }
            if c_value {
                tmp += 4;
            }
            Some(tmp)
        },
        _ => None
    };

    // Allocate the x-coordinate resulting from the lookup
    let res_x = AllocatedNum::alloc(
        cs.namespace(|| "x"),
        || {
            Ok(coords[*i.get()?].0)
        }
    )?;

    // Allocate the y-coordinate resulting from the lookup
    let res_y = AllocatedNum::alloc(
        cs.namespace(|| "y"),
        || {
            Ok(coords[*i.get()?].1)
        }
    )?;

    // Compute the coefficients for the lookup constraints
    let mut x_coeffs = [E::Fr::zero(); 8];
    let mut y_coeffs = [E::Fr::zero(); 8];
    synth::<E, _>(3, coords.iter().map(|c| &c.0), &mut x_coeffs);
    synth::<E, _>(3, coords.iter().map(|c| &c.1), &mut y_coeffs);

    let precomp = Boolean::and(cs.namespace(|| "precomp"), &bits[1], &bits[2])?;

    let one = CS::one();

    cs.enforce(
        || "x-coordinate lookup",
        |lc| lc + (x_coeffs[0b001], one)
                + &bits[1].lc::<E>(one, x_coeffs[0b011])
                + &bits[2].lc::<E>(one, x_coeffs[0b101])
                + &precomp.lc::<E>(one, x_coeffs[0b111]),
        |lc| lc + &bits[0].lc::<E>(one, E::Fr::one()),
        |lc| lc + res_x.get_variable()
                - (x_coeffs[0b000], one)
                - &bits[1].lc::<E>(one, x_coeffs[0b010])
                - &bits[2].lc::<E>(one, x_coeffs[0b100])
                - &precomp.lc::<E>(one, x_coeffs[0b110]),
    );

    cs.enforce(
        || "y-coordinate lookup",
        |lc| lc + (y_coeffs[0b001], one)
                + &bits[1].lc::<E>(one, y_coeffs[0b011])
                + &bits[2].lc::<E>(one, y_coeffs[0b101])
                + &precomp.lc::<E>(one, y_coeffs[0b111]),
        |lc| lc + &bits[0].lc::<E>(one, E::Fr::one()),
        |lc| lc + res_y.get_variable()
                - (y_coeffs[0b000], one)
                - &bits[1].lc::<E>(one, y_coeffs[0b010])
                - &bits[2].lc::<E>(one, y_coeffs[0b100])
                - &precomp.lc::<E>(one, y_coeffs[0b110]),
    );

    Ok((res_x, res_y))
}

/// Performs a 3-bit window table lookup, where
/// one of the bits is a sign bit.
pub fn lookup3_xy_with_conditional_negation<E: Engine, CS>(
    mut cs: CS,
    bits: &[Boolean],
    coords: &[(E::Fr, E::Fr)]
) -> Result<(Num<E>, Num<E>), SynthesisError>
    where CS: ConstraintSystem<E>
{
    assert_eq!(bits.len(), 3);
    assert_eq!(coords.len(), 4);

    // Calculate the index into `coords`
    let i =
    match (bits[0].get_value(), bits[1].get_value()) {
        (Some(a_value), Some(b_value)) => {
            let mut tmp = 0;
            if a_value {
                tmp += 1;
            }
            if b_value {
                tmp += 2;
            }
            Some(tmp)
        },
        _ => None
    };

    // Allocate the y-coordinate resulting from the lookup
    // and conditional negation
    let y = AllocatedNum::alloc(
        cs.namespace(|| "y"),
        || {
            let mut tmp = coords[*i.get()?].1;
            if *bits[2].get_value().get()? {
                tmp.negate();
            }
            Ok(tmp)
        }
    )?;

    let one = CS::one();

    // Compute the coefficients for the lookup constraints
    let mut x_coeffs = [E::Fr::zero(); 4];
    let mut y_coeffs = [E::Fr::zero(); 4];
    synth::<E, _>(2, coords.iter().map(|c| &c.0), &mut x_coeffs);
    synth::<E, _>(2, coords.iter().map(|c| &c.1), &mut y_coeffs);

    let precomp = Boolean::and(cs.namespace(|| "precomp"), &bits[0], &bits[1])?;

    let x = Num::zero()
            .add_bool_with_coeff(one, &Boolean::constant(true), x_coeffs[0b00])
            .add_bool_with_coeff(one, &bits[0], x_coeffs[0b01])
            .add_bool_with_coeff(one, &bits[1], x_coeffs[0b10])
            .add_bool_with_coeff(one, &precomp, x_coeffs[0b11]);

    let y_lc = precomp.lc::<E>(one, y_coeffs[0b11]) +
               &bits[1].lc::<E>(one, y_coeffs[0b10]) +
               &bits[0].lc::<E>(one, y_coeffs[0b01]) +
               (y_coeffs[0b00], one);

    cs.enforce(
        || "y-coordinate lookup",
        |lc| lc + &y_lc + &y_lc,
        |lc| lc + &bits[2].lc::<E>(one, E::Fr::one()),
        |lc| lc + &y_lc - y.get_variable()
    );

    Ok((x, y.into()))
}

#[cfg(test)]
mod test {
    use rand::{SeedableRng, Rand, Rng, XorShiftRng};
    use super::*;
    use ::circuit::test::*;
    use ::circuit::boolean::{Boolean, AllocatedBit};
    use pairing::bls12_381::{Bls12, Fr};

    #[test]
    fn test_lookup3_xy() {
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0656]);

        for _ in 0..100 {
            let mut cs = TestConstraintSystem::<Bls12>::new();

            let a_val = rng.gen();
            let a = Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| "a"), Some(a_val)).unwrap()
            );

            let b_val = rng.gen();
            let b = Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| "b"), Some(b_val)).unwrap()
            );

            let c_val = rng.gen();
            let c = Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| "c"), Some(c_val)).unwrap()
            );

            let bits = vec![a, b, c];

            let points: Vec<(Fr, Fr)> = (0..8).map(|_| (rng.gen(), rng.gen())).collect();

            let res = lookup3_xy(&mut cs, &bits, &points).unwrap();

            assert!(cs.is_satisfied());

            let mut index = 0;
            if a_val { index += 1 }
            if b_val { index += 2 }
            if c_val { index += 4 }

            assert_eq!(res.0.get_value().unwrap(), points[index].0);
            assert_eq!(res.1.get_value().unwrap(), points[index].1);
        }
    }

    #[test]
    fn test_lookup3_xy_with_conditional_negation() {
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

        for _ in 0..100 {
            let mut cs = TestConstraintSystem::<Bls12>::new();

            let a_val = rng.gen();
            let a = Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| "a"), Some(a_val)).unwrap()
            );

            let b_val = rng.gen();
            let b = Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| "b"), Some(b_val)).unwrap()
            );

            let c_val = rng.gen();
            let c = Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| "c"), Some(c_val)).unwrap()
            );

            let bits = vec![a, b, c];

            let points: Vec<(Fr, Fr)> = (0..4).map(|_| (rng.gen(), rng.gen())).collect();

            let res = lookup3_xy_with_conditional_negation(&mut cs, &bits, &points).unwrap();

            assert!(cs.is_satisfied());

            let mut index = 0;
            if a_val { index += 1 }
            if b_val { index += 2 }

            assert_eq!(res.0.get_value().unwrap(), points[index].0);
            let mut tmp = points[index].1;
            if c_val { tmp.negate() }
            assert_eq!(res.1.get_value().unwrap(), tmp);
        }
    }

    #[test]
    fn test_synth() {
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

        let window_size = 4;

        let mut assignment = vec![Fr::zero(); 1 << window_size];
        let constants: Vec<_> = (0..(1 << window_size)).map(|_| Fr::rand(&mut rng)).collect();

        synth::<Bls12, _>(window_size, &constants, &mut assignment);

        for b in 0..(1 << window_size) {
            let mut acc = Fr::zero();

            for j in 0..(1 << window_size) {
                if j & b == j {
                    acc.add_assign(&assignment[j]);
                }
            }

            assert_eq!(acc, constants[b]);
        }
    }
}
