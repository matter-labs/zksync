use thiserror::Error;

use crate::eip712_signature::{EIP712TypedStructure, Eip712Domain};
use parity_crypto::{
    publickey::{public_to_address, recover, sign, KeyPair, Signature as ETHSignature},
    Keccak256,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zksync_basic_types::{Address, H256};
use zksync_utils::ZeroPrefixHexSerde;

/// Struct used for working with ethereum signatures created using eth_sign (using geth, ethers.js, etc)
/// message is serialized as 65 bytes long `0x` prefixed string.
///
/// Some notes on implementation of methods of this structure:
///
/// Ethereum signed message produced by most clients contains v where v = 27 + recovery_id(0,1,2,3),
/// but for some clients v = recovery_id(0,1,2,3).
/// Library that we use for signature verification (written for bitcoin) expects v = recovery_id
///
/// That is why:
/// 1) when we create this structure by deserialization of message produced by user
/// we subtract 27 from v in `ETHSignature` if necessary and store it in the `ETHSignature` structure this way.
/// 2) When we serialize/create this structure we add 27 to v in `ETHSignature`.
///
/// This way when we have methods that consumes &self we can be sure that ETHSignature::recover_signer works
/// And we can be sure that we are compatible with Ethereum clients.
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedEthSignature(ETHSignature);

#[derive(Debug, Error)]
pub enum PackedETHSignatureError {
    #[error("Signature length mismatch")]
    LengthMismatched,
    #[error("Crypto Error: {0:?}")]
    CryptoError(#[from] parity_crypto::publickey::Error),
}

impl PackedEthSignature {
    pub fn serialize_packed(&self) -> [u8; 65] {
        // adds 27 to v
        self.0.clone().into_electrum()
    }

    pub fn deserialize_packed(bytes: &[u8]) -> Result<Self, PackedETHSignatureError> {
        if bytes.len() != 65 {
            return Err(PackedETHSignatureError::LengthMismatched);
        }

        let mut bytes_array = [0u8; 65];
        bytes_array.copy_from_slice(bytes);

        if bytes_array[64] >= 27 {
            bytes_array[64] -= 27;
        }

        Ok(PackedEthSignature(ETHSignature::from(bytes_array)))
    }

    /// Signs message using ethereum private key, results are identical to signature created
    /// using `geth`, `ethecore/lib/types/src/gas_counter.rsrs.js`, etc. No hashing and prefixes required.
    pub fn sign(
        private_key: &H256,
        msg: &[u8],
    ) -> Result<PackedEthSignature, PackedETHSignatureError> {
        let secret_key = (*private_key).into();
        let signed_bytes = Self::message_to_signed_bytes(msg);
        let signature = sign(&secret_key, &signed_bytes)?;
        Ok(PackedEthSignature(signature))
    }

    fn message_to_signed_bytes(msg: &[u8]) -> H256 {
        let prefix = format!("\x19Ethereum Signed Message:\n{}", msg.len());
        let mut bytes = Vec::with_capacity(prefix.len() + msg.len());
        bytes.extend_from_slice(prefix.as_bytes());
        bytes.extend_from_slice(msg);
        bytes.keccak256().into()
    }

    /// Checks signature and returns ethereum address of the signer.
    /// message should be the same message that was passed to `eth.sign`(or similar) method
    /// as argument. No hashing and prefixes required.
    pub fn signature_recover_signer_from_raw_message(
        &self,
        msg: &[u8],
    ) -> Result<Address, PackedETHSignatureError> {
        let signed_bytes = Self::message_to_signed_bytes(msg);
        self.signature_recover_signer_from_hash(signed_bytes)
    }

    /// Checks signature and returns ethereum address of the signer.
    /// The hash should be from the same message that was passed to `eth.sign`(or similar) method
    /// as argument.
    pub fn signature_recover_signer_from_hash(
        &self,
        signed_bytes: H256,
    ) -> Result<Address, PackedETHSignatureError> {
        let public_key = recover(&self.0, &signed_bytes)?;
        Ok(public_to_address(&public_key))
    }

    /// Get Ethereum address from private key.
    pub fn address_from_private_key(
        private_key: &H256,
    ) -> Result<Address, PackedETHSignatureError> {
        Ok(KeyPair::from_secret((*private_key).into())?.address())
    }

    /// Signs typed struct using ethereum private key according to the EIP-712 signature standard.
    /// Result of this function is the equivalent of RPC calling `eth_signTypedData`.
    pub fn sign_typed_data(
        private_key: &H256,
        domain: &Eip712Domain,
        typed_struct: &impl EIP712TypedStructure,
    ) -> Result<PackedEthSignature, PackedETHSignatureError> {
        let secret_key = (*private_key).into();
        let signed_bytes = Self::typed_data_to_signed_bytes(domain, typed_struct);
        let signature = sign(&secret_key, &signed_bytes)?;
        Ok(PackedEthSignature(signature))
    }

    pub fn typed_data_to_signed_message(
        domain: &Eip712Domain,
        typed_struct: &impl EIP712TypedStructure,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice("\x19\x01".as_bytes());
        bytes.extend_from_slice(domain.hash_struct().as_bytes());
        bytes.extend_from_slice(typed_struct.hash_struct().as_bytes());
        bytes
    }

    pub fn typed_data_to_signed_bytes(
        domain: &Eip712Domain,
        typed_struct: &impl EIP712TypedStructure,
    ) -> H256 {
        let bytes = Self::typed_data_to_signed_message(domain, typed_struct);
        bytes.keccak256().into()
    }
}

impl Serialize for PackedEthSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let packed_signature = self.serialize_packed();
        ZeroPrefixHexSerde::serialize(packed_signature, serializer)
    }
}

impl<'de> Deserialize<'de> for PackedEthSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = ZeroPrefixHexSerde::deserialize(deserializer)?;
        Self::deserialize_packed(&bytes).map_err(serde::de::Error::custom)
    }
}
