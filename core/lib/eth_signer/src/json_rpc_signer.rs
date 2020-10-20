use crate::error::{RpcSignerError, SignerError};
use crate::json_rpc_signer::messages::JsonRpcRequest;
use crate::EthereumSigner;
use crate::RawTransaction;

use jsonrpc_core::types::response::Output;
use zksync_types::tx::{PackedEthSignature, TxEthSignature};
use zksync_types::Address;

use parity_crypto::publickey::{public_to_address, recover, Signature};
use parity_crypto::Keccak256;
use serde_json::Value;

pub fn recover_eth_signer(signature: &Signature, msg: &[u8]) -> Result<Address, SignerError> {
    let signed_bytes = msg.keccak256().into();
    let public_key = recover(&signature, &signed_bytes)
        .map_err(|err| SignerError::RecoverAddress(err.to_string()))?;
    Ok(public_to_address(&public_key))
}

#[derive(Debug, Clone)]
pub enum AddressOrIndex {
    Address(Address),
    Index(usize),
}

/// Describes whether to add a prefix `\x19Ethereum Signed Message:\n`
/// when requesting a message signature.
#[derive(Debug, Clone)]
pub enum SignerType {
    NotNeedPrefix,
    NeedPrefix,
}

#[derive(Debug, Clone)]
pub struct JsonRpcSigner {
    rpc_addr: String,
    client: reqwest::Client,
    address: Option<Address>,
    signer_type: Option<SignerType>,
}

