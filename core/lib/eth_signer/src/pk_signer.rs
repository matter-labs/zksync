use crate::raw_ethereum_tx::RawTransaction;
use crate::{EthereumSigner, SignerError};

use parity_crypto::publickey::sign;

use zksync_types::tx::{PackedEthSignature, TxEthSignature};
use zksync_types::{Address, H256};

#[derive(Clone)]
pub struct PrivateKeySigner {
    private_key: H256,
}

impl std::fmt::Debug for PrivateKeySigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrivateKeySigner")
    }
}

impl PrivateKeySigner {
    pub fn new(private_key: H256) -> Self {
        Self { private_key }
    }
}

#[async_trait::async_trait]
impl EthereumSigner for PrivateKeySigner {
    /// Get Ethereum address that matches the private key.
    async fn get_address(&self) -> Result<Address, SignerError> {
        PackedEthSignature::address_from_private_key(&self.private_key)
            .map_err(|_| SignerError::DefineAddress)
    }

    /// The sign method calculates an Ethereum specific signature with:
    /// sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))).
    async fn sign_message(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        let pack = PackedEthSignature::sign(&self.private_key, &message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(TxEthSignature::EthereumSignature(pack))
    }

    /// Signs and returns the RLP-encoded transaction.
    async fn sign_transaction(&self, raw_tx: RawTransaction) -> Result<Vec<u8>, SignerError> {
        let sig = sign(&self.private_key.into(), &raw_tx.hash().into())
            .map_err(|_| SignerError::NoSigningKey)?;
        Ok(raw_tx.rlp_encode_tx(sig))
    }
}

#[cfg(test)]
mod test {
    use super::PrivateKeySigner;
    use super::RawTransaction;
    use crate::EthereumSigner;
    use zksync_types::{H160, H256, U256};

    #[tokio::test]
    async fn test_generating_signature() {
        let private_key = H256::from([5; 32]);
        let signer = PrivateKeySigner::new(private_key);
        let raw_transaction = RawTransaction {
            chain_id: 1,
            nonce: U256::from(1),
            to: Some(H160::zero()),
            value: U256::from(10),
            gas_price: U256::from(1),
            gas: U256::from(2),
            data: vec![1, 2, 3],
        };
        let signature = signer
            .sign_transaction(raw_transaction.clone())
            .await
            .unwrap();
        assert_ne!(signature.len(), 1);
        // precalculated signature with right algorithm implementation
        let precalculated_signature: Vec<u8> = vec![
            248, 96, 1, 1, 2, 148, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10,
            131, 1, 2, 3, 37, 160, 152, 202, 15, 174, 50, 167, 190, 239, 206, 183, 109, 215, 135,
            60, 43, 71, 11, 74, 252, 97, 83, 86, 66, 249, 237, 111, 118, 121, 105, 214, 130, 249,
            160, 106, 110, 143, 138, 113, 12, 177, 239, 121, 188, 247, 21, 236, 236, 163, 254, 28,
            48, 250, 5, 20, 234, 54, 58, 162, 103, 252, 20, 243, 121, 7, 19,
        ];
        assert_eq!(signature, precalculated_signature);
    }
}
