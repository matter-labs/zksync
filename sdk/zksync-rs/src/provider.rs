// Provider API. TODO: Describe what's here (ZKS-116).
// from: https://github.com/matter-labs/zksync-dev/blob/dev/core/loadtest/src/rpc_client.rs

// Built-in imports
use std::time::Duration;

// External uses
use async_trait::async_trait;
use jsonrpc_core::{types::response::Output, ErrorCode};
use num::BigUint;

// Workspace uses
use zksync_types::{
    network::Network,
    tx::{PackedEthSignature, TxHash, ZkSyncTx},
    Address, TokenLike, TxFeeTypes,
};

// Local uses
use self::messages::JsonRpcRequest;
use crate::{error::ClientError, types::*};

/// Returns a corresponding address for a provided network name.
pub fn get_rpc_addr(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "https://api.zksync.io/jsrpc",
        Network::Rinkeby => "https://rinkeby-api.zksync.io/jsrpc",
        Network::Ropsten => "https://ropsten-api.zksync.io/jsrpc",
        Network::Localhost => "http://127.0.0.1:3030",
        Network::Unknown => panic!("Attempt to create a provider from an unknown network"),
        Network::Test => panic!("Attempt to create a provider from an test network"),
    }
}

pub type ResponseResult<T> = Result<T, ClientError>;

#[async_trait]
/// `Provider` used to connect to zkSync network in order to send transactions
/// and retrieve some information from the server about
/// zkSync accounts, transactions, supported tokens and the like.
pub trait Provider {
    /// Requests and returns information about a ZKSync account given its address.
    async fn account_info(&self, address: Address) -> ResponseResult<AccountInfo>;

    /// Requests and returns a list of tokens supported by zkSync.
    async fn tokens(&self) -> ResponseResult<Tokens>;

    /// Requests and returns information about transaction execution status.
    async fn tx_info(&self, tx_hash: TxHash) -> ResponseResult<TransactionInfo>;

    /// Obtains minimum fee required to process transaction in zkSync network.
    async fn get_tx_fee(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token: impl Into<TokenLike> + Send + 'async_trait,
    ) -> ResponseResult<Fee>;

    /// Obtains minimum fee required to process transactions batch in zkSync network.
    async fn get_txs_batch_fee(
        &self,
        tx_types: Vec<TxFeeTypes>,
        addresses: Vec<Address>,
        token: impl Into<TokenLike> + Send + 'async_trait,
    ) -> ResponseResult<BigUint>;

    /// Requests and returns information about an Ethereum operation given its `serial_id`.
    async fn ethop_info(&self, serial_id: u32) -> ResponseResult<EthOpInfo>;

    /// Requests and returns Ethereum withdrawal transaction hash for some offchain withdrawal.
    async fn get_eth_tx_for_withdrawal(
        &self,
        withdrawal_hash: TxHash,
    ) -> ResponseResult<Option<String>>;

    /// Requests and returns a smart contract address (for Ethereum network associated with network specified in `Provider`).
    async fn contract_address(&self) -> ResponseResult<ContractAddress>;

    /// Submits a transaction to the zkSync network.
    /// Returns the hash of the created transaction.
    async fn send_tx(
        &self,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> ResponseResult<TxHash>;

    /// Submits a batch of transactions to the zkSync network.
    /// Returns the hashes of the created transactions.
    async fn send_txs_batch(
        &self,
        txs_signed: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
        eth_signature: Option<PackedEthSignature>,
    ) -> ResponseResult<Vec<TxHash>>;

    /// Type of network this provider is allowing access to.
    fn network(&self) -> Network;
}

/// `RpcProvider` is capable of interacting with the ZKSync node via its
/// JSON RPC interface.
#[derive(Debug, Clone)]
pub struct RpcProvider {
    rpc_addr: String,
    client: reqwest::Client,
    network: Network,
}

#[async_trait]
impl Provider for RpcProvider {
    async fn account_info(&self, address: Address) -> ResponseResult<AccountInfo> {
        let msg = JsonRpcRequest::account_info(address);
        self.send_and_deserialize(&msg).await
    }

