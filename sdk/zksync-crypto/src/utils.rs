use crate::RESCUE_PARAMS;
use crate::{Engine, Fr};
use franklin_crypto::{
    bellman::{pairing::ff::PrimeField, BitIterator},
    circuit::multipack,
    eddsa::PublicKey,
    rescue::rescue_hash,
};

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
    let pub_key_hash = rescue_hash_elements(&[pub_x, pub_y]);
    let mut pub_key_hash_bits = Vec::with_capacity(NEW_PUBKEY_HASH_WIDTH);
    append_le_fixed_width(&mut pub_key_hash_bits, &pub_key_hash, NEW_PUBKEY_HASH_WIDTH);
    let mut bytes = le_bit_vector_into_bytes(&pub_key_hash_bits);
    bytes.reverse();
    bytes
}

fn rescue_hash_fr(input: Vec<bool>) -> Fr {
    RESCUE_PARAMS.with(|params| {
        let packed = multipack::compute_multipacking::<Engine>(&input);
        let sponge_output = rescue_hash::<Engine>(params, &packed);
        assert_eq!(sponge_output.len(), 1, "rescue hash problem");
        sponge_output[0]
    })
}

fn rescue_hash_elements(input: &[Fr]) -> Fr {
    RESCUE_PARAMS.with(|params| {
        let sponge_output = rescue_hash::<Engine>(params, &input);
        assert_eq!(sponge_output.len(), 1, "rescue hash problem");
        sponge_output[0]
    })
}

pub fn rescue_hash_tx_msg(msg: &[u8]) -> Vec<u8> {
    let mut msg_bits = bytes_into_be_bits(msg);
    msg_bits.resize(PAD_MSG_BEFORE_HASH_BITS_LEN, false);
    let hash_fr = rescue_hash_fr(msg_bits);
    let mut hash_bits = Vec::new();
    append_le_fixed_width(&mut hash_bits, &hash_fr, 256);
    pack_bits_into_bytes(hash_bits)
}
