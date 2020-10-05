use crate::SignerError;
use parity_crypto::{publickey::sign, Keccak256};
use rlp::RlpStream;
use zksync_types::tx::{PackedEthSignature, RawTransaction, TxEthSignature};
use zksync_types::{Address, H256};

#[derive(Clone)]
pub struct PrivateKeySigner {
    private_key: H256,
    address: Address,
}

impl PrivateKeySigner {
    pub fn new(private_key: H256) -> Self {
        // FIXME: remove unwrap add Result anf error code
        let address = PackedEthSignature::address_from_private_key(&private_key).unwrap();
        Self {
            private_key,
            address,
        }
    }

    pub fn address(&self) -> Address {
        self.address
    }

    /// FIXME:
    pub fn sign_message(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        let pack = PackedEthSignature::sign(&self.private_key, &message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(TxEthSignature::EthereumSignature(pack))
    }

    pub fn sign_transaction(&self, raw_tx: RawTransaction) -> Result<Vec<u8>, SignerError> {
        let sig = sign(&self.private_key.into(), &raw_tx.hash().into())
            .map_err(|_| SignerError::NoSigningKey)?;
        Ok(raw_tx.rlp_encode_tx(sig))
    }
}
