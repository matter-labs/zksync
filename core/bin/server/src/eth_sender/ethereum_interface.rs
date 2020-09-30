// Built-in deps
// External uses
use eth_client::eth_signer::EthereumSigner;
use failure::ensure;
use futures::compat::Future01CompatExt;
use web3::contract::tokens::Tokenize;
use web3::contract::Options;
use web3::transports::{EventLoopHandle, Http};
use zksync_basic_types::{TransactionReceipt, H256, U256};
// Workspace uses
use super::ExecutedTxStatus;
use eth_client::{ETHClient, SignedCallResult};
use std::time::Duration;
use zksync_config::ConfigurationOptions;
use zksync_contracts::zksync_contract;

/// Sleep time between consecutive requests.
const SLEEP_DURATION: Duration = Duration::from_millis(250);

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
    async fn get_tx_status(&self, hash: &H256) -> Result<Option<ExecutedTxStatus>, failure::Error>;

    /// Gets the actual block number.
    async fn block_number(&self) -> Result<u64, failure::Error>;

    /// Gets the current gas price.
    async fn gas_price(&self) -> Result<U256, failure::Error>;

    /// Sends a signed transaction to the Ethereum blockchain.
    async fn send_tx(&self, signed_tx: &SignedCallResult) -> Result<(), failure::Error>;

    /// Encodes the transaction data (smart contract method and its input) to the bytes
    /// without creating an actual transaction.
    fn encode_tx_data<P: Tokenize>(&self, func: &str, params: P) -> Vec<u8>;

    /// Signs the transaction given the previously encoded data.
    /// Fills in gas/nonce if not supplied inside options.
    async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, failure::Error>;
}

/// Wrapper over `ETHClient` using `Http` transport.
/// Supposed to be an actual Ethereum intermediator for the `ETHSender`.
#[derive(Debug)]
pub struct EthereumHttpClient {
    eth_client: ETHClient<Http>,
    // We have to prevent handle from drop, since it will cause event loop termination.
    _event_loop: EventLoopHandle,
}

impl EthereumHttpClient {
    pub fn new(options: &ConfigurationOptions) -> Result<Self, failure::Error> {
        let (_event_loop, transport) = Http::new(&options.web3_url)?;
        let ethereum_signer = EthereumSigner::from_key(
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

        Ok(Self {
            eth_client,
            _event_loop,
        })
    }

    /// Sleep is required before each Ethereum query because infura blocks requests that are made too often
    fn sleep(&self) {
        std::thread::sleep(SLEEP_DURATION);
    }
}

#[async_trait::async_trait]
impl EthereumInterface for EthereumHttpClient {
    async fn get_tx_status(&self, hash: &H256) -> Result<Option<ExecutedTxStatus>, failure::Error> {
        self.sleep();
        let receipt = self
            .eth_client
            .web3
            .eth()
            .transaction_receipt(*hash)
            .compat()
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

    async fn block_number(&self) -> Result<u64, failure::Error> {
        self.sleep();
        let block_number = self.eth_client.web3.eth().block_number().compat().await?;
        Ok(block_number.as_u64())
    }

    async fn send_tx(&self, signed_tx: &SignedCallResult) -> Result<(), failure::Error> {
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

    async fn gas_price(&self) -> Result<U256, failure::Error> {
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
    ) -> Result<SignedCallResult, failure::Error> {
        self.sleep();
        self.eth_client.sign_prepared_tx(data, options).await
    }
}
