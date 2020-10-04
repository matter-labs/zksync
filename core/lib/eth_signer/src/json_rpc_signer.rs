use crate::error::ClientError;
use crate::json_rpc_signer::messages::JsonRpcRequest;
use crate::SignerError;

use jsonrpc_core::types::response::Output;

use models::tx::{RawTransaction, TxEthSignature};
use models::Address;

#[derive(Clone)]
pub struct JsonRpcSigner {
    rpc_addr: String,
    client: reqwest::Client,
    address: Address,
}

impl JsonRpcSigner {
    pub fn new(rpc_addr: impl Into<String>, address: Address) -> Self {
        Self {
            rpc_addr: rpc_addr.into(),
            client: reqwest::Client::new(),
            address: address,
        }
    }

    pub fn address(&self) -> Address {
        self.address
    }

    /// FIXME: make comment
    pub async fn sign_message(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        let msg = JsonRpcRequest::sign_message(self.address, message);

        let ret = self
            .post(&msg)
            .await
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        let signature = serde_json::from_value(ret)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;

        Ok(signature)
    }

    /// FIXME: make comment
    pub async fn sign_transaction(
        &self,
        raw_tx: RawTransaction,
    ) -> Result<TxEthSignature, SignerError> {
        let msg = JsonRpcRequest::sign_transaction(raw_tx);

        let ret = self
            .post(&msg)
            .await
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        let signature = serde_json::from_value(ret)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;

        Ok(signature)
    }

    /// Performs a POST query to the JSON RPC endpoint,
    /// and decodes the response, returning the decoded `serde_json::Value`.
    /// `Ok` is returned only for successful calls, for any kind of error
    /// the `Err` variant is returned (including the failed RPC method
    /// execution response).
    async fn post(&self, message: impl serde::Serialize) -> Result<serde_json::Value, ClientError> {
        let reply: Output = self.post_raw(message).await?;

        let ret = match reply {
            Output::Success(success) => success.result,
            Output::Failure(failure) => return Err(ClientError::RpcError(failure)),
        };

        Ok(ret)
    }

    /// Performs a POST query to the JSON RPC endpoint,
    /// and decodes the response, returning the decoded `serde_json::Value`.
    /// `Ok` is returned only for successful calls, for any kind of error
    /// the `Err` variant is returned (including the failed RPC method
    /// execution response).
    async fn post_raw(&self, message: impl serde::Serialize) -> Result<Output, ClientError> {
        let res = self
            .client
            .post(&self.rpc_addr)
            .json(&message)
            .send()
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;
        if res.status() != reqwest::StatusCode::OK {
            let error = format!(
                "Post query responded with a non-OK response: {}",
                res.status()
            );
            return Err(ClientError::NetworkError(error));
        }
        let reply: Output = res
            .json()
            .await
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;

        Ok(reply)
    }
}

mod messages {

    use models::tx::RawTransaction;
    use models::Address;

    #[derive(Debug, Serialize)]
    pub struct JsonRpcRequest {
        pub id: String,
        pub method: String,
        pub jsonrpc: String,
        pub params: Vec<serde_json::Value>,
    }

    impl JsonRpcRequest {
        fn create(method: impl ToString, params: Vec<serde_json::Value>) -> Self {
            Self {
                id: "1".to_owned(),
                jsonrpc: "2.0".to_owned(),
                method: method.to_string(),
                params,
            }
        }

        /// The sign method calculates an Ethereum specific signature with:
        /// sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))).
        pub fn sign_message(address: Address, message: &[u8]) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(address).expect("serialization fail"));
            params.push(serde_json::to_value(message).expect("serialization fail"));
            Self::create("eth_sign", params)
        }

        pub fn sign_transaction(tx_data: RawTransaction) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(tx_data).expect("serialization fail"));
            //params.push(serde_json::to_value(message).expect("serialization fail"));
            Self::create("eth_signTransaction", params)
        }
    }
}
