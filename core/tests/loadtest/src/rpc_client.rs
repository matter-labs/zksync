// Built-in imports
use std::str::FromStr;
// External uses
use jsonrpc_core::types::response::Output;
use num::BigUint;
// Workspace uses
use models::node::{
    tx::{FranklinTx, PackedEthSignature, TxHash},
    Address, TokenLike, TxFeeTypes,
};
use zksync::{
    error::ClientError,
    types::AccountInfo,
    types::{EthOpInfo, Fee, TransactionInfo},
    Provider,
};
// Local uses
use self::messages::JsonRpcRequest;

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
    inner: Provider,
}

impl RpcClient {
    /// Creates a new `RpcClient` object.
    pub fn from_addr(rpc_addr: impl Into<String>) -> Self {
        let rpc_addr = rpc_addr.into();

        Self {
            rpc_addr: rpc_addr.clone(),
            client: reqwest::Client::new(),
            inner: Provider::from_addr(rpc_addr),
        }
    }

    pub async fn get_tx_fee(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token_symbol: impl Into<TokenLike>,
    ) -> Result<Fee, ClientError> {
        self.inner.get_tx_fee(tx_type, address, token_symbol).await
    }

    /// Sends the transaction to the ZKSync server using the JSON RPC.
    pub async fn send_tx(
        &self,
        tx: FranklinTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<TxHash, ClientError> {
        self.inner.send_tx(tx, eth_signature).await
    }

    /// Sends the transaction to the ZKSync server and returns raw response.
    #[deprecated = "There is no way to fetch raw response from the Provider"]
    pub async fn send_tx_raw(
        &self,
        tx: FranklinTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<Output, failure::Error> {
        let msg = JsonRpcRequest::submit_tx(tx, eth_signature);

        self.post_raw(&msg).await
    }

    /// Requests and returns information about a ZKSync account given its address.
    pub async fn account_info(&self, address: Address) -> Result<AccountInfo, ClientError> {
        self.inner.account_info(address).await
    }

    /// Requests and returns a tuple `(executed, verified)` (as `OperationState`) for
    /// an Ethereum operation given its `serial_id`.
    pub async fn ethop_info(&self, serial_id: u64) -> Result<EthOpInfo, ClientError> {
        self.inner.ethop_info(serial_id as u32).await
    }

    /// Requests and returns a tuple `(executed, verified)` (as `OperationState`) for
    /// a transaction given its hash`.
    pub async fn tx_info(&self, tx_hash: TxHash) -> Result<TransactionInfo, ClientError> {
        self.inner.tx_info(tx_hash).await
    }

    /// Performs a POST query to the JSON RPC endpoint,
    /// and decodes the response, returning the decoded `serde_json::Value`.
    /// `Ok` is returned only for successful calls, for any kind of error
    /// the `Err` variant is returned (including the failed RPC method
    /// execution response).
    async fn post_raw(&self, message: impl serde::Serialize) -> Result<Output, failure::Error> {
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

        Ok(reply)
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

        pub fn submit_tx(tx: FranklinTx, eth_signature: Option<PackedEthSignature>) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(tx).expect("serialization fail"));
            params.push(
                serde_json::to_value(eth_signature.map(TxEthSignature::EthereumSignature))
                    .expect("serialization fail"),
            );
            Self::create("tx_submit", params)
        }
    }
}
