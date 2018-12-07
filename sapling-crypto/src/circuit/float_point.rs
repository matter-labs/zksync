use pairing::{Engine};
use ff::{Field, PrimeField};
use bellman::{ConstraintSystem, SynthesisError};
use super::boolean::{Boolean};
use super::num::{AllocatedNum, Num};
use super::Assignment;

/// Takes a bit decomposition, parses and packs into an AllocatedNum
/// If exponent is equal to zero, then exponent multiplier is equal to 1
pub fn parse_with_exponent_le<E: Engine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    bits: &[Boolean],
    exponent_length: usize,
    mantissa_length: usize,
    exponent_base: u64
) -> Result<AllocatedNum<E>, SynthesisError>
{
    assert!(bits.len() == exponent_length + mantissa_length);

    let one_allocated = AllocatedNum::alloc(
        cs.namespace(|| "allocate one"),
        || Ok(E::Fr::one())
    )?;

    let mut exponent_result = AllocatedNum::alloc(
        cs.namespace(|| "allocate exponent result"),
        || Ok(E::Fr::one())
    )?;

    let exponent_base_string = exponent_base.to_string();
    let exponent_base_value = E::Fr::from_str(&exponent_base_string.clone()).unwrap();

    let mut exponent_base = AllocatedNum::alloc(
        cs.namespace(|| "allocate exponent base"), 
        || Ok(exponent_base_value)
    )?;

    let exponent_value = exponent_base_value;

    for i in 0..exponent_length {
        let thisbit = &bits[i];

        let multiplier = AllocatedNum::conditionally_select(
            cs.namespace(|| format!("select exponent multiplier {}", i)),
            &exponent_base, 
            &one_allocated, 
            &thisbit
        )?;

        exponent_result = exponent_result.mul(
            cs.namespace(|| format!("make exponent result {}", i)),
            &multiplier
        )?;

        exponent_base = exponent_base.clone().square(
            cs.namespace(|| format!("make exponent base {}", i))
        )?;

        // exponent_base = exponent_base.mul(
        //     cs.namespace(|| format!("make exponent base {}", i)), 
        //     &exponent_base.clone()
        // )?;
    }

    let mut mantissa_result = Num::<E>::zero();
    let mut mantissa_base = E::Fr::one();

    for i in exponent_length..(exponent_length+mantissa_length)
    {
        let thisbit = &bits[i];
        mantissa_result = mantissa_result.add_bool_with_coeff(CS::one(), &thisbit, mantissa_base);
        mantissa_base.double();
    }

    let mantissa = AllocatedNum::alloc(
        cs.namespace(|| "allocating mantissa"),
        || Ok(*mantissa_result.get_value().get()?)
    )?;

    mantissa.mul(
        cs.namespace(|| "calculate floating point result"),
        &exponent_result
    )

    // return 

    // let mut result = mantissa_result.get_value().get()?.clone();

    // let exponent_value = exponent_result.get_value().get()?.clone();

    // result.mul_assign(&exponent_value);

    // let result_allocated = AllocatedNum::alloc(
    //     cs.namespace(|| "float point parsing result"),
    //     || Ok(result)
    // )?;

    // // num * 1 = input
    // cs.enforce(
    //     || "float point result constraint",
    //     |lc| lc + exponent_result.get_variable(),
    //     |_| mantissa_result.lc(E::Fr::one()),
    //     |lc| lc + result_allocated.get_variable()
    // );

    // Ok(result_allocated)
}

pub fn convert_to_float(
    integer: u128,
    exponent_length: usize,
    mantissa_length: usize,
    exponent_base: u32
) -> Result<Vec<bool>, SynthesisError>
{
    let exponent_base = u128::from(exponent_base);
    let mut max_exponent = 1u128;
    let max_power = (1 << exponent_length) - 1;

    for _ in 0..max_power
    {
        max_exponent = max_exponent * exponent_base;
    }

    let max_mantissa = (1u128 << mantissa_length) - 1;
    
    if integer > (max_mantissa * max_exponent) {
        return Err(SynthesisError::Unsatisfiable)
    }
    // always try best precision
    let exponent_guess = integer / max_mantissa;
    let mut exponent_temp = exponent_guess;
    let mut exponent: usize = 0;
    loop {
        if exponent_temp < exponent_base {
            break
        }
        exponent_temp = exponent_temp / exponent_base;
        exponent += 1;
    }

    exponent_temp = 1u128;
    for _ in 0..exponent 
    {
        exponent_temp = exponent_temp * exponent_base;
    }    

    if exponent_temp * max_mantissa < integer 
    {
        exponent += 1;
        exponent_temp = exponent_temp * exponent_base;
    }

    let mantissa = integer / exponent_temp;

    // encode into bits. First bits of mantissa in LE order

    let mut encoding = vec![];

    for i in 0..exponent_length {
        if exponent & (1 << i) != 0 {
            encoding.extend(&[true; 1]);
        } else {
            encoding.extend(&[false; 1]);
        }
    }

    for i in 0..mantissa_length {
        if mantissa & (1 << i) != 0 {
            encoding.extend(&[true; 1]);
        } else {
            encoding.extend(&[false; 1]);
        }
    }

    assert!(encoding.len() == exponent_length + mantissa_length);

    Ok(encoding)
}