#[async_trait::async_trait]
impl EthereumSigner for JsonRpcSigner {
    /// The sign method calculates an Ethereum specific signature with:
    /// checks if the server adds a prefix if not then adds
    /// return sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))).
    async fn sign_message(&self, msg: &[u8]) -> Result<TxEthSignature, SignerError> {
        let signature: PackedEthSignature = {
            let msg = match &self.signer_type {
                Some(SignerType::NotNeedPrefix) => msg.to_vec(),
                Some(SignerType::NeedPrefix) => {
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

            let message = JsonRpcRequest::sign_message(self.address()?, &msg);
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
            == self.address()?
        {
            Ok(TxEthSignature::EthereumSignature(signature))
        } else {
            Err(SignerError::SigningFailed(
                "Invalid signature from JsonRpcSigner".to_string(),
            ))
        }
    }

    /// Signs and returns the RLP-encoded transaction.
    async fn sign_transaction(&self, raw_tx: RawTransaction) -> Result<Vec<u8>, SignerError> {
        let msg = JsonRpcRequest::sign_transaction(self.address()?, raw_tx);

        let ret = self
            .post(&msg)
            .await
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;

        // get Json object and parse it to get raw Transaction
        let json: Value = serde_json::from_value(ret)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;

        let raw_tx: Option<&str> = json
            .get("raw")
            .and_then(|value| value.as_str())
            .map(|value| &value["0x".len()..]);

        if let Some(raw_tx) = raw_tx {
            hex::decode(raw_tx).map_err(|err| SignerError::DecodeRawTxFailed(err.to_string()))
        } else {
            Err(SignerError::DefineAddress)
        }
    }

    async fn get_address(&self) -> Result<Address, SignerError> {
        self.address()
    }
}

impl JsonRpcSigner {
    pub async fn new(
        rpc_addr: impl Into<String>,
        address_or_index: Option<AddressOrIndex>,
        signer_type: Option<SignerType>,
        password_to_unlock: Option<String>,
    ) -> Result<Self, SignerError> {
        let mut signer = Self {
            rpc_addr: rpc_addr.into(),
            client: reqwest::Client::new(),
            address: None,
            signer_type,
        };

        // If the user has not specified either the index or the address,
        // then we will assume that by default the address will be the first one that the server will send
        let address_or_index = match address_or_index {
            Some(address_or_index) => address_or_index,
            None => AddressOrIndex::Index(0),
        };

        // EthereumSigner can support many different addresses,
        // we define only the one we need by the index
        // of receiving from the server or by the address itself.
        signer.detect_address(address_or_index).await?;

        if let Some(password) = password_to_unlock {
            signer.unlock(&password).await?;
        }

        // If it is not known whether it is necessary
        // to add a prefix to messages, then we define this.
        if signer.signer_type.is_none() {
            signer.detect_signer_type().await?;
        };

        Ok(signer)
    }

    /// Get Ethereum address.
    pub fn address(&self) -> Result<Address, SignerError> {
        self.address.ok_or(SignerError::DefineAddress)
    }

    /// Specifies the Ethreum address which sets the address for which all other requests will be processed.
    /// If the address has already been set, then it will all the same change to a new one.
    pub async fn detect_address(
        &mut self,
        address_or_index: AddressOrIndex,
    ) -> Result<Address, SignerError> {
        self.address = match address_or_index {
            AddressOrIndex::Address(address) => Some(address),
            AddressOrIndex::Index(index) => {
                let message = JsonRpcRequest::accounts();
                let ret = self
                    .post(&message)
                    .await
                    .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
                let accounts: Vec<Address> = serde_json::from_value(ret)
                    .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
                accounts.get(index).copied()
            }
        };

        self.address.ok_or(SignerError::DefineAddress)
    }

    /// Server can either add the prefix `\x19Ethereum Signed Message:\n` to the message and not add.
    /// Checks if a prefix should be added to the message.
    pub async fn detect_signer_type(&mut self) -> Result<(), SignerError> {
        // If the `sig_type` is set, then we do not need to detect it from the server.
        if self.signer_type.is_some() {
            return Ok(());
        }

        let msg = "JsonRpcSigner type was not specified. Sign this message to detect the signer type. It only has to be done once per session";
        let msg_with_prefix = format!("\x19Ethereum Signed Message:\n{}{}", msg.len(), msg);

        let signature: PackedEthSignature = {
            let message = JsonRpcRequest::sign_message(self.address()?, msg.as_bytes());

            let ret = self
                .post(&message)
                .await
                .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
            serde_json::from_value(ret)
                .map_err(|err| SignerError::SigningFailed(err.to_string()))?
        };

        if recover_eth_signer(&signature.serialize_packed().into(), &msg.as_bytes())?
            == self.address()?
        {
            self.signer_type = Some(SignerType::NotNeedPrefix);
        }

        if recover_eth_signer(
            &signature.serialize_packed().into(),
            &msg_with_prefix.as_bytes(),
        )? == self.address()?
        {
            self.signer_type = Some(SignerType::NeedPrefix);
        }

        match self.signer_type.is_some() {
            true => Ok(()),
            false => Err(SignerError::SigningFailed(
                "Failed to get the correct signature".to_string(),
            )),
        }
    }

    /// Unlocks the current account, after that the server can sign messages and transactions.
    pub async fn unlock(&self, password: &str) -> Result<(), SignerError> {
        let message = JsonRpcRequest::unlock_account(self.address()?, password);
        let ret = self
            .post(&message)
            .await
            .map_err(|err| SignerError::UnlockingFailed(err.to_string()))?;

        let res: bool = serde_json::from_value(ret)
            .map_err(|err| SignerError::UnlockingFailed(err.to_string()))?;

        if res {
            Ok(())
        } else {
            Err(SignerError::UnlockingFailed(
                "Server response: false".to_string(),
            ))
        }
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
    use crate::RawTransaction;
    use hex::encode;
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

        /// Returns a list of addresses owned by client.
        pub fn accounts() -> Self {
            let params = Vec::new();
            Self::create("eth_accounts", params)
        }

        // Unlocks the address, after that the server can sign messages and transactions.
        pub fn unlock_account(address: Address, password: &str) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(address).expect("serialization fail"));
            params.push(serde_json::to_value(password).expect("serialization fail"));
            Self::create("personal_unlockAccount", params)
        }

        /// The sign method calculates an Ethereum specific signature with:
        /// sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))).
        /// The address to sign with must be unlocked.
        pub fn sign_message(address: Address, message: &[u8]) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(address).expect("serialization fail"));
            params.push(
                serde_json::to_value(format!("0x{}", encode(message))).expect("serialization fail"),
            );
            Self::create("eth_sign", params)
        }

        /// Signs a transaction that can be submitted to the network.
        /// The address to sign with must be unlocked.
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
                    "data": serde_json::to_value(format!("0x{}", encode(tx_data.data))).expect("serialization fail"),
                    "nonce": serde_json::to_value(tx_data.nonce).expect("serialization fail"),
                })
            } else {
                serde_json::json!({
                    "from": serde_json::to_value(from).expect("serialization fail"),
                    "gas": serde_json::to_value(tx_data.gas).expect("serialization fail"),
                    "gasPrice": serde_json::to_value(tx_data.gas_price).expect("serialization fail"),
                    "value": serde_json::to_value(tx_data.value).expect("serialization fail"),
                    "data": serde_json::to_value(format!("0x{}", encode(tx_data.data))).expect("serialization fail"),
                    "nonce": serde_json::to_value(tx_data.nonce).expect("serialization fail"),
                })
            };
            params.push(tx);
            Self::create("eth_signTransaction", params)
        }
    }
}
