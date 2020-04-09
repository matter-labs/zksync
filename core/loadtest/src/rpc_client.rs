// Built-in imports
// External uses
use jsonrpc_core::types::response::Output;
// Workspace uses
use models::node::tx::{FranklinTx, PackedEthSignature, TxHash};
use models::node::Address;
use server::api_server::rpc_server::AccountInfoResp;
// Local uses
use self::messages::{AccountStateRequest, EthopInfoRequest, SubmitTxMsg, TxInfoRequest};

/// State of the ZKSync operation.
#[derive(Debug)]
pub struct OperationState {
    pub executed: bool,
    pub verified: bool,
}

/// `RpcClient` is capable of interacting with the ZKSync node via its
/// JSON RPC interface.
#[derive(Debug, Clone)]
pub struct RpcClient {
    rpc_addr: String,
    client: reqwest::Client,
}

impl RpcClient {
    /// Creates a new `RpcClient` object.
    pub fn new(rpc_addr: impl Into<String>) -> Self {
        Self {
            rpc_addr: rpc_addr.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Sends the transaction to the ZKSync server using the JSON RPC.
    pub async fn send_tx(
        &self,
        tx: FranklinTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<TxHash, failure::Error> {
        let msg = SubmitTxMsg::new(tx, eth_signature);

        let ret = self.post(&msg).await?;
        let tx_hash = serde_json::from_value(ret).expect("failed to parse `send_tx` response");
        Ok(tx_hash)
    }

    /// Requests and returns information about a ZKSync account given its address.
    pub async fn account_state_info(
        &self,
        address: Address,
    ) -> Result<AccountInfoResp, failure::Error> {
        let msg = AccountStateRequest::new(address);

        let ret = self.post(&msg).await?;
        let account_state =
            serde_json::from_value(ret).expect("failed to parse account request response");
        Ok(account_state)
    }

    /// Requests and returns a tuple `(executed, verified)` (as `OperationState`) for
    /// an Ethereum operation given its `serial_id`.
    pub async fn ethop_info(&self, serial_id: u64) -> Result<OperationState, failure::Error> {
        let msg = EthopInfoRequest::new(serial_id);

        let ret = self.post(&msg).await?;
        let obj = ret.as_object().unwrap();
        let executed = obj["executed"].as_bool().unwrap();
        let verified = if executed {
            let block = obj["block"].as_object().unwrap();
            block["verified"].as_bool().unwrap()
        } else {
            false
        };

        Ok(OperationState { executed, verified })
    }

    /// Requests and returns a tuple `(executed, verified)` (as `OperationState`) for
    /// a transaction given its hash`.
    pub async fn tx_info(&self, tx_hash: TxHash) -> Result<OperationState, failure::Error> {
        let msg = TxInfoRequest::new(tx_hash.clone());

        let ret = self.post(&msg).await?;
        let obj = ret.as_object().unwrap();
        let executed = obj["executed"].as_bool().unwrap();
        let verified = if executed {
            let block = obj["block"].as_object().unwrap();
            block["verified"].as_bool().unwrap()
        } else {
            false
        };
        Ok(OperationState { executed, verified })
    }

    /// Performs a POST query to the JSON RPC endpoint,
    /// and decodes the response, returning the decoded `serde_json::Value`.
    /// `Ok` is returned only for successful calls, for any kind of error
    /// the `Err` variant is returned (including the failed RPC method
    /// execution response).
    async fn post(
        &self,
        message: impl serde::Serialize,
    ) -> Result<serde_json::Value, failure::Error> {
        let res = self
            .client
            .post(&self.rpc_addr)
            .json(&message)
            .send()
            .await?;
        if res.status() != reqwest::StatusCode::OK {
            failure::bail!(
                "Post query responded with a non-OK response: {}",
                res.status()
            );
        }
        let reply: Output = res.json().await.unwrap();

        let ret = match reply {
            Output::Success(v) => v.result,
            Output::Failure(v) => failure::bail!("RPC error: {}", v.error),
        };

        Ok(ret)
    }
}

/// Structures representing the RPC request messages.
mod messages {
    use models::node::{
        tx::{FranklinTx, PackedEthSignature, TxEthSignature, TxHash},
        Address,
    };
    use serde_derive::Serialize;

    #[derive(Debug, Serialize)]
    pub struct SubmitTxMsg {
        pub id: String,
        pub method: String,
        pub jsonrpc: String,
        pub params: Vec<serde_json::Value>,
    }

    impl SubmitTxMsg {
        pub fn new(tx: FranklinTx, eth_signature: Option<PackedEthSignature>) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(tx).expect("serialization fail"));
            params.push(
                serde_json::to_value(eth_signature.map(TxEthSignature::EthereumSignature))
                    .expect("serialization fail"),
            );
            Self {
                id: "1".to_owned(),
                method: "tx_submit".to_owned(),
                jsonrpc: "2.0".to_owned(),
                params,
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct AccountStateRequest {
        pub id: String,
        pub method: String,
        pub jsonrpc: String,
        pub params: Vec<Address>,
    }

    impl AccountStateRequest {
        pub fn new(address: Address) -> Self {
            Self {
                id: "2".to_owned(),
                method: "account_info".to_owned(),
                jsonrpc: "2.0".to_owned(),
                params: vec![address],
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct EthopInfoRequest {
        pub id: String,
        pub method: String,
        pub jsonrpc: String,
        pub params: Vec<u64>,
    }

    impl EthopInfoRequest {
        pub fn new(serial_id: u64) -> Self {
            Self {
                id: "3".to_owned(),
                method: "ethop_info".to_owned(),
                jsonrpc: "2.0".to_owned(),
                params: vec![serial_id],
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct TxInfoRequest {
        pub id: String,
        pub method: String,
        pub jsonrpc: String,
        pub params: Vec<TxHash>,
    }

    impl TxInfoRequest {
        pub fn new(h: TxHash) -> Self {
            Self {
                id: "4".to_owned(),
                method: "tx_info".to_owned(),
                jsonrpc: "2.0".to_owned(),
                params: vec![h],
            }
        }
    }
}
