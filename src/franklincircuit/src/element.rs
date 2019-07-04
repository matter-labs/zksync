use crate::utils::{append_packed_public_key, pack_bits_to_element};
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField};
use franklin_crypto::circuit::baby_eddsa::EddsaSignature;
use franklin_crypto::circuit::boolean::{AllocatedBit, Boolean};
use franklin_crypto::circuit::ecc;
use franklin_crypto::circuit::float_point::parse_with_exponent_le;
use franklin_crypto::circuit::num::{AllocatedNum, Num};
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::circuit::polynomial_lookup::{do_the_lookup, generate_powers};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};
use franklinmodels::params as franklin_constants;

#[derive(Clone)]
pub struct CircuitElement<E: JubjubEngine> {
    number: AllocatedNum<E>,
    bits_le: Vec<Boolean>,
    length: usize,
}

impl<E: JubjubEngine> CircuitElement<E> {
    pub fn from_fe_strict<CS: ConstraintSystem<E>, F: FnOnce() -> Result<E::Fr, SynthesisError>>(
        mut cs: CS,
        field_element: F,
        max_length: usize,
    ) -> Result<Self, SynthesisError> {
        let number =
            AllocatedNum::alloc(cs.namespace(|| "number from field element"), field_element)?;
        CircuitElement::from_number_strict(cs.namespace(|| "circuit_element"), number, max_length)
    }
    pub fn from_number<CS: ConstraintSystem<E>>(
        mut cs: CS,
        number: AllocatedNum<E>,
        max_length: usize,
    ) -> Result<Self, SynthesisError> {
        let mut bits = number.into_bits_le(cs.namespace(|| "into_bits_le"))?;
        bits.truncate(max_length);
        Ok(CircuitElement {
            number: number,
            bits_le: bits,
            length: max_length,
        })
    }
    pub fn from_number_strict<CS: ConstraintSystem<E>>(
        mut cs: CS,
        number: AllocatedNum<E>,
        max_length: usize,
    ) -> Result<Self, SynthesisError> {
        let mut bits = number.into_bits_le(cs.namespace(|| "into_bits_le"))?;
        bits.truncate(max_length);
        let number_repacked = pack_bits_to_element(cs.namespace(|| "pack truncated bits"), &bits)?;
        cs.enforce(
            || format!("number can be represented in {} bits", max_length),
            |lc| lc + number.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + number_repacked.get_variable(),
        );

        Ok(CircuitElement {
            number: number,
            bits_le: bits,
            length: max_length,
        })
    }
    pub fn from_le_bits<CS: ConstraintSystem<E>>(
        mut cs: CS,
        bits: Vec<Boolean>,
        max_length: usize,
    ) -> Result<Self, SynthesisError> {
        let mut num_bits = bits.clone();
        num_bits.truncate(max_length);
        let number = pack_bits_to_element(cs.namespace(|| "pack_truncated_bits"), &bits)?;
        Ok(CircuitElement {
            number: number,
            bits_le: num_bits,
            length: max_length,
        })
    }

    // pub fn select_if_eq<CS: ConstraintSystem<E>>(
    //     mut cs: CS,
    //     a: AllocatedNum<E>,
    //     b: AllocatedNum<E>,
    //     x: &Self,
    //     y: &Self,
    // ) -> Result<Self, SynthesisError> {
    //     let mut num_bits = bits.clone();
    //     num_bits.truncate(max_length);
    //     let number = pack_bits_to_element(cs.namespace(|| "pack_truncated_bits"), &bits)?;
    //     Ok(CircuitElement {
    //         number: number,
    //         bits_le: num_bits,
    //         length: max_length,
    //     })
    // }

    // pub fn select_conditionally<CS: ConstraintSystem<E>>(
    //     mut cs: CS,
    //     condition: &Boolean,
    //     x: &Self,
    //     y: &Self,
    // ) -> Result<Self, SynthesisError> {
    //     let mut num_bits = bits.clone();
    //     num_bits.truncate(max_length);
    //     let number = pack_bits_to_element(cs.namespace(|| "pack_truncated_bits"), &bits)?;
    //     Ok(CircuitElement {
    //         number: number,
    //         bits_le: num_bits,
    //         length: max_length,
    //     })
    // }

    pub fn get_number(&self) -> AllocatedNum<E> {
        self.number.clone()
    }

    pub fn get_bits_le(&self) -> Vec<Boolean> {
        self.bits_le.clone()
    }

    pub fn get_bits_be(&self) -> Vec<Boolean> {
        let mut bits_be = self.bits_le.clone();
        bits_be.reverse();
        bits_be
    }
    pub fn grab(self) -> Result<E::Fr, SynthesisError> {
        match self.number.get_value() {
            Some(v) => Ok(v),
            None => Err(SynthesisError::AssignmentMissing),
        }
    }
}

#[derive(Clone)]
pub struct CircuitPubkey<E: JubjubEngine> {
    x: CircuitElement<E>,
    y: CircuitElement<E>,
}

impl<E: JubjubEngine> CircuitPubkey<E> {
    pub fn from_xy_fe<
        CS: ConstraintSystem<E>,
        Fx: FnOnce() -> Result<E::Fr, SynthesisError>,
        Fy: FnOnce() -> Result<E::Fr, SynthesisError>,
    >(
        mut cs: CS,
        x: Fx,
        y: Fy,
    ) -> Result<Self, SynthesisError> {
        let x_num = AllocatedNum::alloc(cs.namespace(|| "x_num"), x)?;
        let y_num = AllocatedNum::alloc(cs.namespace(|| "y_num"), y)?;
        let x_ce = CircuitElement::from_number(cs.namespace(|| "x"), x_num, 1)?;
        let y_ce = CircuitElement::from_number(
            cs.namespace(|| "y"),
            y_num,
            franklin_constants::FR_BIT_WIDTH - 1,
        )?;
        Ok(CircuitPubkey { x: x_ce, y: y_ce })
    }
    pub fn from_xy<CS: ConstraintSystem<E>>(
        mut cs: CS,
        x: AllocatedNum<E>,
        y: AllocatedNum<E>,
    ) -> Result<Self, SynthesisError> {
        let x_ce = CircuitElement::from_number(cs.namespace(|| "x"), x, 1)?;
        let y_ce = CircuitElement::from_number(
            cs.namespace(|| "y"),
            y,
            franklin_constants::FR_BIT_WIDTH - 1,
        )?;
        Ok(CircuitPubkey { x: x_ce, y: y_ce })
    }
    pub fn get_x(&self) -> CircuitElement<E> {
        self.x.clone()
    }
    pub fn get_y(&self) -> CircuitElement<E> {
        self.y.clone()
    }
    pub fn get_packed_key(&self) -> Vec<Boolean> {
        let mut result = vec![];
        result.extend(self.get_y().get_bits_le());
        result.extend(self.get_x().get_bits_le());
        result
    }
}
