// Provider API. TODO: Describe what's here.
// from: https://github.com/matter-labs/zksync-dev/blob/dev/core/loadtest/src/rpc_client.rs

// Built-in imports
use std::time::Duration;

// External uses
use jsonrpc_core::{types::response::Output, ErrorCode};

// Workspace uses
use zksync_types::{
    tx::{PackedEthSignature, TxHash, ZkSyncTx},
    Address, TokenLike, TxFeeTypes,
};

// Local uses
use self::messages::JsonRpcRequest;
use crate::{error::ClientError, types::network::Network, types::*};

/// Returns a corresponding address for a provided network name.
pub fn get_rpc_addr(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "https://api.zksync.io/jsrpc",
        Network::Rinkeby => "https://rinkeby-api.zksync.io/jsrpc",
        Network::Ropsten => "https://ropsten-api.zksync.io/jsrpc",
        Network::Localhost => "http://127.0.0.1:3030",
        Network::Unknown => panic!("Attempt to create a provider from an unknown network"),
    }
}

/// `Provider` is capable of interacting with the ZKSync node via its
/// JSON RPC interface.
#[derive(Debug, Clone)]
pub struct Provider {
    rpc_addr: String,
    client: reqwest::Client,
    pub network: Network,
}

impl Provider {
    /// Creates a new `Provider` connected to the desired zkSync network.
    pub fn new(network: Network) -> Self {
        Self {
            rpc_addr: get_rpc_addr(network).into(),
            client: reqwest::Client::new(),
            network,
        }
    }

    /// Creates a new `Provider` object connected to a custom address.
    pub fn from_addr(rpc_addr: impl Into<String>) -> Self {
        Self {
            rpc_addr: rpc_addr.into(),
            client: reqwest::Client::new(),
            network: Network::Unknown,
        }
    }

    /// Obtains minimum fee required to process transaction in zkSync network.
    pub async fn get_tx_fee(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token: impl Into<TokenLike>,
    ) -> Result<Fee, ClientError> {
        let token = token.into();
        let msg = JsonRpcRequest::get_tx_fee(tx_type, address, token);

        let ret = self.post(&msg).await?;
        let fee = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;

        Ok(fee)
    }

