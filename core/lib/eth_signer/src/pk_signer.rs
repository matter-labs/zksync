use crate::raw_ethereum_tx::{RawTransaction, Transaction};
use crate::{EthereumSigner, SignerError};

use secp256k1::SecretKey;

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
        let pack = PackedEthSignature::sign(&self.private_key, message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(TxEthSignature::EthereumSignature(pack))
    }

    /// Signs and returns the RLP-encoded transaction.
    async fn sign_transaction(&self, raw_tx: RawTransaction) -> Result<Vec<u8>, SignerError> {
        let key = SecretKey::from_slice(self.private_key.as_bytes()).unwrap();

        let gas_price = match raw_tx.max_fee_per_gas {
            Some(val) => val,
            None => raw_tx.gas_price,
        };
        let tx = Transaction {
            to: raw_tx.to,
            nonce: raw_tx.nonce,
            gas: raw_tx.gas,
            gas_price,
            value: raw_tx.value,
            data: raw_tx.data,
            transaction_type: raw_tx.transaction_type,
            access_list: raw_tx.access_list.unwrap_or_default(),
            max_priority_fee_per_gas: raw_tx.max_priority_fee_per_gas.unwrap_or_default(),
        };

        let signed = tx.sign(&key, raw_tx.chain_id);
        Ok(signed.raw_transaction.0)
    }
}

#[cfg(test)]
mod test {
    use super::PrivateKeySigner;
    use super::RawTransaction;
    use crate::EthereumSigner;
    use web3::types::U64;
    use zksync_types::{H160, H256, U256};

    #[tokio::test]
    async fn test_generating_signed_raw_transaction() {
        let private_key = H256::from([5; 32]);
        let signer = PrivateKeySigner::new(private_key);
        let raw_transaction = RawTransaction {
            nonce: U256::from(1u32),
            to: Some(H160::default()),
            gas: Default::default(),
            gas_price: U256::from(2u32),
            max_fee_per_gas: Some(U256::from(2u32)),
            max_priority_fee_per_gas: Some(U256::from(1u32)),
            value: Default::default(),
            data: vec![1, 2, 3],
            chain_id: 270,
            transaction_type: Some(U64::from(1u32)),
            access_list: None,
        };
        let raw_tx = signer
            .sign_transaction(raw_transaction.clone())
            .await
            .unwrap();
        assert_ne!(raw_tx.len(), 1);
        // precalculated signature with right algorithm implementation
        let precalculated_raw_tx: Vec<u8> = vec![
            1, 248, 100, 130, 1, 14, 1, 2, 128, 148, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 128, 131, 1, 2, 3, 192, 1, 160, 98, 201, 238, 158, 215, 98, 23, 231,
            221, 161, 170, 16, 54, 85, 187, 107, 12, 228, 218, 139, 103, 164, 17, 196, 178, 185,
            252, 243, 186, 175, 93, 230, 160, 93, 204, 205, 5, 46, 187, 231, 211, 102, 133, 200,
            254, 119, 94, 206, 81, 8, 143, 204, 14, 138, 43, 183, 214, 209, 166, 16, 116, 176, 44,
            52, 133,
        ];
        assert_eq!(raw_tx, precalculated_raw_tx);
    }
}
