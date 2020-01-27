use crate::allocated_structures::*;
use crate::element::{CircuitElement, CircuitPubkey};
use crate::operation::SignatureData;
use crate::utils::{multi_and, pack_bits_to_element, reverse_bytes};
use franklin_crypto::bellman::{ConstraintSystem, SynthesisError};
use franklin_crypto::bellman::pairing::ff::PrimeField;
use franklin_crypto::circuit::baby_eddsa::EddsaSignature;
use franklin_crypto::circuit::boolean::{AllocatedBit, Boolean};
use franklin_crypto::circuit::ecc;

use franklin_crypto::circuit::expression::Expression;
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::jubjub::JubjubEngine;
use models::params as franklin_constants;

pub struct AllocatedSignatureData<E: JubjubEngine> {
    pub eddsa: EddsaSignature<E>,
    pub is_verified: Boolean,
    pub sig_r_y_bits: Vec<Boolean>,
    pub sig_r_x_bit: Boolean,
    pub sig_s_bits: Vec<Boolean>,
}

impl<E: JubjubEngine> AllocatedSignatureData<E> {
    pub fn get_packed_r(&self) -> Vec<Boolean> {
        let mut r_packed_bits = vec![];
        r_packed_bits.push(self.sig_r_x_bit.clone());
        r_packed_bits.extend(self.sig_r_y_bits.clone());
        reverse_bytes(&r_packed_bits)
    }
}

