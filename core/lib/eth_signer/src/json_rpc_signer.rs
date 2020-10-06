use crate::error::{RpcSignerError, SignerError};
use crate::json_rpc_signer::messages::JsonRpcRequest;

use jsonrpc_core::types::response::Output;
use zksync_types::tx::{PackedEthSignature, RawTransaction, TxEthSignature};
use zksync_types::Address;

#[derive(Clone)]
pub enum SignerType {
    NotPrefixed,
    Prefixed,
}

#[derive(Clone)]
pub struct JsonRpcSigner {
    rpc_addr: String,
    client: reqwest::Client,
    address: Address,
    signer_type: Option<SignerType>,
}

impl JsonRpcSigner {
    pub async fn new(
        rpc_addr: impl Into<String>,
        address: Address,
        signer_type: Option<SignerType>,
    ) -> Result<Self, SignerError> {
        let mut signer = Self {
            rpc_addr: rpc_addr.into(),
            client: reqwest::Client::new(),
            address,
            signer_type,
        };

        // If it is not known whether it is necessary
        // to add a prefix to messages, then we define this.
        if signer.signer_type.is_none() {
            signer.detect_signer_type().await?;
        };

        Ok(signer)
    }

    /// Get Ethereum address.
    pub fn address(&self) -> Address {
        self.address
    }

    /// Server can either add the prefix `\x19Ethereum Signed Message:\n` to the message and not add.
    /// Checks if a prefix should be added to the message
    pub async fn detect_signer_type(&mut self) -> Result<(), SignerError> {
        // If the `sig_type` is set, then we do not need to detect it from the server
        if self.signer_type.is_some() {
            return Ok(());
        }
        let msg = "JsonRpcSigner type was not specified. Sign this message to detect the signer type. It only has to be done once per session".as_bytes();

        let signature_msg_no_prefix: PackedEthSignature = {
            let message = JsonRpcRequest::sign_message(self.address, msg);

            let ret = self
                .post(&message)
                .await
                .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
            serde_json::from_value(ret)
                .map_err(|err| SignerError::SigningFailed(err.to_string()))?
        };

        let signature_msg_with_prefix: PackedEthSignature = {
            let message_with_prefix = {
                let prefix = format!("\x19Ethereum Signed Message:\n{}", msg.len());
                let mut bytes = Vec::with_capacity(prefix.len() + msg.len());
                bytes.extend_from_slice(prefix.as_bytes());
                bytes.extend_from_slice(msg);

                JsonRpcRequest::sign_message(self.address, &bytes)
            };

            let ret = self
                .post(&message_with_prefix)
                .await
                .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
            serde_json::from_value(ret)
                .map_err(|err| SignerError::SigningFailed(err.to_string()))?
        };

        if signature_msg_no_prefix
            .signature_recover_signer(msg)
            .map_err(|_| SignerError::DefineAddress)?
            == self.address
        {
            self.signer_type = Some(SignerType::NotPrefixed);
        }

        if signature_msg_with_prefix
            .signature_recover_signer(msg)
            .map_err(|_| SignerError::DefineAddress)?
            == self.address
        {
            self.signer_type = Some(SignerType::Prefixed);
        }

        match self.signer_type.is_some() {
            true => Ok(()),
            false => Err(SignerError::SigningFailed(
                "Failed to get the correct signature".to_string(),
            )),
        }
    }

