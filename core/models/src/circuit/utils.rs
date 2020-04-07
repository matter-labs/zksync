use crate::params;

use crate::franklin_crypto::alt_babyjubjub::JubjubEngine;
use crate::franklin_crypto::bellman::pairing::ff::{BitIterator, PrimeField};
use crate::franklin_crypto::eddsa::PublicKey;
use crate::merkle_tree::hasher::Hasher;
use crate::node::Fr;
use web3::types::Address;

// fn pub_key_hash_bits<E: JubjubEngine, H: Hasher<E::Fr>>(
//     pub_key: &PublicKey<E>,
//     hasher: &H,
// ) -> Vec<bool> {
//     let (pub_x, pub_y) = pub_key.0.into_xy();
//     let mut pub_key_bits = vec![];
//     append_le_fixed_width(&mut pub_key_bits, &pub_x, params::FR_BIT_WIDTH_PADDED);
//     append_le_fixed_width(&mut pub_key_bits, &pub_y, params::FR_BIT_WIDTH_PADDED);
//     let pub_key_hash = hasher.hash_bits(pub_key_bits);
//     let mut pub_key_hash_bits = vec![];
//     append_le_fixed_width(
//         &mut pub_key_hash_bits,
//         &pub_key_hash,
//         params::NEW_PUBKEY_HASH_WIDTH,
//     );
//     pub_key_hash_bits
// }

fn pub_key_hash_self<E: JubjubEngine, H: Hasher<E::Fr>>(
    pub_key: &PublicKey<E>,
    hasher: &H,
) -> Vec<bool> {
    let (pub_x, pub_y) = pub_key.0.into_xy();
    let input = vec![pub_x, pub_y];
    let pub_key_hash = hasher.hash_elements(input);
    let mut pub_key_hash_bits = vec![];
    append_le_fixed_width(
        &mut pub_key_hash_bits,
        &pub_key_hash,
        params::NEW_PUBKEY_HASH_WIDTH,
    );
    pub_key_hash_bits
}

pub fn pub_key_hash_fe<E: JubjubEngine, H: Hasher<E::Fr>>(
    pub_key: &PublicKey<E>,
    hasher: &H,
) -> E::Fr {
    let pk_hash_bits = pub_key_hash_self(pub_key, hasher);
    le_bit_vector_into_field_element(&pk_hash_bits)
}

pub fn pub_key_hash_bytes<E: JubjubEngine, H: Hasher<E::Fr>>(
    pub_key: &PublicKey<E>,
    hasher: &H,
) -> Vec<u8> {
    let pk_hash_bits = pub_key_hash_self(pub_key, hasher);
    le_bit_vector_into_bytes(&pk_hash_bits)
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

pub fn append_le_fixed_width<P: PrimeField>(content: &mut Vec<bool>, x: &P, width: usize) {
    let mut token_bits: Vec<bool> = BitIterator::new(x.into_repr()).collect();
    token_bits.reverse();
    // token_bits.truncate(width);
    token_bits.resize(width, false);
    content.extend(token_bits);
}

pub fn append_be_fixed_width<P: PrimeField>(content: &mut Vec<bool>, x: &P, width: usize) {
    let mut token_bits: Vec<bool> = BitIterator::new(x.into_repr()).collect();
    token_bits.reverse();
    token_bits.resize(width, false);
    token_bits.reverse();
    content.extend(token_bits);
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

pub fn eth_address_to_fr(address: &Address) -> Fr {
    Fr::from_hex(&format!("{:x}", address)).unwrap()
}
