// Built-in deps
use std::convert::TryInto;
use std::str::FromStr;
// External deps
use bigdecimal::BigDecimal;
use failure::bail;
use ff::ScalarEngine;
use ff::{BitIterator, Field, PrimeField, PrimeFieldRepr};
use franklin_crypto::jubjub::{edwards, JubjubEngine, Unknown};
use pairing::bn256::Bn256;
use pairing::{CurveAffine, Engine};
use web3::types::U256;
// Workspace deps
use crate::circuit::utils::append_le_fixed_width;
use crate::merkle_tree::{hasher::Hasher, pedersen_hasher::BabyPedersenHasher};
use crate::params;

// TODO: replace Vec with Iterator?

pub trait GetBits {
    fn get_bits_le(&self) -> Vec<bool>;
}

impl GetBits for u64 {
    fn get_bits_le(&self) -> Vec<bool> {
        // TODO: - Check function because it may be wrong
        let mut acc = Vec::new();
        let mut i = *self + 1;
        for _ in 0..16 {
            acc.push(i & 1 == 1);
            i >>= 1;
        }
        acc
    }
}

pub trait GetBitsFixed {
    /// Get exactly `n` bits from the value in little endian order
    /// If `n` is larger than value bit length, it is padded with `false`
    /// for the result to exactly match `n`
    fn get_bits_le_fixed(&self, n: usize) -> Vec<bool>;
}

pub fn get_bits_le_fixed_u128(num: u128, n: usize) -> Vec<bool> {
    let mut r: Vec<bool> = Vec::with_capacity(n);
    let it_end = if n > 128 { 128 } else { n };
    let mut tmp = num;
    for _ in 0..it_end {
        let bit = tmp & 1u128 > 0;
        r.push(bit);
        tmp >>= 1;
    }
    r.resize(n, false);

    r
}

pub fn get_bits_le_fixed_big_decimal(num: BigDecimal, n: usize) -> Vec<bool> {
    let as_u128 = big_decimal_to_u128(&num);

    get_bits_le_fixed_u128(as_u128, n)
}

impl<Fr: PrimeField> GetBitsFixed for Fr {
    fn get_bits_le_fixed(&self, n: usize) -> Vec<bool> {
        let mut r: Vec<bool> = Vec::with_capacity(n);
        r.extend(BitIteratorLe::new(self.into_repr()).take(n));
        let len = r.len();
        r.extend((len..n).map(|_| false));
        r
    }
}

pub fn field_element_to_u32<P: PrimeField>(fr: P) -> u32 {
    let mut iterator: Vec<bool> = BitIterator::new(fr.into_repr()).collect();
    iterator.reverse();
    iterator.truncate(32);
    let mut res = 0u32;
    let mut base = 1u32;
    for bit in iterator {
        if bit {
            res += base;
        }
        base <<= 1;
    }

    res
}

pub fn field_element_to_u128<P: PrimeField>(fr: P) -> u128 {
    let mut iterator: Vec<bool> = BitIterator::new(fr.into_repr()).collect();
    iterator.reverse();
    iterator.truncate(128);
    let mut res = 0u128;
    let mut base = 1u128;
    for bit in iterator {
        if bit {
            res += base;
        }
        base <<= 1;
    }

    res
}

pub fn serialize_g1_for_ethereum(point: <Bn256 as Engine>::G1Affine) -> (U256, U256) {
    let uncompressed = point.into_uncompressed();

    let uncompressed_slice = uncompressed.as_ref();

    // bellman serializes points as big endian and in the form x, y
    // ethereum expects the same order in memory
    let x = U256::from_big_endian(&uncompressed_slice[0..32]);
    let y = U256::from_big_endian(&uncompressed_slice[32..64]);

    (x, y)
}

pub fn serialize_g2_for_ethereum(
    point: <Bn256 as Engine>::G2Affine,
) -> ((U256, U256), (U256, U256)) {
    let uncompressed = point.into_uncompressed();

    let uncompressed_slice = uncompressed.as_ref();

    // bellman serializes points as big endian and in the form x1*u, x0, y1*u, y0
    // ethereum expects the same order in memory
    let x_1 = U256::from_big_endian(&uncompressed_slice[0..32]);
    let x_0 = U256::from_big_endian(&uncompressed_slice[32..64]);
    let y_1 = U256::from_big_endian(&uncompressed_slice[64..96]);
    let y_0 = U256::from_big_endian(&uncompressed_slice[96..128]);

    ((x_1, x_0), (y_1, y_0))
}

pub fn serialize_fe_for_ethereum(field_element: <Bn256 as ScalarEngine>::Fr) -> U256 {
    let mut be_bytes = [0u8; 32];
    field_element
        .into_repr()
        .write_be(&mut be_bytes[..])
        .expect("get new root BE bytes");
    U256::from_big_endian(&be_bytes[..])
}