    /// The sign method calculates an Ethereum specific signature with:
    /// checks if the server adds a prefix if not then adds
    /// return sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))).
    pub async fn sign_message(&self, msg: &[u8]) -> Result<TxEthSignature, SignerError> {
        let signature: PackedEthSignature = {
            let msg = match &self.signer_type {
                Some(SignerType::NotPrefixed) => msg.to_vec(),
                Some(SignerType::Prefixed) => {
                    let prefix = format!("\x19Ethereum Signed Message:\n{}", msg.len());
                    let mut bytes = Vec::with_capacity(prefix.len() + msg.len());
                    bytes.extend_from_slice(prefix.as_bytes());
                    bytes.extend_from_slice(msg);

                    bytes
                }
                None => {
                    return Err(SignerError::MissingEthSigner);
                }
            };

            let message = JsonRpcRequest::sign_message(self.address, &msg);
            let ret = self
                .post(&message)
                .await
                .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
            serde_json::from_value(ret)
                .map_err(|err| SignerError::SigningFailed(err.to_string()))?
        };

        // Checks the correctness of the message signature without a prefix
        if signature
            .signature_recover_signer(msg)
            .map_err(|_| SignerError::DefineAddress)?
            == self.address
        {
            Ok(TxEthSignature::EthereumSignature(signature))
        } else {
            Err(SignerError::SigningFailed(
                "Invalid signature from JsonRpcSigner".to_string(),
            ))
        }
    }

    /// Signs and returns the RLP-encoded transaction.
    pub async fn sign_transaction(&self, raw_tx: RawTransaction) -> Result<Vec<u8>, SignerError> {
        let msg = JsonRpcRequest::sign_transaction(self.address, raw_tx);

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
    async fn post(
        &self,
        message: impl serde::Serialize,
    ) -> Result<serde_json::Value, RpcSignerError> {
        let reply: Output = self.post_raw(message).await?;

        let ret = match reply {
            Output::Success(success) => success.result,
            Output::Failure(failure) => return Err(RpcSignerError::RpcError(failure)),
        };

        Ok(ret)
    }

    /// Performs a POST query to the JSON RPC endpoint,
    /// and decodes the response, returning the decoded `serde_json::Value`.
    /// `Ok` is returned only for successful calls, for any kind of error
    /// the `Err` variant is returned (including the failed RPC method
    /// execution response).
    async fn post_raw(&self, message: impl serde::Serialize) -> Result<Output, RpcSignerError> {
        let res = self
            .client
            .post(&self.rpc_addr)
            .json(&message)
            .send()
            .await
            .map_err(|err| RpcSignerError::NetworkError(err.to_string()))?;
        if res.status() != reqwest::StatusCode::OK {
            let error = format!(
                "Post query responded with a non-OK response: {}",
                res.status()
            );
            return Err(RpcSignerError::NetworkError(error));
        }
        let reply: Output = res
            .json()
            .await
            .map_err(|err| RpcSignerError::MalformedResponse(err.to_string()))?;

        Ok(reply)
    }
}

mod messages {
    use zksync_types::tx::RawTransaction;
    use zksync_types::Address;

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

        pub fn sign_transaction(from: Address, tx_data: RawTransaction) -> Self {
            let mut params = Vec::new();

            // Parameter `To` is optional, so we add it only if it is not None
            let tx = if let Some(to) = tx_data.to {
                serde_json::json!({
                    "from": serde_json::to_value(from).expect("serialization fail"),
                    "to": serde_json::to_value(to).expect("serialization fail"),
                    "gas": serde_json::to_value(tx_data.gas).expect("serialization fail"),
                    "gasPrice": serde_json::to_value(tx_data.gas_price).expect("serialization fail"),
                    "value": serde_json::to_value(tx_data.value).expect("serialization fail"),
                    "data": serde_json::to_value(tx_data.data).expect("serialization fail"),
                    "nonce": serde_json::to_value(tx_data.nonce).expect("serialization fail"),
                })
            } else {
                serde_json::json!({
                    "from": serde_json::to_value(from).expect("serialization fail"),
                    "gas": serde_json::to_value(tx_data.gas).expect("serialization fail"),
                    "gasPrice": serde_json::to_value(tx_data.gas_price).expect("serialization fail"),
                    "value": serde_json::to_value(tx_data.value).expect("serialization fail"),
                    "data": serde_json::to_value(tx_data.data).expect("serialization fail"),
                    "nonce": serde_json::to_value(tx_data.nonce).expect("serialization fail"),
                })
            };
            params.push(tx);
            Self::create("eth_signTransaction", params)
        }
    }
}
