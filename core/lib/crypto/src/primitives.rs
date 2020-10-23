// Built-in deps
use std::convert::TryInto;
use std::mem;
// External deps
use crate::franklin_crypto::bellman::pairing::bn256::Bn256;
use crate::franklin_crypto::bellman::pairing::ff::ScalarEngine;
use crate::franklin_crypto::bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
use crate::franklin_crypto::bellman::pairing::{CurveAffine, Engine};
use anyhow::bail;
use num::{BigUint, ToPrimitive};
use zksync_basic_types::U256;
// Workspace deps
use crate::circuit::utils::append_le_fixed_width;
use crate::merkle_tree::{hasher::Hasher, rescue_hasher::BabyRescueHasher};
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

impl<Fr: PrimeField> GetBitsFixed for Fr {
    fn get_bits_le_fixed(&self, n: usize) -> Vec<bool> {
        let mut r: Vec<bool> = Vec::with_capacity(n);
        r.extend(BitIteratorLe::new(self.into_repr()).take(n));
        let len = r.len();
        r.extend((len..n).map(|_| false));
        r
    }
}

pub struct EthereumSerializer;

impl EthereumSerializer {
    pub fn serialize_g1(point: &<Bn256 as Engine>::G1Affine) -> (U256, U256) {
        if point.is_zero() {
            return (U256::zero(), U256::zero());
        }
        let uncompressed = point.into_uncompressed();

        let uncompressed_slice = uncompressed.as_ref();

        // bellman serializes points as big endian and in the form x, y
        // ethereum expects the same order in memory
        let x = U256::from_big_endian(&uncompressed_slice[0..32]);
        let y = U256::from_big_endian(&uncompressed_slice[32..64]);

        (x, y)
    }

