use std::time::Duration;
use std::time::Instant;

use num::BigUint;
use sha2::{Digest, Sha256};

use zksync_crypto::bellman::{pairing::ff::PrimeField, PrimeFieldRepr};
use zksync_crypto::franklin_crypto::alt_babyjubjub::fs::FsRepr;
use zksync_crypto::{priv_key_from_fs, Fs, PrivateKey};
use zksync_eth_signer::EthereumSigner;
use zksync_types::{AccountId, U256};

use crate::{error::ClientError, provider::Provider, wallet::Wallet};

// Public re-exports.
pub use zksync_types::helpers::{
    closest_greater_or_eq_packable_fee_amount, closest_greater_or_eq_packable_token_amount,
    closest_packable_fee_amount, closest_packable_token_amount, is_fee_amount_packable,
    is_token_amount_packable, pack_fee_amount, pack_token_amount,
};

/// Generates a new `PrivateKey` from seed using a deterministic algorithm:
/// seed is hashed via `sha256` hash (twice), and the output treated as a `PrivateKey`.
/// If the obtained value doesn't have a correct value to be a `PrivateKey`, hashing operation is applied
/// repeatedly to the previous output, until the value can be interpreted as a `PrivateKey`.
pub fn private_key_from_seed(seed: &[u8]) -> Result<PrivateKey, ClientError> {
    if seed.len() < 32 {
        return Err(ClientError::SeedTooShort);
    }

    let sha256_bytes = |input: &[u8]| {
        let mut hasher = Sha256::new();
        hasher.input(input);
        hasher.result()
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

///
/// Waits until there is a zkSync account ID associated with the `wallet`.
///
/// Should be used after making the initial deposit or transfer to a newly created account.
///
pub async fn wait_for_account_id<S, P>(
    wallet: &mut Wallet<S, P>,
    timeout_ms: u64,
) -> Option<AccountId>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
    let timeout = Duration::from_millis(timeout_ms);
    let mut poller = tokio::time::interval(Duration::from_millis(100));
    let start = Instant::now();

    while wallet
        .provider
        .account_info(wallet.address())
        .await
        .ok()?
        .id
        .is_none()
    {
        if start.elapsed() > timeout {
            return None;
        }

        poller.tick().await;
    }

    wallet.update_account_id().await.ok()?;

    wallet.account_id()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn biguint_u256_conversion_roundrip(u256: U256) {
        let biguint = u256_to_biguint(u256);
        // Make sure that the string representations are the same.
        assert_eq!(biguint.to_string(), u256.to_string());

        let restored = biguint_to_u256(biguint);
        assert_eq!(u256, restored);
    }

    #[test]
    fn test_zero_conversion() {
        biguint_u256_conversion_roundrip(U256::zero())
    }

    #[test]
    fn test_biguint_u256_conversion() {
        // random value that is big enough
        let u256 = U256::from(1_235_999_123_u64).pow(4u64.into());
        biguint_u256_conversion_roundrip(u256)
    }

    #[test]
    fn test_biguint_with_msb_conversion() {
        // make sure the most significant bit was set
        let u256 = U256::from_big_endian(&[0b11010011; 32]);
        biguint_u256_conversion_roundrip(u256)
    }

    #[test]
    fn test_private_key_from_seed_too_short() {
        let short_seed = &[42; 30];

        let pk_err = private_key_from_seed(short_seed).map(|_| ()).unwrap_err();
        assert_eq!(pk_err, ClientError::SeedTooShort);
    }
}