pub struct AllocatedSignerPubkey<E: JubjubEngine> {
    pub pubkey: CircuitPubkey<E>,
    pub point: ecc::EdwardsPoint<E>,
    pub is_correctly_unpacked: Boolean,
    pub r_y_bits: Vec<Boolean>,
    pub r_x_bit: Boolean,
}
pub fn unpack_point_if_possible<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    packed_key: &[Option<bool>],
    params: &E::Params,
) -> Result<AllocatedSignerPubkey<E>, SynthesisError> {
    assert_eq!(packed_key.len(), franklin_constants::FR_BIT_WIDTH_PADDED);
    let packed_key_bits_correct_order = reverse_bytes(packed_key);
    let r_x_bit =
        AllocatedBit::alloc(cs.namespace(|| "r_x_bit"), packed_key_bits_correct_order[0])?;

    let r_y = CircuitElement::from_witness_be_bits(
        cs.namespace(|| "signature_r_y from bits"),
        &packed_key_bits_correct_order[1..],
    )?;

    let (r_recovered, is_r_correct) = ecc::EdwardsPoint::recover_from_y_unchecked(
        cs.namespace(|| "recover_from_y_unchecked"),
        &Boolean::from(r_x_bit.clone()),
        &r_y.get_number(),
        &params,
    )?;
    debug!(
        "r_recovered.x={:?} \n r_recovered.y={:?}",
        r_recovered.get_x().get_value(),
        r_recovered.get_y().get_value()
    );

    let pubkey = CircuitPubkey::from_xy(
        cs.namespace(|| "pubkey from xy"),
        r_recovered.get_x().clone(),
        r_recovered.get_y().clone(),
        &params,
    )?;
    Ok(AllocatedSignerPubkey {
        pubkey,
        point: r_recovered,
        is_correctly_unpacked: is_r_correct,
        r_x_bit: Boolean::from(r_x_bit),
        r_y_bits: r_y.get_bits_be(),
    })
}
pub fn verify_circuit_signature<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    op_data: &AllocatedOperationData<E>,
    signer_key: &AllocatedSignerPubkey<E>,
    signature_data: SignatureData,
    params: &E::Params,
    generator: ecc::EdwardsPoint<E>,
) -> Result<AllocatedSignatureData<E>, SynthesisError> {
    let signature_data_r_packed = reverse_bytes(&signature_data.r_packed);
    let signature_data_s = reverse_bytes(&signature_data.s);
    assert_eq!(
        signature_data.r_packed.len(),
        franklin_constants::FR_BIT_WIDTH_PADDED
    );
    let r_x_bit = AllocatedBit::alloc(cs.namespace(|| "r_x_bit"), signature_data_r_packed[0])?;

    let r_y = CircuitElement::from_witness_be_bits(
        cs.namespace(|| "signature_r_y from bits"),
        &signature_data_r_packed[1..],
    )?;

    assert_eq!(
        signature_data.s.len(),
        franklin_constants::FR_BIT_WIDTH_PADDED
    );
    let signature_s =
        CircuitElement::from_witness_be_bits(cs.namespace(|| "signature_s"), &signature_data_s)?;
    let (r_recovered, is_sig_r_correct) = ecc::EdwardsPoint::recover_from_y_unchecked(
        cs.namespace(|| "recover_from_y_unchecked"),
        &Boolean::from(r_x_bit.clone()),
        &r_y.get_number(),
        &params,
    )?;

    let signature = EddsaSignature {
        r: r_recovered,
        s: signature_s.get_number(),
        pk: signer_key.point.clone(),
    };

    debug!(
        "signature_r_x={:?} \n signature_r_y={:?}",
        signature.r.get_x().get_value(),
        signature.r.get_y().get_value()
    );
    debug!("s={:?}", signature.s.get_value());

    let serialized_tx_bits = {
        let mut temp_bits = op_data.first_sig_msg.get_bits_le();
        temp_bits.extend(op_data.second_sig_msg.get_bits_le());
        temp_bits.extend(op_data.third_sig_msg.get_bits_le());
        temp_bits
    };

    // signature msg is the hash of serialized transaction
    let sig_msg = pedersen_hash::pedersen_hash(
        cs.namespace(|| "sig_msg"),
        pedersen_hash::Personalization::NoteCommitment,
        &serialized_tx_bits,
        params,
    )?
    .get_x()
    .clone();
    let mut sig_msg_bits = sig_msg.into_bits_le(cs.namespace(|| "sig_msg_bits"))?;
    sig_msg_bits.resize(256, Boolean::constant(false));

    // signature.verify_sha256_musig(
    //     cs.namespace(|| "verify_sha"),
    //     self.params,
    //     &sig_msg_bits,
    //     generator,
    // )?;

    //TODO: put bits here

    let is_sig_verified = verify_pedersen(
        cs.namespace(|| "musig pedersen"),
        &sig_msg_bits,
        &signature,
        params,
        generator,
    )?;
    debug!("is_sig_verified={:?}", is_sig_verified.get_value());
    debug!("is_sig_r_correct={:?}", is_sig_r_correct.get_value());
    debug!(
        "signer_key.is_correctly_unpacked={:?}",
        signer_key.is_correctly_unpacked.get_value()
    );
    let is_signature_correctly_verified = multi_and(
        cs.namespace(|| "is_signature_correctly_verified"),
        &[
            is_sig_verified,
            is_sig_r_correct,
            signer_key.is_correctly_unpacked.clone(),
        ],
    )?;

    Ok(AllocatedSignatureData {
        eddsa: signature,
        is_verified: is_signature_correctly_verified,
        sig_r_x_bit: Boolean::from(r_x_bit),
        sig_r_y_bits: r_y.get_bits_be(),
        sig_s_bits: signature_s.get_bits_be(),
    })
}
pub fn verify_signature_message_construction<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    mut serialized_tx_bits: Vec<Boolean>,
    op_data: &AllocatedOperationData<E>,
) -> Result<Boolean, SynthesisError> {
    assert!(serialized_tx_bits.len() < franklin_constants::MAX_CIRCUIT_PEDERSEN_HASH_BITS);

    serialized_tx_bits.resize(
        franklin_constants::MAX_CIRCUIT_PEDERSEN_HASH_BITS,
        Boolean::constant(false),
    );
    let (first_sig_part_bits, remaining) = serialized_tx_bits.split_at(E::Fr::CAPACITY as usize);
    let remaining = remaining.to_vec();
    let (second_sig_part_bits, third_sig_part_bits) = remaining.split_at(E::Fr::CAPACITY as usize);
    let first_sig_part =
        pack_bits_to_element(cs.namespace(|| "first_sig_part"), &first_sig_part_bits)?;

    let second_sig_part =
        pack_bits_to_element(cs.namespace(|| "second_sig_part"), &second_sig_part_bits)?;
    let third_sig_part =
        pack_bits_to_element(cs.namespace(|| "third_sig_part"), &third_sig_part_bits)?;

    let is_first_sig_part_correct = Boolean::from(Expression::equals(
        cs.namespace(|| "is_first_sig_part_correct"),
        Expression::from(&first_sig_part),
        Expression::from(&op_data.first_sig_msg.get_number()),
    )?);

    let is_second_sig_part_correct = Boolean::from(Expression::equals(
        cs.namespace(|| "is_second_sig_part_correct"),
        Expression::from(&second_sig_part),
        Expression::from(&op_data.second_sig_msg.get_number()),
    )?);

    let is_third_sig_part_correct = Boolean::from(Expression::equals(
        cs.namespace(|| "is_third_sig_part_correct"),
        Expression::from(&third_sig_part),
        Expression::from(&op_data.third_sig_msg.get_number()),
    )?);
    let is_serialized_transaction_correct = multi_and(
        cs.namespace(|| "first part and second part"),
        &[
            is_first_sig_part_correct,
            is_second_sig_part_correct,
            is_third_sig_part_correct,
        ],
    )?;
    Ok(is_serialized_transaction_correct)
}

