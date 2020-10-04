#[macro_use]
extern crate serde_derive;

use error::SignerError;
use models::tx::{RawTransaction, TxEthSignature};
use models::{Address, H256};

use json_rpc_signer::JsonRpcSigner;
use pk_signer::PrivateKeySigner;

pub mod error;
pub mod json_rpc_signer;
pub mod pk_signer;

#[derive(Clone)]
pub enum EthereumSigner {
    PrivateKey(PrivateKeySigner),
    JsonRpc(JsonRpcSigner),
}

impl EthereumSigner {
    pub fn from_key(private_key: H256) -> Self {
        let signer = PrivateKeySigner::new(private_key);
        Self::PrivateKey(signer)
    }

    pub fn from_rpc(rpc_signer: JsonRpcSigner) -> Self {
        Self::JsonRpc(rpc_signer)
    }

    pub async fn sign_message(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        match self {
            EthereumSigner::PrivateKey(pk_signer) => pk_signer.sign_message(message),
            EthereumSigner::JsonRpc(json_rpc_signer) => json_rpc_signer.sign_message(message).await,
        }
    }

    pub async fn sign_transaction(
        &self,
        raw_tx: RawTransaction,
    ) -> Result<TxEthSignature, SignerError> {
        match self {
            EthereumSigner::PrivateKey(pk_signer) => pk_signer.sign_transaction(raw_tx),
            EthereumSigner::JsonRpc(json_rpc_signer) => {
                json_rpc_signer.sign_transaction(raw_tx).await
            }
        }
    }

    pub fn get_address(&self) -> Address {
        match self {
            EthereumSigner::PrivateKey(pk_signer) => pk_signer.address(),
            EthereumSigner::JsonRpc(json_rpc_signer) => json_rpc_signer.address(),
        }
    }
}
