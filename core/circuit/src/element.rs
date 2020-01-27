use crate::utils::{allocate_bits_vector, pack_bits_to_element, reverse_bytes};
use franklin_crypto::bellman::{ConstraintSystem, SynthesisError};
use franklin_crypto::circuit::boolean::Boolean;

use franklin_crypto::circuit::expression::Expression;
use franklin_crypto::circuit::num::AllocatedNum;
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::jubjub::JubjubEngine;
use models::params as franklin_constants;

#[derive(Clone)]
pub struct CircuitElement<E: JubjubEngine> {
    number: AllocatedNum<E>,
    bits_le: Vec<Boolean>,
    length: usize,
}
impl<E: JubjubEngine> CircuitElement<E> {
    pub fn pad(self, n: usize) -> Self {
        assert!(self.length <= n);
        let mut padded_bits = self.get_bits_le();
        padded_bits.resize(n, Boolean::constant(false));
        CircuitElement {
            number: self.number,
            bits_le: padded_bits,
            length: n,
        }
    }
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

    pub fn from_witness_be_bits<CS: ConstraintSystem<E>>(
        mut cs: CS,
        witness_bits: &[Option<bool>],
    ) -> Result<Self, SynthesisError> {
        let mut allocated_bits =
            allocate_bits_vector(cs.namespace(|| "allocate bits"), witness_bits)?;
        allocated_bits.reverse();
        let length = allocated_bits.len();
        let number = pack_bits_to_element(cs.namespace(|| "ce from bits"), &allocated_bits)?;
        Ok(Self {
            number,
            bits_le: allocated_bits,
            length,
        })
    }

    pub fn from_number<CS: ConstraintSystem<E>>(
        mut cs: CS,
        number: AllocatedNum<E>,
        max_length: usize,
    ) -> Result<Self, SynthesisError> {
        // let mut bits = number.into_bits_le(cs.namespace(|| "into_bits_le"))?;
        let mut bits = number.into_bits_le(cs.namespace(|| "into_bits_le_fixed"))?;
        bits.truncate(max_length);
        Ok(CircuitElement {
            number,
            bits_le: bits,
            length: max_length,
        })
    }
    pub fn from_number_strict<CS: ConstraintSystem<E>>(
        mut cs: CS,
        number: AllocatedNum<E>,
        max_length: usize,
    ) -> Result<Self, SynthesisError> {
        // let mut bits = number.into_bits_le(cs.namespace(|| "into_bits_le"))?;
        let bits = number.into_bits_le_fixed(cs.namespace(|| "into_bits_le_fixed"), max_length)?;

        let ce = CircuitElement {
            number,
            bits_le: bits,
            length: max_length,
        };
        // ce.enforce_length(cs.namespace(|| "enforce_length"))?;

        Ok(ce)
    }
    pub fn from_expression_padded<CS: ConstraintSystem<E>>(
        mut cs: CS,
        expr: Expression<E>,
    ) -> Result<Self, SynthesisError> {
        let mut bits = expr.into_bits_le(cs.namespace(|| "into_bits_le"))?;
        bits.resize(256, Boolean::constant(false));
        let number = pack_bits_to_element(cs.namespace(|| "pack back"), &bits)?;
        let ce = CircuitElement {
            number,
            bits_le: bits,
            length: 256,
        };

        Ok(ce)
    }

    pub fn from_number_padded<CS: ConstraintSystem<E>>(
        mut cs: CS,
        number: AllocatedNum<E>,
    ) -> Result<Self, SynthesisError> {
        let mut bits = number.into_bits_le(cs.namespace(|| "into_bits_le"))?;
        bits.resize(256, Boolean::constant(false));

        let ce = CircuitElement {
            number,
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
            number,
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
    pub fn conditionally_select_with_number_strict<
        CS: ConstraintSystem<E>,
        EX: Into<Expression<E>>,
    >(
        mut cs: CS,
        x: EX,
        y: &Self,
        condition: &Boolean,
    ) -> Result<Self, SynthesisError> {
        let selected_number = Expression::conditionally_select(
            cs.namespace(|| "conditionally_select"),
            x,
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
    hash: CircuitElement<E>,
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
        params: &E::Params,
    ) -> Result<Self, SynthesisError> {
        let x_num = AllocatedNum::alloc(cs.namespace(|| "x_num"), x)?;
        let y_num = AllocatedNum::alloc(cs.namespace(|| "y_num"), y)?;
        let x_ce = CircuitElement::from_number_padded(cs.namespace(|| "x"), x_num)?;
        let y_ce = CircuitElement::from_number_padded(cs.namespace(|| "y"), y_num)?;
        let mut to_hash = vec![];
        to_hash.extend(x_ce.get_bits_le());
        to_hash.extend(y_ce.get_bits_le());
        let hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &to_hash,
            params,
        )?;
        let mut hash_bits = hash
            .get_x()
            .into_bits_le(cs.namespace(|| "hash into_bits"))?;
        hash_bits.truncate(franklin_constants::NEW_PUBKEY_HASH_WIDTH);
        let hash_repacked = pack_bits_to_element(cs.namespace(|| "repack_hash"), &hash_bits)?;
        let hash_repacked_ce = CircuitElement::from_number(
            cs.namespace(|| "hash_repacked_ce"),
            hash_repacked,
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        )?;

        Ok(CircuitPubkey {
            x: x_ce,
            y: y_ce,
            hash: hash_repacked_ce,
        })
    }
    pub fn from_xy<CS: ConstraintSystem<E>>(
        mut cs: CS,
        x: AllocatedNum<E>,
        y: AllocatedNum<E>,
        params: &E::Params,
    ) -> Result<Self, SynthesisError> {
        let x_ce = CircuitElement::from_number_padded(cs.namespace(|| "x"), x)?;
        let y_ce = CircuitElement::from_number_padded(cs.namespace(|| "y"), y)?;
        let mut to_hash = vec![];
        to_hash.extend(x_ce.get_bits_le());
        to_hash.extend(y_ce.get_bits_le());
        let hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &to_hash,
            params,
        )?;
        debug!("hash when fromxy: {:?}", hash.get_x().get_value());
        let mut hash_bits = hash
            .get_x()
            .into_bits_le(cs.namespace(|| "hash into_bits"))?;
        hash_bits.truncate(franklin_constants::NEW_PUBKEY_HASH_WIDTH);
        let hash_repacked = pack_bits_to_element(cs.namespace(|| "repack_hash"), &hash_bits)?;
        let hash_repacked_ce = CircuitElement::from_number(
            cs.namespace(|| "hash_repacked_ce"),
            hash_repacked,
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        )?;

        Ok(CircuitPubkey {
            x: x_ce,
            y: y_ce,
            hash: hash_repacked_ce,
        })
    }
    pub fn get_x(&self) -> CircuitElement<E> {
        self.x.clone()
    }
    pub fn get_y(&self) -> CircuitElement<E> {
        self.y.clone()
    }
    pub fn get_hash(&self) -> CircuitElement<E> {
        self.hash.clone()
    }
    pub fn get_external_packing(&self) -> Vec<Boolean> {
        let mut ext_bits = vec![];
        ext_bits.push(self.get_x().get_bits_le()[0].clone());
        ext_bits.extend(self.get_y().get_bits_be()[1..].to_vec());
        reverse_bytes(&ext_bits)
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
        let selected_hash = CircuitElement::conditionally_select(
            cs.namespace(|| "conditionally_select_hash"),
            &a.get_hash(),
            &b.get_hash(),
            &condition,
        )?;
        Ok(CircuitPubkey {
            x: selected_x,
            y: selected_y,
            hash: selected_hash,
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