pub fn parse_float_to_u128(
    encoding: Vec<bool>,
    exponent_length: usize,
    mantissa_length: usize,
    exponent_base: u32
) -> Result<u128, SynthesisError>
{
    assert!(exponent_length + mantissa_length == encoding.len());

    let exponent_base = u128::from(exponent_base);
    let mut exponent_multiplier = exponent_base;
    let mut exponent = 1u128;
    let bitslice: &[bool] = &encoding;
    for i in 0..exponent_length
    {
        if bitslice[i] {
            let max_exponent = (u128::max_value() / exponent_multiplier) + 1;
            if exponent >= max_exponent {
                return Err(SynthesisError::Unsatisfiable)
            }
            exponent = exponent * exponent_multiplier;
        }
        exponent_multiplier = exponent_multiplier * exponent_multiplier;
    }

    let mut max_mantissa = u128::max_value();
    if exponent != 1u128 {
        max_mantissa = (u128::max_value() / exponent) + 1;
    }

    let mut mantissa_power = 1u128;
    let mut mantissa = 0u128;
    for i in exponent_length..(exponent_length + mantissa_length)
    {
        if bitslice[i] {
            let max_mant = (max_mantissa / 2u128) + 1;
            if mantissa >= max_mant {
                return Err(SynthesisError::Unsatisfiable)
            }
            mantissa = mantissa + mantissa_power;
        }
        mantissa_power = mantissa_power * 2u128;
    }

    let result = mantissa * exponent;

    Ok(result)
}

#[test]
fn test_parsing() {
    use rand::{SeedableRng, Rng, XorShiftRng};
    use bellman::{ConstraintSystem};
    use pairing::bn256::{Bn256};
    use ::circuit::test::*;
    use super::boolean::{AllocatedBit, Boolean};

    let rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let mut cs = TestConstraintSystem::<Bn256>::new();

    // exp = 1  
    // let bits: Vec<bool> = vec![false, false, false, false, false, true];

    // exp = 10
    // let bits: Vec<bool> = vec![true, false, false, false, false, true];

    // exp = 1000 = 10^3
    // let bits: Vec<bool> = vec![true, true, false, false, false, true];

    // exp = 10^7 = 10000000
    // let bits: Vec<bool> = vec![true, true, true, false, false, true];

    // exp = 10^15 = 1000000000000000
    let bits: Vec<bool> = vec![true, true, true, true, false, true];

    // let bits: Vec<bool> = vec![true, true, true, true, true, true];

    let circuit_bits = bits.iter().enumerate()
                            .map(|(i, &b)| {
                                Boolean::from(
                                    AllocatedBit::alloc(
                                    cs.namespace(|| format!("bit {}", i)),
                                    Some(b)
                                    ).unwrap()
                                )
                            })
                            .collect::<Vec<_>>();

    let exp_result = parse_with_exponent_le(cs.namespace(|| "parse"), &circuit_bits, 5, 1, 10).unwrap();

    print!("{}\n", exp_result.get_value().unwrap().into_repr());
    assert!(cs.is_satisfied());
    print!("constraints for float parsing = {}\n", cs.num_constraints());
}

#[test]
fn test_encoding() {
    use rand::{SeedableRng, Rng, XorShiftRng};
    let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    // max encoded value is 10^31 * 2047 ~ 10^34 ~ 112 bits

    for _ in 0..1000 {
        let top_word = rng.next_u64() & 0x0000ffffffffffff;
        let bottom_word = rng.next_u64();
        let integer = (u128::from(top_word) << 64) + u128::from(bottom_word);

        let encoding = convert_to_float(integer, 5, 11, 10);

        assert!(encoding.is_ok());

        let decoded = parse_float_to_u128(encoding.unwrap(), 5, 11, 10);

        assert!(decoded.is_ok());

        let dec = decoded.unwrap();

        assert!(integer/dec == 1u128);
        assert!(dec/integer <= 1u128);
    }
}

#[test]
fn test_encoding_powers_of_two() {
    use rand::{SeedableRng, Rng, XorShiftRng};
    let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let mantissa_length = 11;

    for i in 0..mantissa_length {
        let mantissa = 1u128 << i;
        let encoding = convert_to_float(mantissa, 5, mantissa_length, 10).unwrap();
        for (j, bit) in encoding.into_iter().enumerate() {
            if j != 5 + i  {
                assert!(!bit);
            } else {
                assert!(bit);
            }
        }
    }
}
