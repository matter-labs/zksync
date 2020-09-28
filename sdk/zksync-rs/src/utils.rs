use crate::error::ClientError;
use models::U256;
use num::BigUint;
use sha2::{Digest, Sha256};
use zksync_crypto::bellman::{pairing::ff::PrimeField, PrimeFieldRepr};
use zksync_crypto::franklin_crypto::alt_babyjubjub::fs::FsRepr;
use zksync_crypto::{priv_key_from_fs, Fs, PrivateKey};

// Public re-exports.
pub use models::helpers::{
    closest_packable_fee_amount, closest_packable_token_amount, is_fee_amount_packable,
    is_token_amount_packable,
};

/// Generates a new `PrivateKey` from seed using a deterministic algorithm:
/// seed is hashed via `sha256` hash, and the output treated as a `PrivateKey`.
/// If the obtained value doesn't have a correct value to be a `PrivateKey`, hashing operation is applied
/// repeatedly to the previous output, until the value can be interpreted as a `PrivateKey`.
pub fn private_key_from_seed(seed: &[u8]) -> Result<PrivateKey, ClientError> {
    if seed.len() < 32 {
        return Err(ClientError::SeedTooShort);
    }

    let sha256_bytes = |input: &[u8]| -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.input(input);
        hasher.result().to_vec()
    };

    let mut effective_seed = sha256_bytes(seed);

    loop {
        let raw_priv_key = sha256_bytes(&effective_seed);
        let mut fs_repr = FsRepr::default();
        fs_repr
            .read_be(&raw_priv_key[..])
            .expect("failed to read raw_priv_key");
        match Fs::from_repr(fs_repr) {
            Ok(fs) => return Ok(priv_key_from_fs(fs)),
            Err(_) => {
                effective_seed = raw_priv_key;
            }
        }
    }
}

/// Converts `U256` into the corresponding `BigUint` value.
pub fn u256_to_biguint(value: U256) -> BigUint {
    let mut bytes = [0u8; 32];
    value.to_little_endian(&mut bytes);
    BigUint::from_bytes_le(&bytes)
}

/// Converts `BigUint` value into the corresponding `U256` value.
pub fn biguint_to_u256(value: BigUint) -> U256 {
    let bytes = value.to_bytes_le();
    U256::from_little_endian(&bytes)
}

#[test]
fn test_biguint_u256_conversions() {
    // Make the value is big enough.
    let u256 = U256::from(1_235_999_123_u64).pow(4u64.into());

    let biguint = u256_to_biguint(u256);
    // Make sure that the string representations are the same.
    assert_eq!(biguint.to_string(), u256.to_string());

    let u256_2 = biguint_to_u256(biguint);

    assert_eq!(u256, u256_2);
}