    async fn tokens(&self) -> ResponseResult<Tokens> {
        let msg = JsonRpcRequest::tokens();
        self.send_and_deserialize(&msg).await
    }

    async fn tx_info(&self, tx_hash: TxHash) -> ResponseResult<TransactionInfo> {
        let msg = JsonRpcRequest::tx_info(tx_hash);
        self.send_and_deserialize(&msg).await
    }

    async fn get_tx_fee(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token: impl Into<TokenLike> + Send + 'async_trait,
    ) -> ResponseResult<Fee> {
        let token = token.into();
        let msg = JsonRpcRequest::get_tx_fee(tx_type, address, token);
        self.send_and_deserialize(&msg).await
    }

    async fn get_txs_batch_fee(
        &self,
        tx_types: Vec<TxFeeTypes>,
        addresses: Vec<Address>,
        token: impl Into<TokenLike> + Send + 'async_trait,
    ) -> ResponseResult<BigUint> {
        let msg = JsonRpcRequest::get_txs_batch_fee_in_wei(tx_types, addresses, token.into());

        let batch_fee: BatchFee = self.send_and_deserialize(&msg).await?;
        Ok(batch_fee.total_fee)
    }

    async fn ethop_info(&self, serial_id: u32) -> ResponseResult<EthOpInfo> {
        let msg = JsonRpcRequest::ethop_info(serial_id);
        self.send_and_deserialize(&msg).await
    }

    async fn get_eth_tx_for_withdrawal(
        &self,
        withdrawal_hash: TxHash,
    ) -> ResponseResult<Option<String>> {
        let msg = JsonRpcRequest::eth_tx_for_withdrawal(withdrawal_hash);
        self.send_and_deserialize(&msg).await
    }

    async fn contract_address(&self) -> ResponseResult<ContractAddress> {
        let msg = JsonRpcRequest::contract_address();
        self.send_and_deserialize(&msg).await
    }

    async fn send_tx(
        &self,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> ResponseResult<TxHash> {
        let msg = JsonRpcRequest::submit_tx(tx, eth_signature);
        self.send_and_deserialize(&msg).await
    }

    async fn send_txs_batch(
        &self,
        txs_signed: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
        eth_signature: Option<PackedEthSignature>,
    ) -> ResponseResult<Vec<TxHash>> {
        let msg = JsonRpcRequest::submit_tx_batch(txs_signed, eth_signature);
        self.send_and_deserialize(&msg).await
    }

    fn network(&self) -> Network {
        self.network
    }
}

impl RpcProvider {
    /// Creates a new `RpcProvider` connected to the desired zkSync network.
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

    /// Creates a new `Provider` object connected to a custom address and the desired zkSync network.
    pub fn from_addr_and_network(rpc_addr: impl Into<String>, network: Network) -> Self {
        Self {
            rpc_addr: rpc_addr.into(),
            client: reqwest::Client::new(),
            network,
        }
    }

    /// Submits a batch transaction to the zkSync network.
    /// Returns the hashes of the created transactions.
    pub async fn send_txs_batch(
        &self,
        txs_signed: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<Vec<TxHash>, ClientError> {
        let msg = JsonRpcRequest::submit_tx_batch(txs_signed, eth_signature);
        self.send_and_deserialize(&msg).await
    }

    /// Requests and returns information about an Ethereum operation given its `serial_id`.
    pub async fn ethop_info(&self, serial_id: u32) -> Result<EthOpInfo, ClientError> {
        let msg = JsonRpcRequest::ethop_info(serial_id);
        self.send_and_deserialize(&msg).await
    }

    /// Requests and returns eth withdrawal transaction hash for some offchain withdrawal.
    pub async fn get_eth_tx_for_withdrawal(
        &self,
        withdrawal_hash: TxHash,
    ) -> Result<Option<String>, ClientError> {
        let msg = JsonRpcRequest::eth_tx_for_withdrawal(withdrawal_hash);
        self.send_and_deserialize(&msg).await
    }

    /// Performs a POST query to the JSON RPC endpoint,
    /// and decodes the response, returning the decoded `serde_json::Value`.
    /// `Ok` is returned only for successful calls, for any kind of error
    /// the `Err` variant is returned (including the failed RPC method
    /// execution response).
    async fn post(&self, message: impl serde::Serialize) -> ResponseResult<serde_json::Value> {
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
                Err(ClientError::RpcError(fail)) => is_recoverable(&fail.error.code),
                Ok(Output::Failure(fail)) => is_recoverable(&fail.error.code),
                _ => false,
            };

            if should_retry && delay < MAX_DURATION {
                delay *= 2;
                tokio::time::delay_for(delay).await;
                continue;
            }

            return match result? {
                Output::Success(success) => Ok(success.result),
                Output::Failure(failure) => Err(ClientError::RpcError(failure)),
            };
        }
    }

