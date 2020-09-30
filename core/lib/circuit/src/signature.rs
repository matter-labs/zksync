// External deps
use zksync_crypto::franklin_crypto::{
    bellman::{pairing::ff::PrimeField, ConstraintSystem, SynthesisError},
    circuit::{
        baby_eddsa::EddsaSignature,
        boolean::{le_bits_into_le_bytes, AllocatedBit, Boolean},
        ecc,
        expression::Expression,
        multipack, rescue,
    },
    jubjub::JubjubEngine,
    rescue::RescueEngine,
};
// Workspace deps
use zksync_crypto::params::{self as franklin_constants, FR_BIT_WIDTH, FR_BIT_WIDTH_PADDED};
// Local deps
use crate::{
    allocated_structures::*,
    element::{CircuitElement, CircuitPubkey},
    operation::SignatureData,
    utils::{multi_and, pack_bits_to_element, resize_grow_only, reverse_bytes},
};

/// Max len of message for signature, we use Pedersen hash to compress message to this len before signing.
const MAX_SIGN_MESSAGE_BIT_WIDTH: usize = 256;

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

pub struct AllocatedSignerPubkey<E: RescueEngine + JubjubEngine> {
    pub pubkey: CircuitPubkey<E>,
    pub point: ecc::EdwardsPoint<E>,
    pub is_correctly_unpacked: Boolean,
    pub r_y_bits: Vec<Boolean>,
    pub r_x_bit: Boolean,
}

pub fn unpack_point_if_possible<E: RescueEngine + JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    packed_key: &[Option<bool>],
    rescue_params: &<E as RescueEngine>::Params,
    jubjub_params: &<E as JubjubEngine>::Params,
) -> Result<AllocatedSignerPubkey<E>, SynthesisError> {
    assert_eq!(packed_key.len(), franklin_constants::FR_BIT_WIDTH_PADDED);
    let packed_key_bits_correct_order = reverse_bytes(packed_key);
    let r_x_bit =
        AllocatedBit::alloc(cs.namespace(|| "r_x_bit"), packed_key_bits_correct_order[0])?;

    let witness_length = packed_key.len();
    let start_of_y = witness_length - (E::Fr::NUM_BITS as usize);
    let r_y = CircuitElement::from_witness_be_bits(
        cs.namespace(|| "signature_r_y from bits"),
        &packed_key_bits_correct_order[start_of_y..],
    )?;

    let (r_recovered, is_r_correct) = ecc::EdwardsPoint::recover_from_y_unchecked(
        cs.namespace(|| "recover_from_y_unchecked"),
        &Boolean::from(r_x_bit.clone()),
        &r_y.get_number(),
        &jubjub_params,
    )?;
    log::debug!(
        "r_recovered.x={:?} \n r_recovered.y={:?}",
        r_recovered.get_x().get_value(),
        r_recovered.get_y().get_value()
    );

    let pubkey = CircuitPubkey::from_xy(
        cs.namespace(|| "pubkey from xy"),
        r_recovered.get_x().clone(),
        r_recovered.get_y().clone(),
        &rescue_params,
    )?;

    Ok(AllocatedSignerPubkey {
        pubkey,
        point: r_recovered,
        is_correctly_unpacked: is_r_correct,
        r_x_bit: Boolean::from(r_x_bit),
        r_y_bits: r_y.get_bits_be(),
    })
}