    /// Submits a transaction to the zkSync network.
    /// Returns the hash of the created transaction.
    pub async fn send_tx(
        &self,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<TxHash, ClientError> {
        let msg = JsonRpcRequest::submit_tx(tx, eth_signature);

        let ret = self.post(&msg).await?;
        let tx_hash = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;
        Ok(tx_hash)
    }

    /// Submits a batch transaction to the zkSync network.
    /// Returns the hashes of the created transactions.
    pub async fn send_txs_batch(
        &self,
        txs_signed: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
    ) -> Result<Vec<TxHash>, ClientError> {
        let msg = JsonRpcRequest::submit_tx_batch(txs_signed);

        let ret = self.post(&msg).await?;
        let tx_hashes = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;
        Ok(tx_hashes)
    }

    /// Requests and returns information about a ZKSync account given its address.
    pub async fn account_info(&self, address: Address) -> Result<AccountInfo, ClientError> {
        let msg = JsonRpcRequest::account_info(address);

        let ret = self.post(&msg).await?;
        let account_state = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;
        Ok(account_state)
    }

    /// Requests and returns information about an Ethereum operation given its `serial_id`.
    pub async fn ethop_info(&self, serial_id: u32) -> Result<EthOpInfo, ClientError> {
        let msg = JsonRpcRequest::ethop_info(serial_id);

        let ret = self.post(&msg).await?;
        let eth_op_info = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;
        Ok(eth_op_info)
    }

    /// Requests and returns information about transaction execution status.
    pub async fn tx_info(&self, tx_hash: TxHash) -> Result<TransactionInfo, ClientError> {
        let msg = JsonRpcRequest::tx_info(tx_hash);

        let ret = self.post(&msg).await?;
        let tx_info = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;
        Ok(tx_info)
    }

    /// Requests and returns a list of tokens supported by zkSync.
    pub async fn tokens(&self) -> Result<Tokens, ClientError> {
        let msg = JsonRpcRequest::tokens();

        let ret = self.post(&msg).await?;
        let tx_info = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;
        Ok(tx_info)
    }

    /// Requests and returns a smart contract address (for Ethereum network associated with network specified in `Provider`).
    pub async fn contract_address(&self) -> Result<ContractAddress, ClientError> {
        let msg = JsonRpcRequest::contract_address();

        let ret = self.post(&msg).await?;
        let tx_info = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;
        Ok(tx_info)
    }

    /// Requests and returns eth withdrawal transaction hash for some offchain withdrawal.
    pub async fn get_eth_tx_for_withdrawal(
        &self,
        withdrawal_hash: TxHash,
    ) -> Result<Option<String>, ClientError> {
        let msg = JsonRpcRequest::eth_tx_for_withdrawal(withdrawal_hash);

        let ret = self.post(&msg).await?;
        let tx_info = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;
        Ok(tx_info)
    }

    /// Performs a POST query to the JSON RPC endpoint,
    /// and decodes the response, returning the decoded `serde_json::Value`.
    /// `Ok` is returned only for successful calls, for any kind of error
    /// the `Err` variant is returned (including the failed RPC method
    /// execution response).
    async fn post(&self, message: impl serde::Serialize) -> Result<serde_json::Value, ClientError> {
        // Repeat requests with exponential backoff until an ok response is received to avoid
        // network and internal errors impact.
        const MAX_DURATION: Duration = Duration::from_secs(30);
        let mut delay = Duration::from_millis(50);
        loop {
            let result = self.post_raw(&message).await;

            /// Determines if the error code is recoverable or not.
            fn is_recoverable(code: &ErrorCode) -> bool {
                code == &ErrorCode::InternalError
                // This is a communication error code, so we can make attempt to retry request.
                || code == &ErrorCode::ServerError(300)
            }

            let should_retry = match result.as_ref() {
                Err(ClientError::NetworkError(..)) => true,
                Err(ClientError::RpcError(fail)) if is_recoverable(&fail.error.code) => true,
                Ok(Output::Failure(fail)) if is_recoverable(&fail.error.code) => true,
                _ => false,
            };

            if should_retry && delay < MAX_DURATION {
                delay += delay;
                tokio::time::delay_for(delay).await;
                continue;
            }

            match result? {
                Output::Success(success) => return Ok(success.result),
                Output::Failure(failure) => return Err(ClientError::RpcError(failure)),
            };
        }
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
    use serde_derive::Serialize;
    use zksync_types::{
        tx::{PackedEthSignature, TxEthSignature, TxHash, ZkSyncTx},
        Address, TokenLike, TxFeeTypes,
    };

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

        pub fn account_info(address: Address) -> Self {
            let mut params = Vec::with_capacity(1);
            params.push(serde_json::to_value(address).expect("serialization fail"));
            Self::create("account_info", params)
        }

        pub fn submit_tx(tx: ZkSyncTx, eth_signature: Option<PackedEthSignature>) -> Self {
            let mut params = Vec::with_capacity(2);
            params.push(serde_json::to_value(tx).expect("serialization fail"));
            params.push(
                serde_json::to_value(eth_signature.map(TxEthSignature::EthereumSignature))
                    .expect("serialization fail"),
            );
            Self::create("tx_submit", params)
        }

        pub fn submit_tx_batch(txs_signed: Vec<(ZkSyncTx, Option<PackedEthSignature>)>) -> Self {
            let mut params = Vec::with_capacity(1);

            let txs_signed = txs_signed.into_iter().map(|(tx, eth_signature)| {
                serde_json::json!({
                    "tx": serde_json::to_value(tx).expect("serialization fail"),
                    "signature": serde_json::to_value(eth_signature.map(TxEthSignature::EthereumSignature))
                        .expect("serialization fail"),
                })
            }).collect();
            params.push(serde_json::Value::Array(txs_signed));

            Self::create("submit_txs_batch", params)
        }

        pub fn ethop_info(serial_id: u32) -> Self {
            let mut params = Vec::with_capacity(1);
            params.push(serde_json::to_value(serial_id).expect("serialization fail"));
            Self::create("ethop_info", params)
        }

        pub fn tx_info(tx_hash: TxHash) -> Self {
            let mut params = Vec::with_capacity(1);
            params.push(serde_json::to_value(tx_hash).expect("serialization fail"));
            Self::create("tx_info", params)
        }

        pub fn tokens() -> Self {
            let params = Vec::with_capacity(0);
            Self::create("tokens", params)
        }

        pub fn contract_address() -> Self {
            let params = Vec::with_capacity(0);
            Self::create("contract_address", params)
        }

        pub fn eth_tx_for_withdrawal(withdrawal_hash: TxHash) -> Self {
            let mut params = Vec::with_capacity(1);
            params.push(serde_json::to_value(withdrawal_hash).expect("serialization fail"));
            Self::create("get_eth_tx_for_withdrawal", params)
        }

        pub fn get_tx_fee(tx_type: TxFeeTypes, address: Address, token_symbol: TokenLike) -> Self {
            let mut params = Vec::with_capacity(3);
            params.push(serde_json::to_value(tx_type).expect("serialization fail"));
            params.push(serde_json::to_value(address).expect("serialization fail"));
            params.push(serde_json::to_value(token_symbol).expect("serialization fail"));
            Self::create("get_tx_fee", params)
        }
    }
}