pub fn verify_pedersen<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    sig_data_bits: &[Boolean],
    signature: &EddsaSignature<E>,
    params: &E::Params,
    generator: ecc::EdwardsPoint<E>,
) -> Result<Boolean, SynthesisError> {
    let mut sig_data_bits = sig_data_bits.to_vec();
    sig_data_bits.resize(256, Boolean::constant(false));

    let mut first_round_bits: Vec<Boolean> = vec![];

    let mut pk_x_serialized = signature
        .pk
        .get_x()
        .clone()
        .into_bits_le(cs.namespace(|| "pk_x_bits"))?;
    pk_x_serialized.resize(256, Boolean::constant(false));

    let mut r_x_serialized = signature
        .r
        .get_x()
        .clone()
        .into_bits_le(cs.namespace(|| "r_x_bits"))?;
    r_x_serialized.resize(256, Boolean::constant(false));

    first_round_bits.extend(pk_x_serialized);
    first_round_bits.extend(r_x_serialized);

    let first_round_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "first_round_hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &first_round_bits,
        params,
    )?;
    let mut first_round_hash_bits = first_round_hash
        .get_x()
        .into_bits_le(cs.namespace(|| "first_round_hash_bits"))?;
    first_round_hash_bits.resize(256, Boolean::constant(false));

    let mut second_round_bits = vec![];
    second_round_bits.extend(first_round_hash_bits);
    second_round_bits.extend(sig_data_bits);
    let second_round_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "second_hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &second_round_bits,
        params,
    )?
    .get_x()
    .clone();

    let h_bits = second_round_hash.into_bits_le(cs.namespace(|| "h_bits"))?;

    let max_message_len = 32 as usize; //since it is the result of pedersen hash

    let is_sig_verified = is_verified_raw_message_signature(
        signature,
        cs.namespace(|| "verify transaction signature"),
        params,
        &h_bits,
        generator,
        max_message_len,
    )?;
    Ok(is_sig_verified)
}

pub fn is_verified_raw_message_signature<CS, E>(
    signature: &EddsaSignature<E>,
    mut cs: CS,
    params: &E::Params,
    message: &[Boolean],
    generator: ecc::EdwardsPoint<E>,
    max_message_len: usize,
) -> Result<Boolean, SynthesisError>
where
    CS: ConstraintSystem<E>,
    E: JubjubEngine,
{
    // TODO check that s < Fs::Char

    // message is always padded to 256 bits in this gadget, but still checked on synthesis
    assert!(message.len() <= max_message_len * 8);

    let scalar_bits = signature.s.into_bits_le(cs.namespace(|| "Get S bits"))?;

    let sb = generator.mul(cs.namespace(|| "S*B computation"), &scalar_bits, params)?;

    // only order of R is checked. Public key and generator can be guaranteed to be in proper group!
    // by some other means for out particular case
    let r_is_not_small_order = is_not_small_order(
        &signature.r,
        cs.namespace(|| "R is in right order"),
        &params,
    )?;

    let mut h: Vec<Boolean> = vec![];
    h.extend(message.iter().cloned());
    h.resize(256, Boolean::Constant(false));

    assert_eq!(h.len(), 256);

    let pk_mul_hash = signature
        .pk
        .mul(cs.namespace(|| "Calculate h*PK"), &h, params)?;

    let rhs = pk_mul_hash.add(cs.namespace(|| "Make signature RHS"), &signature.r, params)?;

    let rhs_x = rhs.get_x();
    let rhs_y = rhs.get_y();

    let sb_x = sb.get_x();
    let sb_y = sb.get_y();

    let is_x_correct = Boolean::from(Expression::equals(
        cs.namespace(|| "is x coordinate correct"),
        Expression::from(rhs_x),
        Expression::from(sb_x),
    )?);
    let is_y_correct = Boolean::from(Expression::equals(
        cs.namespace(|| "is y coordinate correct"),
        Expression::from(rhs_y),
        Expression::from(sb_y),
    )?);
    Ok(multi_and(
        cs.namespace(|| "is signature correct"),
        &[r_is_not_small_order, is_x_correct, is_y_correct],
    )?)
}

pub fn is_not_small_order<CS, E>(
    point: &ecc::EdwardsPoint<E>,
    mut cs: CS,
    params: &E::Params,
) -> Result<Boolean, SynthesisError>
where
    CS: ConstraintSystem<E>,
    E: JubjubEngine,
{
    let tmp = point.double(cs.namespace(|| "first doubling"), params)?;
    let tmp = tmp.double(cs.namespace(|| "second doubling"), params)?;
    let tmp = tmp.double(cs.namespace(|| "third doubling"), params)?;

    // (0, -1) is a small order point, but won't ever appear here
    // because cofactor is 2^3, and we performed three doublings.
    // (0, 1) is the neutral element, so checking if x is nonzero
    // is sufficient to prevent small order points here.
    let is_zero = Expression::equals(
        cs.namespace(|| "x==0"),
        Expression::from(tmp.get_x()),
        Expression::u64::<CS>(0),
    )?;

    Ok(Boolean::from(is_zero).not())
}