    /// Performs a POST query to the JSON RPC endpoint,
    /// and decodes the response, returning the decoded `serde_json::Value`.
    /// `Ok` is returned only for successful calls, for any kind of error
    /// the `Err` variant is returned (including the failed RPC method
    /// execution response).
    async fn post_raw(&self, message: impl serde::Serialize) -> ResponseResult<Output> {
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

    async fn send_and_deserialize<R>(&self, msg: &JsonRpcRequest) -> ResponseResult<R>
    where
        R: serde::de::DeserializeOwned,
    {
        let ret = self.post(msg).await?;
        let result = serde_json::from_value(ret)
            .map_err(|err| ClientError::MalformedResponse(err.to_string()))?;
        Ok(result)
    }
}

mod messages {
    use serde::Serialize;
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

    #[macro_export]
    macro_rules! json_values {
        // Separate values with the comma
        // and allow optional trailing comma
        ($($value: expr),* $(,)?) => {
            vec![
            $(
                to_json_value($value),
            )*
            ]
        }
    }

    #[inline(always)]
    fn to_json_value<T: serde::Serialize>(val: T) -> serde_json::Value {
        serde_json::to_value(val).expect("serialization fail")
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
            Self::create("account_info", json_values![address])
        }

        pub fn submit_tx(tx: ZkSyncTx, eth_signature: Option<PackedEthSignature>) -> Self {
            let params = json_values![tx, eth_signature.map(TxEthSignature::EthereumSignature)];
            Self::create("tx_submit", params)
        }

        pub fn submit_tx_batch(
            txs_signed: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
            eth_signature: Option<PackedEthSignature>,
        ) -> Self {
            let mut params = Vec::with_capacity(2);

            let txs_signed = txs_signed.into_iter().map(|(tx, eth_signature)| {
                serde_json::json!({
                    "tx": to_json_value(tx),
                    "signature": to_json_value(eth_signature.map(TxEthSignature::EthereumSignature)),
                })
            }).collect();
            params.push(serde_json::Value::Array(txs_signed));
            params.push(to_json_value(
                eth_signature.map(TxEthSignature::EthereumSignature),
            ));

            Self::create("submit_txs_batch", params)
        }

        pub fn ethop_info(serial_id: u32) -> Self {
            Self::create("ethop_info", json_values![serial_id])
        }

        pub fn tx_info(tx_hash: TxHash) -> Self {
            Self::create("tx_info", json_values![tx_hash])
        }

        pub fn tokens() -> Self {
            Self::create("tokens", json_values![])
        }

        pub fn contract_address() -> Self {
            Self::create("contract_address", json_values![])
        }

        pub fn eth_tx_for_withdrawal(withdrawal_hash: TxHash) -> Self {
            Self::create("get_eth_tx_for_withdrawal", json_values![withdrawal_hash])
        }

        pub fn get_tx_fee(tx_type: TxFeeTypes, address: Address, token_symbol: TokenLike) -> Self {
            let params = json_values![tx_type, address, token_symbol];
            Self::create("get_tx_fee", params)
        }

        pub fn get_txs_batch_fee_in_wei(
            tx_types: Vec<TxFeeTypes>,
            addresses: Vec<Address>,
            token_like: TokenLike,
        ) -> Self {
            let params = json_values![tx_types, addresses, token_like];
            Self::create("get_txs_batch_fee_in_wei", params)
        }
    }
}
