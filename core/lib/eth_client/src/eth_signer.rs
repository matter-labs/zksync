use crate::error::SignerError;
use models::tx::PackedEthSignature;
use models::tx::TxEthSignature;
use models::H256;

#[derive(Clone)]
pub enum EthereumSigner {
    PrivateKey(PrivateKeySigner),
    // JsonRpc(JsonRpcSigner),
}

impl EthereumSigner {
    pub fn from_key(private_key: H256) -> Self {
        Self::PrivateKey(PrivateKeySigner { private_key })
    }

    pub async fn sign(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        match self {
            EthereumSigner::PrivateKey(pk_signer) => pk_signer.sign(message),
            // EthereumSigner::JsonRpc(JsonRpcSigner) => JsonRpcSigner.sign(message)
        }
    }

    // TODO
    // pub async fn send_tx(&self, tx_options: web3::types::TransactionRequest) -> Result<H256, ClientError> {
    // } // Sends the transaction to the Ethereum network.
    // pub async fn get_address(&self) -> Address { ... } // Returns the Ethereum wallet address
    // pub async fn from_rpc(rpc_signer: JsonRpcSigner) -> Self { ... }
}

#[derive(Clone)]
pub struct PrivateKeySigner {
    private_key: H256, // rpc_addr: Option<...> // If set, we can send transactions to the node via provided RPC address.
}

impl PrivateKeySigner {
    pub fn sign(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        let pack = PackedEthSignature::sign(&self.private_key, message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(TxEthSignature::EthereumSignature(pack))
    }
}

// pub struct JsonRpcSigner {
//     // .. Fields required for JsonRpcSigner to operate
// }