pub fn unpack_edwards_point<E: JubjubEngine>(
    serialized: [u8; 32],
    params: &E::Params,
) -> Result<edwards::Point<E, Unknown>, String> {
    // TxSignature has S and R in compressed form serialized as BE
    let x_sign = serialized[0] & 0x80 > 0;
    let mut tmp = serialized;
    tmp[0] &= 0x7f; // strip the top bit

    // read from byte array
    let mut y_repr = E::Fr::zero().into_repr();
    y_repr.read_be(&tmp[..]).expect("read R_y as field element");

    let y = E::Fr::from_repr(y_repr).expect("make y from representation");

    // here we convert it to field elements for all further uses
    let r = edwards::Point::get_for_y(y, x_sign, params);
    if r.is_none() {
        return Err("Invalid R point".to_string());
    }

    Ok(r.unwrap())
}

pub fn pack_edwards_point<E: JubjubEngine>(
    point: edwards::Point<E, Unknown>,
) -> Result<[u8; 32], String> {
    let mut tmp = [0u8; 32];
    let (y, sign) = point.compress_into_y();
    y.into_repr().write_be(&mut tmp[..]).expect("write y");
    if sign {
        tmp[0] |= 0x80
    }

    Ok(tmp)
}

#[test]
fn test_get_bits() {
    use pairing::bn256::Fr;

    // 12 = b1100, 3 lowest bits in little endian encoding are: 0, 0, 1.
    let bits = Fr::from_str("12").unwrap().get_bits_le_fixed(3);
    assert_eq!(bits, vec![false, false, true]);

    let bits = Fr::from_str("0").unwrap().get_bits_le_fixed(512);
    assert_eq!(bits, vec![false; 512]);
}

//

// Resulting iterator is little endian: lowest bit first

#[derive(Debug)]
pub struct BitIteratorLe<E> {
    t: E,
    n: usize,
    len: usize,
}

impl<E: AsRef<[u64]>> BitIteratorLe<E> {
    pub fn new(t: E) -> Self {
        let len = t.as_ref().len() * 64;

        BitIteratorLe { t, n: 0, len }
    }
}

impl<E: AsRef<[u64]>> Iterator for BitIteratorLe<E> {
    type Item = bool;

    fn next(&mut self) -> Option<bool> {
        if self.n == self.len {
            None
        } else {
            let part = self.n / 64;
            let bit = self.n - (64 * part);
            self.n += 1;

            Some(self.t.as_ref()[part] & (1 << bit) > 0)
        }
    }
}

