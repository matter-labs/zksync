use crate::SignerError;

use parity_crypto::publickey::sign;

use zksync_types::tx::{PackedEthSignature, RawTransaction, TxEthSignature};
use zksync_types::{Address, H256};

#[derive(Clone)]
pub struct PrivateKeySigner {
    private_key: H256,
}

impl PrivateKeySigner {
    pub fn new(private_key: H256) -> Self {
        Self { private_key }
    }

    /// Get Ethereum address that matches the private key.
    pub fn address(&self) -> Result<Address, SignerError> {
        PackedEthSignature::address_from_private_key(&self.private_key)
            .map_err(|_| SignerError::DefineAddress)
    }

    /// The sign method calculates an Ethereum specific signature with:
    /// sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))).
    pub fn sign_message(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        let pack = PackedEthSignature::sign(&self.private_key, &message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(TxEthSignature::EthereumSignature(pack))
    }

    /// Signs and returns the RLP-encoded transaction.
    pub fn sign_transaction(&self, raw_tx: RawTransaction) -> Result<Vec<u8>, SignerError> {
        let sig = sign(&self.private_key.into(), &raw_tx.hash().into())
            .map_err(|_| SignerError::NoSigningKey)?;
        Ok(raw_tx.rlp_encode_tx(sig))
    }
}
