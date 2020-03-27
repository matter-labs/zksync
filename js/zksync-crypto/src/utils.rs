use crate::JUBJUB_PARAMS;
use crate::{Engine, Fr};
use crypto_exports::franklin_crypto::{
    bellman::{pairing::ff::PrimeField, BitIterator},
    eddsa::PublicKey,
    pedersen_hash::{baby_pedersen_hash, Personalization},
};

const FR_BIT_WIDTH_PADDED: usize = 256;
const PAD_MSG_BEFORE_HASH_BITS_LEN: usize = 736;
const NEW_PUBKEY_HASH_WIDTH: usize = 160;

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

pub fn bytes_into_be_bits(bytes: &[u8]) -> Vec<bool> {
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    for byte in bytes {
        let mut temp = *byte;
        for _ in 0..8 {
            bits.push(temp & 0x80 == 0x80);
            temp <<= 1;
        }
    }
    bits
}

pub fn pack_bits_into_bytes(bits: Vec<bool>) -> Vec<u8> {
    let mut message_bytes: Vec<u8> = Vec::with_capacity(bits.len() / 8);
    let byte_chunks = bits.chunks(8);
    for byte_chunk in byte_chunks {
        let mut byte = 0u8;
        for (i, bit) in byte_chunk.iter().enumerate() {
            if *bit {
                byte |= 1 << i;
            }
        }
        message_bytes.push(byte);
    }
    message_bytes
}

pub fn append_le_fixed_width(content: &mut Vec<bool>, x: &Fr, width: usize) {
    let mut token_bits: Vec<bool> = BitIterator::new(x.into_repr()).collect();
    token_bits.reverse();
    token_bits.resize(width, false);
    content.extend(token_bits);
}

pub fn le_bit_vector_into_bytes(bits: &[bool]) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::with_capacity(bits.len() / 8);

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

pub fn pub_key_hash(pub_key: &PublicKey<Engine>) -> Vec<u8> {
    let (pub_x, pub_y) = pub_key.0.into_xy();
    let mut pub_key_bits = Vec::with_capacity(FR_BIT_WIDTH_PADDED * 2);
    append_le_fixed_width(&mut pub_key_bits, &pub_x, FR_BIT_WIDTH_PADDED);
    append_le_fixed_width(&mut pub_key_bits, &pub_y, FR_BIT_WIDTH_PADDED);
    let pub_key_hash = pedersen_hash_fr(pub_key_bits);
    let mut pub_key_hash_bits = Vec::with_capacity(NEW_PUBKEY_HASH_WIDTH);
    append_le_fixed_width(&mut pub_key_hash_bits, &pub_key_hash, NEW_PUBKEY_HASH_WIDTH);
    let mut bytes = le_bit_vector_into_bytes(&pub_key_hash_bits);
    bytes.reverse();
    bytes
}

fn pedersen_hash_fr(input: Vec<bool>) -> Fr {
    JUBJUB_PARAMS.with(|params| {
        baby_pedersen_hash::<Engine, _>(Personalization::NoteCommitment, input, params)
            .into_xy()
            .0
    })
}

fn pedersen_hash_bits(input: Vec<bool>) -> Vec<bool> {
    let hash_fr = pedersen_hash_fr(input);
    let mut hash_bits: Vec<bool> = BitIterator::new(hash_fr.into_repr()).collect();
    hash_bits.reverse();
    hash_bits.resize(256, false);
    hash_bits
}

pub fn pedersen_hash_tx_msg(msg: &[u8]) -> Vec<u8> {
    let mut msg_bits = bytes_into_be_bits(msg);
    msg_bits.resize(PAD_MSG_BEFORE_HASH_BITS_LEN, false);
    let hash_bits = pedersen_hash_bits(msg_bits);
    pack_bits_into_bytes(hash_bits)
}