pub fn pack_bits_into_bytes(bits: Vec<bool>) -> Vec<u8> {
    assert_eq!(bits.len() % 8, 0);
    let mut message_bytes: Vec<u8> = vec![];

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

pub fn pack_bits_into_bytes_in_order(bits: Vec<bool>) -> Vec<u8> {
    assert_eq!(bits.len() % 8, 0);
    let mut message_bytes: Vec<u8> = vec![];

    let byte_chunks = bits.chunks(8);
    for byte_chunk in byte_chunks {
        let mut byte = 0u8;
        for (i, bit) in byte_chunk.iter().rev().enumerate() {
            if *bit {
                byte |= 1 << i;
            }
        }
        message_bytes.push(byte);
    }

    message_bytes
}

pub fn unpack_float(data: &[u8], exponent_len: usize, mantissa_len: usize) -> Option<u128> {
    if exponent_len + mantissa_len != data.len() * 8 {
        return None;
    }

    let bits = bytes_into_be_bits(data);

    let mut mantissa = 0u128;
    for (i, bit) in bits[0..mantissa_len].iter().rev().enumerate() {
        if *bit {
            mantissa = mantissa.checked_add(1u128 << i)?;
        }
    }

    let mut exponent_pow = 0u32;
    for (i, bit) in bits[mantissa_len..(mantissa_len + exponent_len)]
        .iter()
        .rev()
        .enumerate()
    {
        if *bit {
            exponent_pow = exponent_pow.checked_add(1u32 << i)?;
        }
    }

    let exponent = 10u128.checked_pow(exponent_pow)?;

    mantissa.checked_mul(exponent)
}

pub fn pack_as_float(number: &BigDecimal, exponent_len: usize, mantissa_len: usize) -> Vec<u8> {
    let uint = big_decimal_to_u128(number);

    let mut vec = convert_to_float(uint, exponent_len, mantissa_len, 10).expect("packing error");
    vec.reverse();
    pack_bits_into_bytes_in_order(vec)
}

pub fn unpack_as_big_decimal(
    bytes: &[u8],
    exponent_len: usize,
    mantissa_len: usize,
) -> Option<BigDecimal> {
    let amount_u128: u128 = unpack_float(bytes, exponent_len, mantissa_len)?;
    BigDecimal::from_str(&amount_u128.to_string()).ok()
}

pub fn convert_to_float(
    integer: u128,
    exponent_length: usize,
    mantissa_length: usize,
    exponent_base: u32,
) -> Result<Vec<bool>, failure::Error> {
    let exponent_base = u128::from(exponent_base);

    let mut max_exponent = 1u128;
    let max_power = (1 << exponent_length) - 1;

    for _ in 0..max_power {
        max_exponent = max_exponent.saturating_mul(exponent_base);
    }

    let max_mantissa = (1u128 << mantissa_length) - 1;

    if integer > (max_mantissa.saturating_mul(max_exponent)) {
        bail!("Integer is too big");
    }

    let mut exponent: usize = 0;
    let mantissa = if integer > max_mantissa {
        // always try best precision
        let exponent_guess = integer / max_mantissa;
        let mut exponent_temp = exponent_guess;

        loop {
            if exponent_temp < exponent_base {
                break;
            }
            exponent_temp /= exponent_base;
            exponent += 1;
        }

        exponent_temp = 1u128;
        for _ in 0..exponent {
            exponent_temp *= exponent_base;
        }

        if exponent_temp * max_mantissa < integer {
            exponent += 1;
            exponent_temp *= exponent_base;
        }

        integer / exponent_temp
    } else {
        integer
    };

    // encode into bits. First bits of mantissa in LE order

    let mut encoding = Vec::with_capacity(exponent_length + mantissa_length);

    for i in 0..exponent_length {
        if exponent & (1 << i) != 0 {
            encoding.push(true);
        } else {
            encoding.push(false);
        }
    }

    for i in 0..mantissa_length {
        if mantissa & (1 << i) != 0 {
            encoding.push(true);
        } else {
            encoding.push(false);
        }
    }

    debug_assert_eq!(encoding.len(), exponent_length + mantissa_length);

    Ok(encoding)
}

pub fn bytes_into_be_bits(bytes: &[u8]) -> Vec<bool> {
    let mut bits = vec![];
    for byte in bytes {
        let mut temp = *byte;
        for _ in 0..8 {
            bits.push(temp & 0x80 == 0x80);
            temp <<= 1;
        }
    }
    bits
}

pub fn pedersen_hash_tx_msg(msg: &[u8]) -> Vec<u8> {
    let mut msg_bits = bytes_into_be_bits(msg);
    msg_bits.resize(params::PAD_MSG_BEFORE_HASH_BITS_LEN, false);
    let hasher = &params::PEDERSEN_HASHER as &BabyPedersenHasher;
    let hash_fr = hasher.hash_bits(msg_bits.into_iter());
    let mut hash_bits = Vec::new();
    append_le_fixed_width(&mut hash_bits, &hash_fr, 256);
    pack_bits_into_bytes(hash_bits)
}

/// Its important to use this, instead of bit_decimal.to_u128()
pub fn big_decimal_to_u128(big_decimal: &BigDecimal) -> u128 {
    big_decimal.to_string().parse().unwrap()
}

/// Its important to use this, instead of BigDecimal::from_u128()
pub fn u128_to_bigdecimal(n: u128) -> BigDecimal {
    n.to_string().parse().unwrap()
}

pub fn bytes_slice_to_uint32(bytes: &[u8]) -> Option<u32> {
    let size = bytes.len();
    let mut vec: Vec<u8> = bytes.to_vec();
    vec.reverse();
    for _ in 0..4 - size {
        vec.push(0);
    }
    vec.reverse();
    let new_bytes = vec.as_slice();
    Some(u32::from_be_bytes(new_bytes.try_into().ok()?))
}

pub fn bytes_slice_to_uint16(bytes: &[u8]) -> Option<u16> {
    let size = bytes.len();
    let mut vec: Vec<u8> = bytes.to_vec();
    vec.reverse();
    for _ in 0..2 - size {
        vec.push(0);
    }
    vec.reverse();
    let new_bytes = vec.as_slice();
    Some(u16::from_be_bytes(new_bytes.try_into().ok()?))
}

pub fn bytes_slice_to_uint128(bytes: &[u8]) -> Option<u128> {
    let size = bytes.len();
    let mut vec: Vec<u8> = bytes.to_vec();
    vec.reverse();
    for _ in 0..16 - size {
        vec.push(0);
    }
    vec.reverse();
    let new_bytes = vec.as_slice();
    Some(u128::from_be_bytes(new_bytes.try_into().ok()?))
}

pub fn bytes32_from_slice(bytes: &[u8]) -> Option<[u8; 32]> {
    if bytes.len() != 32 {
        return None;
    }
    let mut array = [0; 32];
    let bytes = &bytes[..array.len()];
    array.copy_from_slice(bytes);
    Some(array)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bit_iterator_e() {
        let test_vector = [0xa953_d79b_83f6_ab59, 0x6dea_2059_e200_bd39];
        let mut reference: Vec<bool> = BitIterator::new(&test_vector).collect();
        reference.reverse();
        let out: Vec<bool> = BitIteratorLe::new(&test_vector).collect();
        assert_eq!(reference, out);
    }
}
