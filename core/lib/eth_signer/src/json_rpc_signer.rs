use crate::error::{RpcSignerError, SignerError};
use crate::json_rpc_signer::messages::JsonRpcRequest;
use crate::EthereumSigner;
use crate::RawTransaction;

use jsonrpc_core::types::response::Output;
use zksync_types::tx::{PackedEthSignature, TxEthSignature};
use zksync_types::Address;

use serde_json::Value;

pub fn is_signature_from_address(
    signature: &PackedEthSignature,
    msg: &[u8],
    address: Address,
) -> Result<bool, SignerError> {
    let signature_is_correct = signature
        .signature_recover_signer(msg)
        .map_err(|err| SignerError::RecoverAddress(err.to_string()))?
        == address;
    Ok(signature_is_correct)
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
        if is_signature_from_address(&signature, msg, self.address()?)? {
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

        if is_signature_from_address(&signature, &msg.as_bytes(), self.address()?)? {
            self.signer_type = Some(SignerType::NotNeedPrefix);
        }

        if is_signature_from_address(&signature, &msg_with_prefix.as_bytes(), self.address()?)? {
            self.signer_type = Some(SignerType::NotNeedPrefix);
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

    #[derive(Debug, Serialize, Deserialize)]
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
            let params = vec![
                serde_json::to_value(address).expect("serialization fail"),
                serde_json::to_value(password).expect("serialization fail"),
            ];
            Self::create("personal_unlockAccount", params)
        }

        /// The sign method calculates an Ethereum specific signature with:
        /// sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))).
        /// The address to sign with must be unlocked.
        pub fn sign_message(address: Address, message: &[u8]) -> Self {
            let params = vec![
                serde_json::to_value(address).expect("serialization fail"),
                serde_json::to_value(format!("0x{}", encode(message))).expect("serialization fail"),
            ];
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

#[cfg(test)]
mod tests {
    use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
    use futures::future::{AbortHandle, Abortable};
    use jsonrpc_core::{Failure, Id, Output, Success, Version};
    use parity_crypto::publickey::{Generator, KeyPair, Random};
    use serde_json::json;

    use zksync_types::{
        tx::{PackedEthSignature, TxEthSignature},
        Address,
    };

    use super::{is_signature_from_address, messages::JsonRpcRequest};
    use crate::{EthereumSigner, JsonRpcSigner, RawTransaction};

    #[post("/")]
    async fn index(req: web::Json<JsonRpcRequest>, state: web::Data<State>) -> impl Responder {
        let resp = match req.method.as_str() {
            "eth_accounts" => {
                let mut addresses = vec![];
                for pair in &state.key_pairs {
                    addresses.push(pair.address())
                }

                create_success(json!(addresses))
            }
            "personal_unlockAccount" => create_success(json!(true)),
            "eth_sign" => {
                let _address: Address = serde_json::from_value(req.params[0].clone()).unwrap();
                let data: String = serde_json::from_value(req.params[1].clone()).unwrap();
                let data_bytes = hex::decode(&data[2..]).unwrap();
                let signature =
                    PackedEthSignature::sign(state.key_pairs[0].secret(), &data_bytes).unwrap();
                create_success(json!(signature))
            }
            "eth_signTransaction" => {
                let tx_value = json!(req.params[0].clone()).to_string();
                let tx = tx_value.as_bytes();
                let hex_data = hex::encode(tx);
                create_success(json!({ "raw": hex_data }))
            }
            _ => create_fail(req.method.clone()),
        };
        HttpResponse::Ok().json(json!(resp))
    }

    fn create_fail(method: String) -> Output {
        Output::Failure(Failure {
            jsonrpc: Some(Version::V2),
            error: jsonrpc_core::Error {
                code: jsonrpc_core::ErrorCode::ParseError,
                message: method,
                data: None,
            },
            id: Id::Num(1),
        })
    }

    fn create_success(result: serde_json::Value) -> Output {
        Output::Success(Success {
            jsonrpc: Some(Version::V2),
            result,
            id: Id::Num(1),
        })
    }
    #[derive(Clone)]
    struct State {
        key_pairs: Vec<KeyPair>,
    }

    fn run_server(state: State) -> (String, AbortHandle) {
        let mut url = None;
        let mut server = None;
        for i in 9000..9999 {
            let new_url = format!("127.0.0.1:{}", i);
            // Try to bind to some port, hope that 999 variants will be enough
            let tmp_state = state.clone();
            if let Ok(ser) =
                HttpServer::new(move || App::new().data(tmp_state.clone()).service(index))
                    .bind(new_url.clone())
            {
                server = Some(ser);
                url = Some(new_url);
                break;
            }
        }

        let server = server.expect("Could not bind to port from 9000 to 9999");
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let future = Abortable::new(server.run(), abort_registration);
        tokio::spawn(future);
        let address = format!("http://{}/", &url.unwrap());
        (address, abort_handle)
    }

    #[actix_rt::test]
    async fn run_client() {
        let (address, abort_handle) = run_server(State {
            key_pairs: vec![Random.generate()],
        });
        // Get address is ok,  unlock address is ok, recover address from signature is also ok
        let client = JsonRpcSigner::new(address, None, None, None).await.unwrap();
        let msg = b"some_text_message";
        if let TxEthSignature::EthereumSignature(signature) =
            client.sign_message(msg).await.unwrap()
        {
            assert!(is_signature_from_address(&signature, msg, client.address().unwrap()).unwrap())
        } else {
            panic!("Wrong signature type")
        }
        let transaction_signature = client
            .sign_transaction(RawTransaction {
                chain_id: 0,
                nonce: Default::default(),
                to: None,
                value: Default::default(),
                gas_price: Default::default(),
                gas: Default::default(),
                data: vec![],
            })
            .await
            .unwrap();
        assert_ne!(transaction_signature.len(), 0);
        abort_handle.abort();
    }
}
