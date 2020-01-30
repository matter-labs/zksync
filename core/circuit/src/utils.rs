use bellman::{ConstraintSystem, SynthesisError};
use ff::{BitIterator, Field, PrimeField};

use franklin_crypto::circuit::boolean::{AllocatedBit, Boolean};
use franklin_crypto::circuit::num::{AllocatedNum, Num};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::eddsa::Signature;
use franklin_crypto::eddsa::{PrivateKey, PublicKey};
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine};

use crate::operation::TransactionSignature;
use crate::operation::{ETHSignatureData, SignatureData};
use models::circuit::utils::le_bit_vector_into_field_element;
use models::node::tx::PackedEthSignature;
use models::params as franklin_constants;
use models::primitives::*;

pub fn reverse_bytes<T: Clone>(bits: &[T]) -> Vec<T> {
    bits.chunks(8)
        .rev()
        .map(|x| x.to_vec())
        .fold(Vec::new(), |mut acc, mut byte| {
            acc.append(&mut byte);
            acc
        })
}
pub fn sign_pedersen<R, E>(
    msg_data: &[bool],
    private_key: &PrivateKey<E>,
    p_g: FixedGenerators,
    params: &E::Params,
    rng: &mut R,
) -> SignatureData
where
    R: rand::Rng,
    E: JubjubEngine,
{
    let message_bytes = pack_bits_into_bytes(msg_data.to_vec());

    let signature = private_key.musig_pedersen_sign(&message_bytes, rng, p_g, params);

    let pk = PublicKey::from_private(&private_key, p_g, params);
    let _is_valid_signature =
        pk.verify_musig_pedersen(&message_bytes, &signature.clone(), p_g, params);

    // TODO: handle the case where it is not valid
    // if !is_valid_signature {
    //     return None;
    // }
    let (sig_r_x, sig_r_y) = signature.r.into_xy();
    debug!("signature.s: {}", signature.s);
    debug!("signature.r.x: {}", sig_r_x);
    debug!("signature.r.y: {}", sig_r_y);

    convert_signature_to_representation(signature)
}

pub fn convert_signature_to_representation<E>(signature: Signature<E>) -> SignatureData
where
    E: JubjubEngine,
{
    let (sig_x, sig_y) = signature.clone().r.into_xy();
    let mut signature_s_be_bits: Vec<bool> = BitIterator::new(signature.s.into_repr()).collect();
    signature_s_be_bits.reverse();
    signature_s_be_bits.resize(franklin_constants::FR_BIT_WIDTH_PADDED, false);
    signature_s_be_bits.reverse();
    let mut signature_r_x_be_bits: Vec<bool> = BitIterator::new(sig_x.into_repr()).collect();
    signature_r_x_be_bits.reverse();
    signature_r_x_be_bits.resize(franklin_constants::FR_BIT_WIDTH_PADDED, false);
    signature_r_x_be_bits.reverse();
    let mut signature_r_y_be_bits: Vec<bool> = BitIterator::new(sig_y.into_repr()).collect();
    signature_r_y_be_bits.reverse();
    signature_r_y_be_bits.resize(franklin_constants::FR_BIT_WIDTH_PADDED, false);
    signature_r_y_be_bits.reverse();
    let mut sig_r_packed_bits = vec![];
    sig_r_packed_bits.push(signature_r_x_be_bits[franklin_constants::FR_BIT_WIDTH_PADDED - 1]);
    sig_r_packed_bits.extend(signature_r_y_be_bits[1..].iter());
    let sig_r_packed_bits = reverse_bytes(&sig_r_packed_bits);

    assert_eq!(
        sig_r_packed_bits.len(),
        franklin_constants::FR_BIT_WIDTH_PADDED
    );

    let sig_s_bits = signature_s_be_bits.clone();
    let sig_s_bits = reverse_bytes(&sig_s_bits);

    SignatureData {
        r_packed: sig_r_packed_bits.iter().map(|x| Some(*x)).collect(),
        s: sig_s_bits.iter().map(|x| Some(*x)).collect(),
    }
}

pub fn convert_eth_signature_to_representation<E>(
    signature: &PackedEthSignature,
) -> ETHSignatureData<E>
where
    E: JubjubEngine,
{
    let mut signature_r_be_bits: Vec<bool> = bytes_into_be_bits(&signature.0.r);
    signature_r_be_bits.reverse();
    signature_r_be_bits.resize(franklin_constants::FR_BIT_WIDTH_PADDED, false);
    signature_r_be_bits.reverse();
    let mut signature_s_be_bits: Vec<bool> = bytes_into_be_bits(&signature.0.s);
    signature_s_be_bits.reverse();
    signature_s_be_bits.resize(franklin_constants::FR_BIT_WIDTH_PADDED, false);
    signature_s_be_bits.reverse();

    ETHSignatureData {
        r: signature_r_be_bits.iter().map(|x| Some(*x)).collect(),
        s: signature_s_be_bits.iter().map(|x| Some(*x)).collect(),
        v: Some(E::Fr::from_str(&signature.0.v.to_string()).unwrap()),
    }
}

