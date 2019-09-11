use bellman::{ConstraintSystem, SynthesisError};
use ff::{BitIterator, Field, PrimeField};

use franklin_crypto::circuit::boolean;
use franklin_crypto::circuit::num::{AllocatedNum, Num};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::eddsa::{PrivateKey, PublicKey};
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine};

use crate::operation::TransactionSignature;
use models::params as franklin_constants;

pub fn sign_pedersen<R, E>(
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
                byte |= 1 << i;
            }
        }
        message_bytes.push(byte);
    }

    let signature = private_key.musig_pedersen_sign(&message_bytes, rng, p_g, params);
    println!(
        "s: {:?} . r: {:?}",
        signature.clone().s,
        signature.clone().r.into_xy()
    );
    println!(" message_byts: {:?}", message_bytes.clone());

    let pk = PublicKey::from_private(&private_key, p_g, params);
    let is_valid_signature =
        pk.verify_musig_pedersen(&message_bytes, &signature.clone(), p_g, params);

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
    bits: &[boolean::Boolean],
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
    a: &[boolean::Boolean],
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

pub fn allocate_audit_path<E, CS>(
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
) -> Result<Vec<boolean::Boolean>, SynthesisError>
where
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
{
    let mut allocated = vec![];
    for (i, e) in bits.iter().enumerate() {
        let element = boolean::Boolean::from(boolean::AllocatedBit::alloc(
            cs.namespace(|| format!("path element{}", i)),
            e.clone(),
        )?);
        allocated.push(element);
    }

    Ok(allocated)
}

pub fn append_packed_public_key(
    content: &mut Vec<boolean::Boolean>,
    x_bits: Vec<boolean::Boolean>,
    y_bits: Vec<boolean::Boolean>,
) {
    assert_eq!(franklin_constants::FR_BIT_WIDTH - 1, y_bits.len());
    assert_eq!(1, x_bits.len());
    content.extend(y_bits);
    content.extend(x_bits);
}

pub fn append_le_fixed_width<P: PrimeField>(content: &mut Vec<bool>, x: &P, width: usize) {
    let mut token_bits: Vec<bool> = BitIterator::new(x.into_repr()).collect();
    token_bits.reverse();
    // token_bits.truncate(width);
    token_bits.resize(width, false);
    content.extend(token_bits.clone());
}

pub fn append_be_fixed_width<P: PrimeField>(content: &mut Vec<bool>, x: &P, width: usize) {
    let mut token_bits: Vec<bool> = BitIterator::new(x.into_repr()).collect();
    token_bits.reverse();
    token_bits.resize(width, false);
    token_bits.reverse();
    content.extend(token_bits.clone());
}
pub fn be_bytes_into_bits(bytes: &[u8]) -> Vec<bool> {
    let mut bits = vec![];
    for byte in bytes {
        for i in 0..8 {
            bits.push((byte >> (7 - i)) % 2u8 == 1u8);
        }
    }
    bits
}
pub fn le_bit_vector_into_field_element<P: PrimeField>(bits: &[bool]) -> P {
    // double and add
    let mut fe = P::zero();
    let mut base = P::one();

    for bit in bits {
        if *bit {
            fe.add_assign(&base);
        }
        base.double();
    }

    fe
}

pub fn be_bit_vector_into_bytes(bits: &[bool]) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![];

    let byte_chunks = bits.chunks(8);

    for byte_chunk in byte_chunks {
        let mut byte = 0u8;
        // pack just in order
        for (i, bit) in byte_chunk.iter().enumerate() {
            if *bit {
                byte |= 1 << (7 - i);
            }
        }
        bytes.push(byte);
    }

    bytes
}

pub fn le_bit_vector_into_bytes(bits: &[bool]) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![];

    let byte_chunks = bits.chunks(8);

    for byte_chunk in byte_chunks {
        let mut byte = 0u8;
        // pack just in order
        for (i, bit) in byte_chunk.iter().enumerate() {
            if *bit {
                byte |= 1 << i;
            }
        }
        bytes.push(byte);
    }

    bytes
}

pub fn encode_fs_into_fr<E: JubjubEngine>(input: E::Fs) -> E::Fr {
    let mut fs_le_bits: Vec<bool> = BitIterator::new(input.into_repr()).collect();
    fs_le_bits.reverse();

    le_bit_vector_into_field_element::<E::Fr>(&fs_le_bits)
}

pub fn encode_fr_into_fs<E: JubjubEngine>(input: E::Fr) -> E::Fs {
    let mut fr_le_bits: Vec<bool> = BitIterator::new(input.into_repr()).collect();
    fr_le_bits.reverse();

    le_bit_vector_into_field_element::<E::Fs>(&fr_le_bits)
}
