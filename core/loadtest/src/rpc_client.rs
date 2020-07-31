// Built-in imports
use std::str::FromStr;
// External uses
use jsonrpc_core::types::response::Output;
use num::BigUint;
// Workspace uses
use models::node::{
    tx::{FranklinTx, PackedEthSignature, TxHash},
    Address,
};
use server::api_server::rpc_server::AccountInfoResp;
// Local uses
pub use self::messages::JsonRpcRequest;

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

async fn post(
    delay: u64,
    rpc_addr: String,
    client: reqwest::Client,
    message: JsonRpcRequest,
) -> Result<(), failure::Error> {
    let client = RpcClient { rpc_addr, client };

    // A short delay (<= 10 ms) to not overwhelm server with requests.
    tokio::time::delay_for(std::time::Duration::from_millis(delay)).await;

    client.post(&message).await?;

    Ok(())
}

async fn post_batch(
    delay: u64,
    rpc_addr: String,
    client: reqwest::Client,
    messages: Vec<JsonRpcRequest>,
) -> Result<(), failure::Error> {
    let client = RpcClient { rpc_addr, client };

    // A short delay (<= 10 ms) to not overwhelm server with requests.
    tokio::time::delay_for(std::time::Duration::from_millis(delay)).await;

    for message in messages {
        client.post(&message).await?;
    }

    Ok(())
}