pub fn sign_sha<R, E>(
    msg_data: &[bool],
    private_key: &PrivateKey<E>,
    p_g: FixedGenerators,
    params: &E::Params,
    rng: &mut R,
) -> Option<TransactionSignature<E>>
where
    R: rand::Rng,
    E: JubjubEngine,
{
    let raw_data: Vec<bool> = msg_data.to_vec();

    let mut message_bytes: Vec<u8> = vec![];

    let byte_chunks = raw_data.chunks(8);
    for byte_chunk in byte_chunks {
        let mut byte = 0u8;
        for (i, bit) in byte_chunk.iter().enumerate() {
            if *bit {
                byte |= 1 << (7 - i); //TODO: ask shamatar why do we need rev here, but not in pedersen
            }
        }
        message_bytes.push(byte);
    }

    let signature = private_key.musig_sha256_sign(&message_bytes, rng, p_g, params);

    let pk = PublicKey::from_private(&private_key, p_g, params);
    let is_valid_signature =
        pk.verify_musig_sha256(&message_bytes, &signature.clone(), p_g, params);
    if !is_valid_signature {
        return None;
    }

    let mut sigs_le_bits: Vec<bool> = BitIterator::new(signature.s.into_repr()).collect();
    sigs_le_bits.reverse();

    let sigs_converted = le_bit_vector_into_field_element(&sigs_le_bits);

    Some(TransactionSignature {
        r: signature.r,
        s: sigs_converted,
    })
}
pub fn multi_and<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    x: &[Boolean],
) -> Result<Boolean, SynthesisError> {
    let mut result = Boolean::constant(true);

    for (i, bool_x) in x.iter().enumerate() {
        result = Boolean::and(
            cs.namespace(|| format!("multi and iteration number: {}", i)),
            &result,
            bool_x,
        )?;
    }

    Ok(result)
}

pub fn allocate_sum<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    a: &AllocatedNum<E>,
    b: &AllocatedNum<E>,
) -> Result<AllocatedNum<E>, SynthesisError> {
    let sum = AllocatedNum::alloc(cs.namespace(|| "sum"), || {
        let mut sum = a.get_value().grab()?;
        sum.add_assign(&b.get_value().grab()?);
        Ok(sum)
    })?;
    cs.enforce(
        || "enforce sum",
        |lc| lc + a.get_variable() + b.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + sum.get_variable(),
    );

    Ok(sum)
}

pub fn pack_bits_to_element<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    bits: &[Boolean],
) -> Result<AllocatedNum<E>, SynthesisError> {
    let mut data_from_lc = Num::<E>::zero();
    let mut coeff = E::Fr::one();
    for bit in bits {
        data_from_lc = data_from_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let data_packed = AllocatedNum::alloc(cs.namespace(|| "allocate account data packed"), || {
        Ok(*data_from_lc.get_value().get()?)
    })?;

    cs.enforce(
        || "pack account data",
        |lc| lc + data_packed.get_variable(),
        |lc| lc + CS::one(),
        |_| data_from_lc.lc(E::Fr::one()),
    );

    Ok(data_packed)
}

// count a number of non-zero bits in a bit decomposition
pub fn count_number_of_ones<E, CS>(
    mut cs: CS,
    a: &[Boolean],
) -> Result<AllocatedNum<E>, SynthesisError>
where
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
{
    let mut counter = Num::zero();
    for bit in a.iter() {
        counter = counter.add_bool_with_coeff(CS::one(), &bit, E::Fr::one());
    }

    let result = AllocatedNum::alloc(cs.namespace(|| "number of zeroes number"), || {
        Ok(*counter.get_value().get()?)
    })?;

    cs.enforce(
        || "pack number of ones",
        |lc| lc + result.get_variable(),
        |lc| lc + CS::one(),
        |_| counter.lc(E::Fr::one()),
    );

    Ok(result)
}

pub fn allocate_numbers_vec<E, CS>(
    mut cs: CS,
    audit_path: &[Option<E::Fr>],
) -> Result<Vec<AllocatedNum<E>>, SynthesisError>
where
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
{
    let mut allocated = vec![];
    for (i, e) in audit_path.iter().enumerate() {
        let path_element =
            AllocatedNum::alloc(cs.namespace(|| format!("path element{}", i)), || {
                Ok(*e.get()?)
            })?;
        allocated.push(path_element);
    }

    Ok(allocated)
}

pub fn allocate_bits_vector<E, CS>(
    mut cs: CS,
    bits: &[Option<bool>],
) -> Result<Vec<Boolean>, SynthesisError>
where
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
{
    let mut allocated = vec![];
    for (i, e) in bits.iter().enumerate() {
        let element = Boolean::from(AllocatedBit::alloc(
            cs.namespace(|| format!("path element{}", i)),
            *e,
        )?);
        allocated.push(element);
    }

    Ok(allocated)
}

pub fn append_packed_public_key(
    content: &mut Vec<Boolean>,
    x_bits: Vec<Boolean>,
    y_bits: Vec<Boolean>,
) {
    assert_eq!(franklin_constants::FR_BIT_WIDTH - 1, y_bits.len());
    assert_eq!(1, x_bits.len());
    content.extend(y_bits);
    content.extend(x_bits);
}
