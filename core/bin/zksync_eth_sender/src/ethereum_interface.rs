// Built-in deps
// External uses

use anyhow::ensure;
use web3::contract::tokens::Tokenize;
use web3::contract::Options;
use web3::transports::Http;
use zksync_basic_types::{TransactionReceipt, H256, U256};
use zksync_eth_signer::PrivateKeySigner;
// Workspace uses
use super::ExecutedTxStatus;
use std::time::Duration;
use zksync_config::ConfigurationOptions;
use zksync_contracts::zksync_contract;
use zksync_eth_client::{ETHClient, SignedCallResult};

/// Sleep time between consecutive requests.
const SLEEP_DURATION: Duration = Duration::from_millis(250);

/// Information about transaction failure.
#[derive(Debug, Clone)]
pub struct FailureInfo {
    pub revert_code: String,
    pub revert_reason: String,
    pub gas_used: Option<U256>,
    pub gas_limit: U256,
}

/// Ethereum Interface module provides an abstract interface to
/// interact with the Ethereum blockchain.
///
/// Since this interface is declared as a trait, `ETHSender` won't
/// be highly tied to the actually running Ethereum node, which
/// is good for testing purposes.
///
/// The provided interface is not as rich as the actual `ETHClient`
/// structure, but it is instead optimized for the needs of `ETHSender`.
#[async_trait::async_trait]
pub(super) trait EthereumInterface {
    /// Obtains a transaction status from the Ethereum blockchain.
    /// The resulting information is reduced to the following minimum:
    ///
    /// - If transaction was not executed, returned value is `None`.
    /// - If transaction was executed, the information about its success and amount
    ///   of confirmations is returned.
    async fn get_tx_status(&self, hash: &H256) -> Result<Option<ExecutedTxStatus>, anyhow::Error>;

    /// Gets the actual block number.
    async fn block_number(&self) -> Result<u64, anyhow::Error>;

    /// Gets the current gas price.
    async fn gas_price(&self) -> Result<U256, anyhow::Error>;

    /// Sends a signed transaction to the Ethereum blockchain.
    async fn send_tx(&self, signed_tx: &SignedCallResult) -> Result<(), anyhow::Error>;

    /// Encodes the transaction data (smart contract method and its input) to the bytes
    /// without creating an actual transaction.
    fn encode_tx_data<P: Tokenize>(&self, func: &str, params: P) -> Vec<u8>;

    /// Signs the transaction given the previously encoded data.
    /// Fills in gas/nonce if not supplied inside options.
    async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error>;

    /// Returns the information about transaction failure reason.
    async fn failure_reason(&self, tx_hash: H256) -> Option<FailureInfo>;
}

/// Wrapper over `ETHClient` using `Http` transport.
/// Supposed to be an actual Ethereum intermediator for the `ETHSender`.
#[derive(Debug)]
pub struct EthereumHttpClient {
    eth_client: ETHClient<Http, PrivateKeySigner>,
}

impl EthereumHttpClient {
    pub fn new(options: &ConfigurationOptions) -> Result<Self, anyhow::Error> {
        let transport = Http::new(&options.web3_url)?;
        let ethereum_signer = PrivateKeySigner::new(
            options
                .operator_private_key
                .expect("Operator private key is required for eth_sender"),
        );

        let eth_client = ETHClient::new(
            transport,
            zksync_contract(),
            options.operator_commit_eth_addr,
            ethereum_signer,
            options.contract_eth_addr,
            options.chain_id,
            options.gas_price_factor,
        );

        Ok(Self { eth_client })
    }

    /// Sleep is required before each Ethereum query because infura blocks requests that are made too often
    fn sleep(&self) {
        std::thread::sleep(SLEEP_DURATION);
    }
}

#[async_trait::async_trait]
impl EthereumInterface for EthereumHttpClient {
    async fn get_tx_status(&self, hash: &H256) -> Result<Option<ExecutedTxStatus>, anyhow::Error> {
        self.sleep();
        let receipt = self
            .eth_client
            .web3
            .eth()
            .transaction_receipt(*hash)
            .await?;

        match receipt {
            Some(TransactionReceipt {
                block_number: Some(tx_block_number),
                status: Some(status),
                ..
            }) => {
                let confirmations = self
                    .block_number()
                    .await?
                    .saturating_sub(tx_block_number.as_u64());
                let success = status.as_u64() == 1;

                // Set the receipt only for failures.
                let receipt = if success {
                    None
                } else {
                    Some(receipt.unwrap())
                };

                Ok(Some(ExecutedTxStatus {
                    confirmations,
                    success,
                    receipt,
                }))
            }
            _ => Ok(None),
        }
    }

    async fn block_number(&self) -> Result<u64, anyhow::Error> {
        self.sleep();
        let block_number = self.eth_client.web3.eth().block_number().await?;
        Ok(block_number.as_u64())
    }

    async fn send_tx(&self, signed_tx: &SignedCallResult) -> Result<(), anyhow::Error> {
        self.sleep();
        let hash = self
            .eth_client
            .send_raw_tx(signed_tx.raw_tx.clone())
            .await?;
        ensure!(
            hash == signed_tx.hash,
            "Hash from signer and Ethereum node mismatch"
        );
        Ok(())
    }

    async fn gas_price(&self) -> Result<U256, anyhow::Error> {
        self.sleep();
        self.eth_client.get_gas_price().await
    }

    fn encode_tx_data<P: Tokenize>(&self, func: &str, params: P) -> Vec<u8> {
        self.eth_client.encode_tx_data(func, params)
    }

    async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        self.sleep();
        self.eth_client.sign_prepared_tx(data, options).await
    }

    async fn failure_reason(&self, tx_hash: H256) -> Option<FailureInfo> {
        let transaction = self
            .eth_client
            .web3
            .eth()
            .transaction(tx_hash.into())
            .await
            .ok()??;
        let receipt = self
            .eth_client
            .web3
            .eth()
            .transaction_receipt(tx_hash)
            .await
            .ok()??;

        let gas_limit = transaction.gas;
        let gas_used = receipt.gas_used;

        let call_request = web3::types::CallRequest {
            from: Some(transaction.from),
            to: transaction.to,
            gas: Some(transaction.gas),
            gas_price: Some(transaction.gas_price),
            value: Some(transaction.value),
            data: Some(transaction.input),
        };

        let encoded_revert_reason = self
            .eth_client
            .web3
            .eth()
            .call(call_request, receipt.block_number.map(Into::into))
            .await
            .ok()?;
        let revert_code = hex::encode(&encoded_revert_reason.0);
        let revert_reason = if encoded_revert_reason.0.len() >= 4 {
            let encoded_string_without_function_hash = &encoded_revert_reason.0[4..];

            ethabi::decode(
                &[ethabi::ParamType::String],
                encoded_string_without_function_hash,
            )
            .ok()?
            .into_iter()
            .next()?
            .to_string()?
        } else {
            "unknown".to_string()
        };

        Some(FailureInfo {
            gas_limit,
            gas_used,
            revert_code,
            revert_reason,
        })
    }
}
