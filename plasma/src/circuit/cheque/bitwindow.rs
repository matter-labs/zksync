use ff::{
    PrimeField,
    Field,
    BitIterator,
    PrimeFieldRepr
};

use bellman::{
    SynthesisError,
    ConstraintSystem,
    Circuit
};

use sapling_crypto::jubjub::{
    JubjubEngine,
    FixedGenerators,
    Unknown,
    edwards,
    JubjubParams
};

use super::Assignment;
use super::boolean;
use super::ecc;
use super::pedersen_hash;
use super::sha256;
use super::num;
use super::multipack;
use super::num::{AllocatedNum, Num};
use super::float_point::{parse_with_exponent_le, convert_to_float};
use super::baby_eddsa::EddsaSignature;

use sapling_crypto::eddsa::{
    Signature,
    PrivateKey,
    PublicKey
};

use crate::models::params as plasma_constants;
use super::super::leaf::{LeafWitness, LeafContent, make_leaf_content};
use crate::circuit::utils::{le_bit_vector_into_field_element, allocate_audit_path, append_packed_public_key};

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

/// This is an instance of the `Spend` circuit.
pub struct BitSet<'a, E: JubjubEngine> {
    pub params: &'a E::Params,

    pub action: (BitNumber<E>, BitWindowWitness<E>),

}

impl<'a, E: JubjubEngine> Circuit<E> for BitSet<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError>
    {
        let (bit_number, witness) = self.action;
        let current_bits_fe = AllocatedNum::alloc(
            cs.namespace(|| "allocate bits witness"),
            || {
                Ok(*witness.bits.get()?)
            }
        )?;

        let start = AllocatedNum::alloc(
            cs.namespace(|| "allocate window start witness"),
            || {
                Ok(*witness.start.get()?)
            }
        )?;

        let two_inversed = E::Fr::from_str("2").unwrap();

        let current_bits = current_bits_fe.into_bits_le(
            cs.namespace(|| "get current bits")
        )?;

        let bit_number = AllocatedNum::alloc(
            cs.namespace(|| "allocate bit number"),
            || {
                Ok(*bit_number.number.get()?)
            }
        )?;

        start.limit_number_of_bits(
            cs.namespace(|| "limit start as 2^32"),
            32
        )?;

        let distance = AllocatedNum::alloc(
            cs.namespace(|| "allocate bit number"),
            || {
                let mut num = bit_number.get_value().get()?.clone();
                let start = start.get_value().get()?.clone();
                num.sub_assign(&start);

                Ok(num)
            }
        )?;

        cs.enforce(
            || "enforce distance calculation",
            |lc| lc + distance.get_variable(),
            |lc| lc + CS::one(),
            |ls| ls + bit_number.get_variable() - start.get_variable()
        );

        // TODO: optimize to manual decomposition and limits to save on decompositions

        distance.limit_number_of_bits(
            cs.namespace(|| "limit distance as 2^32"),
            32
        )?;

        let distance_bits = distance.into_bits_le(cs.namespace(|| "decompose distance"))?;

        let capacity = E::Fr::CAPACITY;

        // there are three cases:
        // - distance is less than capacity, so the bits of interest is in the current bitset
        // - start + capacity < distance < start + 2 * capacity, make the bits of interest the highest bit,
        // so start = bit_number - capacity
        // - distance >= start + 2 * capacity, just zero the bitmask


        // calculate if distance < capacity



        Ok(())


    }
}

#[test]
fn test_redeem() {
    use ff::{Field, BitIterator};
    use pairing::bn256::*;
    use rand::{SeedableRng, Rng, XorShiftRng, Rand};
    use sapling_crypto::circuit::test::*;
    use sapling_crypto::alt_babyjubjub::{AltJubjubBn256, fs, edwards, PrimeOrder};
    use crate::models::circuit::{AccountTree, Account};
    // use super::super::account_tree::{AccountTree, Account};
    use crypto::sha2::Sha256;
    use crypto::digest::Digest;
    use crate::circuit::utils::{encode_fs_into_fr, be_bit_vector_into_bytes};
    use crate::primitives::GetBits;
    extern crate hex;

    let params = &AltJubjubBn256::new();
    let p_g = FixedGenerators::SpendingKeyGenerator;

    let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
}