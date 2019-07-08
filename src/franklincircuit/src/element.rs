use crate::utils::pack_bits_to_element;
use bellman::{ConstraintSystem, SynthesisError};
use ff::Field;

use franklin_crypto::circuit::boolean::Boolean;

use franklin_crypto::circuit::num::AllocatedNum;

use franklin_crypto::jubjub::JubjubEngine;
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

      pub fn from_fe_padded<CS: ConstraintSystem<E>, F: FnOnce() -> Result<E::Fr, SynthesisError>>(
        mut cs: CS,
        field_element: F,
    ) -> Result<Self, SynthesisError> {
        let number =
            AllocatedNum::alloc(cs.namespace(|| "number from field element"), field_element)?;
        CircuitElement::from_number_padded(cs.namespace(|| "circuit_element"), number)
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

        let ce = CircuitElement {
            number: number,
            bits_le: bits,
            length: max_length,
        };
        ce.enforce_length(cs.namespace(|| "enforce_length"))?;

        Ok(ce)
    }

    pub fn from_number_padded<CS: ConstraintSystem<E>>(
        mut cs: CS,
        number: AllocatedNum<E>,
    ) -> Result<Self, SynthesisError> {
        let mut bits = number.into_bits_le(cs.namespace(|| "into_bits_le"))?;
        bits.resize(256, Boolean::constant(false));

        let ce = CircuitElement {
            number: number,
            bits_le: bits,
            length: 256,
        };

        Ok(ce)
    }

    pub fn enforce_length<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
    ) -> Result<(), SynthesisError> {
        let number_repacked =
            pack_bits_to_element(cs.namespace(|| "pack truncated bits"), &self.bits_le)?;
        cs.enforce(
            || format!("number can be represented in {} bits", self.length),
            |lc| lc + self.number.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + number_repacked.get_variable(),
        );
        Ok(())
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

    pub fn select_if_eq<CS: ConstraintSystem<E>>(
        mut cs: CS,
        a: &AllocatedNum<E>,
        b: &AllocatedNum<E>,
        x: &Self,
        y: &Self,
    ) -> Result<Self, SynthesisError> {
        assert_eq!(x.length, y.length);

        let selected_number = AllocatedNum::select_ifeq(
            cs.namespace(|| "select_ifeq"),
            &a,
            &b,
            &x.get_number(),
            &y.get_number(),
        )?;
        Ok(CircuitElement::from_number(
            cs.namespace(|| "chosen nonce"),
            selected_number,
            x.length,
        )?)
    }

    // doesn't enforce length by design, though applied to both strict values will give strict result
    pub fn conditionally_select<CS: ConstraintSystem<E>>(
        mut cs: CS,
        x: &Self,
        y: &Self,
        condition: &Boolean,
    ) -> Result<Self, SynthesisError> {
        assert_eq!(x.length, y.length);

        let selected_number = AllocatedNum::conditionally_select(
            cs.namespace(|| "conditionally_select"),
            &x.get_number(),
            &y.get_number(),
            &condition,
        )?;
        Ok(CircuitElement::from_number(
            cs.namespace(|| "chosen number as ce"),
            selected_number,
            x.length,
        )?)
    }

    // doesn't enforce length by design, though applied to both strict values will give strict result
    pub fn conditionally_select_with_number_strict<CS: ConstraintSystem<E>>(
        mut cs: CS,
        x: &AllocatedNum<E>,
        y: &Self,
        condition: &Boolean,
    ) -> Result<Self, SynthesisError> {
        let selected_number = AllocatedNum::conditionally_select(
            cs.namespace(|| "conditionally_select"),
            &x,
            &y.get_number(),
            &condition,
        )?;
        Ok(CircuitElement::from_number_strict(
            cs.namespace(|| "chosen number as ce"),
            selected_number,
            y.length,
        )?)
    }

    pub fn equals<CS: ConstraintSystem<E>>(
        mut cs: CS,
        x: &Self,
        y: &Self,
    ) -> Result<Boolean, SynthesisError> {
        let is_equal =
            AllocatedNum::equals(cs.namespace(|| "equals"), &x.get_number(), &y.get_number())?;
        Ok(Boolean::from(is_equal))
    }

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
    pub fn grab(&self) -> Result<E::Fr, SynthesisError> {
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
    pub fn conditionally_select<CS: ConstraintSystem<E>>(
        mut cs: CS,
        a: &Self,
        b: &Self,
        condition: &Boolean,
    ) -> Result<Self, SynthesisError> {
        let selected_x = CircuitElement::conditionally_select(
            cs.namespace(|| "conditionally_select_x"),
            &a.get_x(),
            &b.get_x(),
            &condition,
        )?;
        let selected_y = CircuitElement::conditionally_select(
            cs.namespace(|| "conditionally_select_y"),
            &a.get_y(),
            &b.get_y(),
            &condition,
        )?;
        Ok(CircuitPubkey {
            x: selected_x,
            y: selected_y,
        })
    }
    pub fn equals<CS: ConstraintSystem<E>>(
        mut cs: CS,
        a: &Self,
        b: &Self,
    ) -> Result<Boolean, SynthesisError> {
        let is_equal_x = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_equal_x"),
            &a.get_x().get_number(),
            &b.get_x().get_number(),
        )?);

        let is_equal_y = Boolean::from(AllocatedNum::equals(
            cs.namespace(|| "is_equal_y"),
            &a.get_x().get_number(),
            &b.get_x().get_number(),
        )?);
        Boolean::and(cs.namespace(|| "is_equal"), &is_equal_x, &is_equal_y)
    }
}