pub fn verify_circuit_signature<E: RescueEngine + JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    op_data: &AllocatedOperationData<E>,
    signer_key: &AllocatedSignerPubkey<E>,
    signature_data: SignatureData,
    rescue_params: &<E as RescueEngine>::Params,
    jubjub_params: &<E as JubjubEngine>::Params,
    generator: ecc::EdwardsPoint<E>,
) -> Result<AllocatedSignatureData<E>, SynthesisError> {
    let signature_data_r_packed = reverse_bytes(&signature_data.r_packed);
    let signature_data_s = reverse_bytes(&signature_data.s);
    assert_eq!(
        signature_data.r_packed.len(),
        franklin_constants::FR_BIT_WIDTH_PADDED
    );

    let witness_length = signature_data_r_packed.len();
    let r_x_bit = AllocatedBit::alloc(cs.namespace(|| "r_x_bit"), signature_data_r_packed[0])?;

    let start_of_y = witness_length - (E::Fr::NUM_BITS as usize);

    let r_y = CircuitElement::from_witness_be_bits(
        cs.namespace(|| "signature_r_y from bits"),
        &signature_data_r_packed[start_of_y..],
    )?;

    assert_eq!(
        signature_data.s.len(),
        franklin_constants::FR_BIT_WIDTH_PADDED
    );

    let witness_length = signature_data_s.len();
    let start_of_s = witness_length - (E::Fr::NUM_BITS as usize);

    let signature_s = CircuitElement::from_witness_be_bits(
        cs.namespace(|| "signature_s"),
        &signature_data_s[start_of_s..],
    )?;

    let (r_recovered, is_sig_r_correct) = ecc::EdwardsPoint::recover_from_y_unchecked(
        cs.namespace(|| "recover_from_y_unchecked"),
        &Boolean::from(r_x_bit.clone()),
        &r_y.get_number(),
        &jubjub_params,
    )?;

    let signature = EddsaSignature {
        r: r_recovered,
        s: signature_s.get_number(),
        pk: signer_key.point.clone(),
    };

    log::debug!(
        "signature_r_x={:?} \n signature_r_y={:?}",
        signature.r.get_x().get_value(),
        signature.r.get_y().get_value()
    );
    log::debug!("s={:?}", signature.s.get_value());

    let serialized_tx_bits = {
        let mut temp_bits = op_data.first_sig_msg.get_bits_le();
        temp_bits.extend(op_data.second_sig_msg.get_bits_le());
        temp_bits.extend(op_data.third_sig_msg.get_bits_le());
        temp_bits
    };

    assert_eq!(
        serialized_tx_bits.len(),
        franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS
    );

    let input = multipack::pack_into_witness(
        cs.namespace(|| "pack transaction bits into field elements for rescue"),
        &serialized_tx_bits,
    )?;

    // signature msg is the hash of serialized transaction
    let mut sponge_output = rescue::rescue_hash(cs.namespace(|| "sig_msg"), &input, rescue_params)?;

    assert_eq!(sponge_output.len(), 1);

    let sig_msg = sponge_output.pop().expect("must get an element");
    let mut sig_msg_bits = sig_msg.into_bits_le(cs.namespace(|| "sig_msg_bits"))?;
    resize_grow_only(&mut sig_msg_bits, 256, Boolean::constant(false));

    let is_sig_verified = is_rescue_signature_verified(
        cs.namespace(|| "musig sha256"),
        &sig_msg_bits,
        &signature,
        rescue_params,
        jubjub_params,
        generator,
    )?;

    log::debug!("is_sig_verified={:?}", is_sig_verified.get_value());
    log::debug!("is_sig_r_correct={:?}", is_sig_r_correct.get_value());
    log::debug!(
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
        sig_r_y_bits: r_y.into_padded_be_bits(franklin_constants::FR_BIT_WIDTH_PADDED - 1),
        sig_s_bits: signature_s.into_padded_be_bits(franklin_constants::FR_BIT_WIDTH_PADDED),
    })
}
pub fn verify_signature_message_construction<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    mut serialized_tx_bits: Vec<Boolean>,
    op_data: &AllocatedOperationData<E>,
) -> Result<Boolean, SynthesisError> {
    assert!(serialized_tx_bits.len() < franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS);

    resize_grow_only(
        &mut serialized_tx_bits,
        franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS,
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

pub fn is_rescue_signature_verified<E: RescueEngine + JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    sig_data_bits: &[Boolean],
    signature: &EddsaSignature<E>,
    rescue_params: &<E as RescueEngine>::Params,
    jubjub_params: &<E as JubjubEngine>::Params,
    generator: ecc::EdwardsPoint<E>,
) -> Result<Boolean, SynthesisError> {
    // This constant is also used inside `franklin_crypto` verify rescue(enforce version of this check)
    const INPUT_PAD_LEN_FOR_RESCUE: usize = 768;
    let mut sig_data_bits = sig_data_bits.to_vec();
    assert!(
        sig_data_bits.len() <= MAX_SIGN_MESSAGE_BIT_WIDTH,
        "Signature message len is too big {}/{}",
        sig_data_bits.len(),
        MAX_SIGN_MESSAGE_BIT_WIDTH
    );
    resize_grow_only(
        &mut sig_data_bits,
        MAX_SIGN_MESSAGE_BIT_WIDTH,
        Boolean::constant(false),
    );
    sig_data_bits = le_bits_into_le_bytes(sig_data_bits);

    let mut hash_input: Vec<Boolean> = vec![];
    {
        let mut pk_x_serialized = signature
            .pk
            .get_x()
            .into_bits_le_strict(cs.namespace(|| "pk_x_bits into bits strict"))?;

        assert_eq!(pk_x_serialized.len(), FR_BIT_WIDTH);

        resize_grow_only(
            &mut pk_x_serialized,
            FR_BIT_WIDTH_PADDED,
            Boolean::constant(false),
        );
        hash_input.extend(le_bits_into_le_bytes(pk_x_serialized));
    }
    {
        let mut r_x_serialized = signature
            .r
            .get_x()
            .into_bits_le_strict(cs.namespace(|| "r_x_bits into bits strict"))?;

        assert_eq!(r_x_serialized.len(), FR_BIT_WIDTH);

        resize_grow_only(
            &mut r_x_serialized,
            FR_BIT_WIDTH_PADDED,
            Boolean::constant(false),
        );
        hash_input.extend(le_bits_into_le_bytes(r_x_serialized));
    }
    hash_input.extend(sig_data_bits);
    resize_grow_only(
        &mut hash_input,
        INPUT_PAD_LEN_FOR_RESCUE,
        Boolean::constant(false),
    );

    let hash_input = multipack::pack_into_witness(
        cs.namespace(|| "pack FS parameter bits into fiedl elements"),
        &hash_input,
    )?;

    assert_eq!(
        hash_input.len(),
        4,
        "multipacking of FS hash is expected to have length 4"
    );

    let mut sponge = rescue::StatefulRescueGadget::new(rescue_params);
    sponge.specialize(
        cs.namespace(|| "specialize rescue on input length"),
        hash_input.len() as u8,
    );

    sponge.absorb(
        cs.namespace(|| "apply rescue hash on FS parameters"),
        &hash_input,
        &rescue_params,
    )?;

    let s0 = sponge.squeeze_out_single(
        cs.namespace(|| "squeeze first word form sponge"),
        &rescue_params,
    )?;

    let s1 = sponge.squeeze_out_single(
        cs.namespace(|| "squeeze second word form sponge"),
        &rescue_params,
    )?;

    let s0_bits =
        s0.into_bits_le_strict(cs.namespace(|| "make bits of first word for FS challenge"))?;
    let s1_bits =
        s1.into_bits_le_strict(cs.namespace(|| "make bits of second word for FS challenge"))?;

    let take_bits = (<E as JubjubEngine>::Fs::CAPACITY / 2) as usize;

    let mut bits = Vec::with_capacity(<E as JubjubEngine>::Fs::CAPACITY as usize);
    bits.extend_from_slice(&s0_bits[0..take_bits]);
    bits.extend_from_slice(&s1_bits[0..take_bits]);
    assert!(bits.len() == E::Fs::CAPACITY as usize);

    let max_message_len = 32 as usize; //since it is the result of sha256 hash

    // we can use lowest bits of the challenge
    let is_sig_verified = verify_schnorr_relationship(
        signature,
        cs.namespace(|| "verify transaction signature"),
        jubjub_params,
        &bits,
        generator,
        max_message_len,
    )?;
    Ok(is_sig_verified)
}

pub fn verify_schnorr_relationship<CS, E>(
    signature: &EddsaSignature<E>,
    mut cs: CS,
    params: &E::Params,
    fs_challenge: &[Boolean],
    generator: ecc::EdwardsPoint<E>,
    max_message_len: usize,
) -> Result<Boolean, SynthesisError>
where
    CS: ConstraintSystem<E>,
    E: JubjubEngine,
{
    // message is always padded to 256 bits in this gadget, but still checked on synthesis
    assert!(fs_challenge.len() <= max_message_len * 8);

    let scalar_bits = signature
        .s
        .into_bits_le_fixed(cs.namespace(|| "Get S bits"), E::Fs::NUM_BITS as usize)?;

    let sb = generator.mul(cs.namespace(|| "S*B computation"), &scalar_bits, params)?;

    // only order of R is checked. Public key and generator can be guaranteed to be in proper group!
    // by some other means for out particular case
    let r_is_not_small_order = is_not_small_order(
        &signature.r,
        cs.namespace(|| "R is in right order"),
        &params,
    )?;

    let challenge = fs_challenge;

    let pk_mul_hash = signature
        .pk
        .mul(cs.namespace(|| "Calculate h*PK"), &challenge, params)?;

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