    pub fn serialize_g2(point: &<Bn256 as Engine>::G2Affine) -> ((U256, U256), (U256, U256)) {
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

    pub fn serialize_fe(field_element: &<Bn256 as ScalarEngine>::Fr) -> U256 {
        let mut be_bytes = [0u8; 32];
        field_element
            .into_repr()
            .write_be(&mut be_bytes[..])
            .expect("get new root BE bytes");
        U256::from_big_endian(&be_bytes[..])
    }
}

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

pub struct BitConvert;

impl BitConvert {
    /// Сonverts a set of bits to a set of bytes in direct order.
    #[allow(clippy::wrong_self_convention)]
    pub fn into_bytes(bits: Vec<bool>) -> Vec<u8> {
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

    /// Сonverts a set of bits to a set of bytes in reverse order for each byte.
    #[allow(clippy::wrong_self_convention)]
    pub fn into_bytes_ordered(bits: Vec<bool>) -> Vec<u8> {
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

    /// Сonverts a set of Big Endian bytes to a set of bits.
    pub fn from_be_bytes(bytes: &[u8]) -> Vec<bool> {
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
}

/// Convert Uint to the floating-point and vice versa.
pub struct FloatConversions;

impl FloatConversions {
    /// Packs a BigUint less than 2 ^ 128  to a floating-point number with an exponent base = 10.
    /// Can lose accuracy with small parameters `exponent_len` and `mantissa_len`.
    pub fn pack(number: &BigUint, exponent_len: usize, mantissa_len: usize) -> Vec<u8> {
        let uint = number.to_u128().expect("Only u128 allowed");

        let mut vec = Self::to_float(uint, exponent_len, mantissa_len, 10).expect("packing error");
        vec.reverse();
        BitConvert::into_bytes_ordered(vec)
    }

    /// Unpacks a floating point number with the given parameters.
    /// Returns `None` for numbers greater than 2 ^ 128.
    pub fn unpack(data: &[u8], exponent_len: usize, mantissa_len: usize) -> Option<u128> {
        if exponent_len + mantissa_len != data.len() * 8 {
            return None;
        }

        let bits = BitConvert::from_be_bytes(data);

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

    /// Packs a u128 to a floating-point number with the given parameters.
    /// Can lose accuracy with small parameters `exponent_len` and `mantissa_len`.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_float(
        integer: u128,
        exponent_length: usize,
        mantissa_length: usize,
        exponent_base: u32,
    ) -> Result<Vec<bool>, anyhow::Error> {
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
}

pub fn rescue_hash_tx_msg(msg: &[u8]) -> Vec<u8> {
    let mut msg_bits = BitConvert::from_be_bytes(msg);
    msg_bits.resize(params::PAD_MSG_BEFORE_HASH_BITS_LEN, false);
    let hasher = &params::RESCUE_HASHER as &BabyRescueHasher;
    let hash_fr = hasher.hash_bits(msg_bits.into_iter());
    let mut hash_bits = Vec::new();
    append_le_fixed_width(&mut hash_bits, &hash_fr, 256);
    BitConvert::into_bytes(hash_bits)
}

pub trait FromBytes: Sized {
    /// Converts a sequence of bytes to a number.
    fn from_bytes(bytes: &[u8]) -> Option<Self>;
}

macro_rules! impl_from_bytes_for_primitive {
    ($Type:ty) => {
        impl FromBytes for $Type {
            fn from_bytes(bytes: &[u8]) -> Option<Self> {
                const COUNT: usize = mem::size_of::<$Type>();
                let mut vec: Vec<u8> = bytes.to_vec();
                vec.reverse();
                vec.extend(vec![0; COUNT - bytes.len()]);
                vec.reverse();
                let new_bytes = vec.as_slice();
                Some(Self::from_be_bytes(new_bytes.try_into().ok()?))
            }
        }
    };
}

impl_from_bytes_for_primitive!(u16);
impl_from_bytes_for_primitive!(u32);
impl_from_bytes_for_primitive!(u128);

#[cfg(test)]
mod test {
    use super::*;
    use crate::franklin_crypto::bellman::pairing::ff::BitIterator;

    #[test]
    fn test_get_bits() {
        use crate::franklin_crypto::bellman::pairing::bn256::Fr;

        // 12 = b1100, 3 lowest bits in little endian encoding are: 0, 0, 1.
        let bits = Fr::from_str("12").unwrap().get_bits_le_fixed(3);
        assert_eq!(bits, vec![false, false, true]);

        let bits = Fr::from_str("0").unwrap().get_bits_le_fixed(512);
        assert_eq!(bits, vec![false; 512]);
    }

    #[test]
    fn test_bits_conversions() {
        let mut bits = vec![];

        bits.extend(vec![true, false, false, true, true, false, true, false]);
        bits.extend(vec![false, false, true, true, false, true, true, false]);
        bits.extend(vec![false, false, false, false, false, false, false, true]);

        let bytes = BitConvert::into_bytes(bits.clone());
        assert_eq!(bytes, vec![89, 108, 128]);

        let bytes = BitConvert::into_bytes_ordered(bits.clone());
        assert_eq!(bytes, vec![154, 54, 1]);

        assert_eq!(BitConvert::from_be_bytes(&[154, 54, 1]), bits);
    }

    #[test]
    fn test_float_conversions() {
        let (number, exponent_len, mantissa_len, exponent_base): (u128, usize, usize, u32) =
            (0xDEADBEAF, 5, 35, 10);

        let packed_number =
            FloatConversions::pack(&num::BigUint::from(number), exponent_len, mantissa_len);
        let unpacked_number = FloatConversions::unpack(&packed_number, exponent_len, mantissa_len);
        let convert_number =
            FloatConversions::to_float(number, exponent_len, mantissa_len, exponent_base);

        assert_eq!(unpacked_number, Some(number));
        assert_eq!(packed_number, vec![27, 213, 183, 213, 224]);
        assert_eq!(
            convert_number.ok(),
            Some(vec![
                false, false, false, false, false, true, true, true, true, false, true, false,
                true, false, true, true, true, true, true, false, true, true, false, true, true,
                false, true, false, true, false, true, true, true, true, false, true, true, false,
                false, false
            ])
        );
    }

    #[test]
    fn test_rescue_hash_tx_msg() {
        let msg = [1u8, 2u8, 3u8, 4u8];
        let hash = rescue_hash_tx_msg(&msg);

        assert_eq!(
            hash,
            vec![
                249, 154, 208, 123, 96, 89, 132, 235, 231, 63, 56, 200, 153, 131, 27, 183, 128, 71,
                26, 245, 208, 120, 49, 246, 233, 72, 230, 84, 66, 150, 170, 27
            ]
        );
    }

    #[test]
    fn test_uint_from_bytes() {
        let bytes = vec![1; 1];
        let number: u32 = FromBytes::from_bytes(&bytes).unwrap();
        assert_eq!(number, 1);

        let bytes = [1u8, 2u8, 3u8, 4u8];
        let number: u32 = FromBytes::from_bytes(&bytes).unwrap();
        assert_eq!(number, 0x01020304);

        let bytes = [1u8, 2u8, 3u8, 4u8, 5u8];
        let number: u128 = FromBytes::from_bytes(&bytes).unwrap();
        assert_eq!(number, 0x0102030405);
    }

    #[test]
    fn test_bit_iterator_e() {
        let test_vector = [0xa953_d79b_83f6_ab59, 0x6dea_2059_e200_bd39];
        let mut reference: Vec<bool> = BitIterator::new(&test_vector).collect();
        reference.reverse();
        let out: Vec<bool> = BitIteratorLe::new(&test_vector).collect();
        assert_eq!(reference, out);
    }
}
