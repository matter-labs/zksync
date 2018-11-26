use pairing::{Engine,};
use ff::{Field, PrimeField, PrimeFieldRepr};
use bellman::{ConstraintSystem, SynthesisError};
use super::boolean::{Boolean};
use super::num::{AllocatedNum, Num};
use super::Assignment;

/// Takes a bit decomposition, parses and packs into an AllocatedNum
pub fn parse_with_exponent_le<E, CS>(
    mut cs: CS,
    bits: &[Boolean],
    exponent_length: usize,
    mantissa_length: usize,
    exponent_base: usize
) -> Result<AllocatedNum<E>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>
{
    assert!(bits.len() == exponent_length + mantissa_length);

    let mut exponent_result = Num::<E>::zero();
    let mut exp_base = E::Fr::from_str("10").unwrap();

    for i in 0..exponent_length
    {
        let thisbit = &bits[i];
        exponent_result = exponent_result.add_bool_with_coeff(CS::one(), &thisbit, exp_base);
        exp_base.square();
    }

    let mut mantissa_result = Num::<E>::zero();
    let mut mantissa_base = E::Fr::one();

    for i in exponent_length..(exponent_length+mantissa_length)
    {
        let thisbit = &bits[i];
        mantissa_result = mantissa_result.add_bool_with_coeff(CS::one(), &thisbit, mantissa_base);
        mantissa_base.double();
    }

    let mut result = mantissa_result.get_value().unwrap();
    result.mul_assign(&exponent_result.get_value().unwrap());
    let result_allocated = AllocatedNum::alloc(cs.namespace(|| "float point parsing result"), || Ok(result)).unwrap();

    // num * 1 = input
    cs.enforce(
        || "float point result constraint",
        |_| exponent_result.lc(E::Fr::one()),
        |_| mantissa_result.lc(E::Fr::one()),
        |lc| lc + result_allocated.get_variable()
    );

    Ok(result_allocated)
}

#[test]
fn test_parsing() {
    use rand::{SeedableRng, Rng, XorShiftRng};
    use bellman::{ConstraintSystem};
    use pairing::bn256::{Bn256};
    use ::circuit::test::*;
    use super::boolean::{AllocatedBit, Boolean};

    let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let mut cs = TestConstraintSystem::<Bn256>::new();

    let bits: Vec<bool> = vec![true, false, false, false, false, true, true];

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

    let exp_result = parse_with_exponent_le(cs.namespace(|| "parse"), &circuit_bits, 5, 2, 10).unwrap();

    print!("{}\n", exp_result.get_value().unwrap());
}
