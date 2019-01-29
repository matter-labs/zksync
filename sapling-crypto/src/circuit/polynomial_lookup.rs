use pairing::{Engine};
use bellman::{ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};
use super::*;

pub fn generate_powers<E: Engine, CS>(
    mut cs: CS,
    base: &num::AllocatedNum<E>, 
    max_power: usize
) -> Result<Vec<num::AllocatedNum<E>>, SynthesisError>
    where CS: ConstraintSystem<E>
    {
        let mut result = vec![];

        let mut power = num::AllocatedNum::alloc(
            cs.namespace(|| format!("0-th power")), 
            || {
                return Ok(E::Fr::one());
            }
        )?;

        result.push(power.clone());

        for i in 1..max_power {
            power = num::AllocatedNum::alloc(
                cs.namespace(|| format!("{}-th power", i)), 
                || {
                    let mut power = power.get_value().get()?.clone();
                    let value = base.get_value().get()?.clone();
                    power.mul_assign(&value);
                    
                    Ok(power)
                }
            )?;

            result.push(power.clone());
        }

    Ok(result)
}

pub fn do_the_lookup<E: Engine, CS>(
    mut cs: CS,
    coeffs: &[E::Fr], 
    bases: &[num::AllocatedNum<E>],
) -> Result<num::AllocatedNum<E>, SynthesisError>
    where CS: ConstraintSystem<E>
    {
        assert_eq!(coeffs.len(), bases.len());

        let mut num = num::Num::<E>::zero();
        for (c, b) in coeffs.iter().zip(bases.iter()) {
            num = num.add_number_with_coeff(b, c.clone());
        }

        let result = num::AllocatedNum::alloc(
            cs.namespace(|| "do the lookup"),
            || {
                Ok(*num.get_value().get()?)
            }
        )?;

        cs.enforce(
            || "enforce a lookup",
            |lc| lc + result.get_variable(),
            |lc| lc + CS::one(),
            |_| num.lc(E::Fr::one())
        );

        Ok(result)
    }