impl RpcClient {
    /// Creates a new `RpcClient` object.
    pub fn new(rpc_addr: impl Into<String>) -> Self {
        Self {
            rpc_addr: rpc_addr.into(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_tx_fee(
        &self,
        tx_type: &str,
        address: Address,
        token_symbol: &str,
    ) -> Result<BigUint, failure::Error> {
        let msg = JsonRpcRequest::get_tx_fee(tx_type, address, token_symbol);

        let ret = self.post(&msg).await?;
        let fee_value = ret["totalFee"]
            .as_str()
            .expect("Incorrect `totalFee` entry of response");
        let fee = BigUint::from_str(&fee_value).expect("failed to parse `get_tx_fee` response");

        Ok(fee)
    }

    // /// Sends the transaction to the ZKSync server using the JSON RPC.
    // pub async fn send_tx(
    //     &self,
    //     tx: FranklinTx,
    //     eth_signature: Option<PackedEthSignature>,
    // ) -> Result<TxHash, failure::Error> {
    //     // let tx_hash = tx.clone().hash();

    //     let msg = JsonRpcRequest::submit_tx(tx, eth_signature);

    //     // let rpc_addr = self.rpc_addr.clone();
    //     // let client = self.client.clone();
    //     // tokio::spawn(post(rpc_addr, client, msg));
    //     let ret = self.post(&msg).await?;
    //     let tx_hash = serde_json::from_value(ret).expect("failed to parse `send_tx` response");
    //     Ok(tx_hash)
    // }

    /// Sends the transaction to the ZKSync server using the JSON RPC.
    pub async fn send_tx(
        &self,
        tx: FranklinTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<TxHash, failure::Error> {
        use rand::Rng;
        let tx_hash = tx.clone().hash();

        let msg = JsonRpcRequest::submit_tx(tx, eth_signature);

        let mut rng = rand::thread_rng();
        let delay = rng.gen_range::<u64>(0, 10);

        let rpc_addr = self.rpc_addr.clone();
        let client = self.client.clone();
        tokio::spawn(post(delay, rpc_addr, client, msg));
        // let tx_hash = serde_json::from_value(ret).expect("failed to parse `send_tx` response");
        Ok(tx_hash)
    }

    pub async fn post_batch(&self, messages: Vec<JsonRpcRequest>) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let delay = rng.gen_range::<u64>(0, 10);

        let rpc_addr = self.rpc_addr.clone();
        let client = self.client.clone();
        tokio::spawn(post_batch(delay, rpc_addr, client, messages));
    }

    pub async fn prepare_send_tx_request(
        &self,
        tx: FranklinTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> (TxHash, JsonRpcRequest) {
        let tx_hash = tx.clone().hash();

        let msg = JsonRpcRequest::submit_tx(tx, eth_signature);

        (tx_hash, msg)
    }

    pub async fn prepare_send_txs_batch_request(
        &self,
        txs: Vec<(FranklinTx, Option<PackedEthSignature>)>,
    ) -> (Vec<TxHash>, JsonRpcRequest) {
        let tx_hashes = txs.iter().map(|tx| tx.0.hash()).collect();

        let msg = JsonRpcRequest::submit_txs_batch(txs);

        (tx_hashes, msg)

        // let ret = self.post(&msg).await?;
        // let tx_hashes =
        //     serde_json::from_value(ret).expect("failed to parse `send_txs_batch` response");
        // Ok(tx_hashes)
    }

    // pub async fn send_txs_batch(
    //     &self,
    //     txs: Vec<(FranklinTx, Option<PackedEthSignature>)>,
    // ) -> Result<Vec<TxHash>, failure::Error> {
    //     let msg = JsonRpcRequest::submit_txs_batch(txs);

    //     let ret = self.post(&msg).await?;
    //     let tx_hashes =
    //         serde_json::from_value(ret).expect("failed to parse `send_txs_batch` response");
    //     Ok(tx_hashes)
    // }

    /// Sends the transaction to the ZKSync server and returns raw response.
    pub async fn send_tx_raw(
        &self,
        tx: FranklinTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<Output, failure::Error> {
        let msg = JsonRpcRequest::submit_tx(tx, eth_signature);

        self.post_raw(&msg).await
    }

    /// Requests and returns information about a ZKSync account given its address.
    pub async fn account_state_info(
        &self,
        address: Address,
    ) -> Result<AccountInfoResp, failure::Error> {
        let msg = JsonRpcRequest::account_state(address);

        let ret = self.post(&msg).await?;
        let account_state =
            serde_json::from_value(ret).expect("failed to parse account request response");
        Ok(account_state)
    }

    /// Requests and returns a tuple `(executed, verified)` (as `OperationState`) for
    /// an Ethereum operation given its `serial_id`.
    pub async fn ethop_info(&self, serial_id: u64) -> Result<OperationState, failure::Error> {
        let msg = JsonRpcRequest::ethop_info(serial_id);

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
        let msg = JsonRpcRequest::tx_info(tx_hash);

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
        let reply: Output = self.post_raw(message).await?;

        let ret = match reply {
            Output::Success(v) => v.result,
            Output::Failure(v) => failure::bail!("RPC error: {}", v.error),
        };

        Ok(ret)
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

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TxWithSignature {
        tx: FranklinTx,
        signature: Option<TxEthSignature>,
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

        pub fn submit_txs_batch(txs: Vec<(FranklinTx, Option<PackedEthSignature>)>) -> Self {
            let txs: Vec<_> = txs
                .into_iter()
                .map(|(tx, signature)| TxWithSignature {
                    tx,
                    signature: signature.map(TxEthSignature::EthereumSignature),
                })
                .collect();

            let mut params = Vec::new();
            params.push(serde_json::to_value(txs).expect("serialization fail"));
            Self::create("submit_txs_batch", params)
        }

        pub fn account_state(address: Address) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(address).expect("serialization fail"));
            Self::create("account_info", params)
        }

        pub fn ethop_info(serial_id: u64) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(serial_id).expect("serialization fail"));
            Self::create("ethop_info", params)
        }

        pub fn tx_info(tx_hash: TxHash) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(tx_hash).expect("serialization fail"));
            Self::create("tx_info", params)
        }

        pub fn get_tx_fee(tx_type: &str, address: Address, token_symbol: &str) -> Self {
            let mut params = Vec::new();
            params.push(serde_json::to_value(tx_type).expect("serialization fail"));
            params.push(serde_json::to_value(address).expect("serialization fail"));
            params.push(serde_json::to_value(token_symbol).expect("serialization fail"));
            Self::create("get_tx_fee", params)
        }
    }
}
