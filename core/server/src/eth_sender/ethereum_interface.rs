// Built-in deps
// External uses
use failure::ensure;
use futures::{compat::Future01CompatExt, executor::block_on};
use web3::contract::tokens::Tokenize;
use web3::contract::Options;
use web3::transports::{EventLoopHandle, Http};
use web3::types::{TransactionReceipt, H256, U256};
// Workspace uses
use super::ExecutedTxStatus;
use eth_client::{ETHClient, SignedCallResult};
use models::abi::zksync_contract;
use models::config_options::ConfigurationOptions;

/// Ethereum Interface module provides an abstract interface to
/// interact with the Ethereum blockchain.
///
/// Since this interface is declared as a trait, `ETHSender` won't
/// be highly tied to the actually running Ethereum node, which
/// is good for testing purposes.
///
/// The provided interface is not as rich as the actual `ETHClient`
/// structure, but it is instead optimized for the needs of `ETHSender`.
pub(super) trait EthereumInterface {
    /// Obtains a transaction status from the Ethereum blockchain.
    /// The resulting information is reduced to the following minimum:
    ///
    /// - If transaction was not executed, returned value is `None`.
    /// - If transaction was executed, the information about its success and amount
    ///   of confirmations is returned.
    fn get_tx_status(&self, hash: &H256) -> Result<Option<ExecutedTxStatus>, failure::Error>;

    /// Gets the actual block number.
    fn block_number(&self) -> Result<u64, failure::Error>;

    /// Gets the current gas price.
    fn gas_price(&self) -> Result<U256, failure::Error>;

    /// Gets the current nonce to be used in the transactions.
    fn current_nonce(&self) -> Result<U256, failure::Error>;

    /// Sends a signed transaction to the Ethereum blockchain.
    fn send_tx(&self, signed_tx: &SignedCallResult) -> Result<(), failure::Error>;

    /// Creates a transaction based on the provided parameters and signs it.
    fn sign_call_tx<P: Tokenize>(
        &self,
        func: &str,
        params: P,
        options: Options,
    ) -> Result<SignedCallResult, failure::Error>;
}

/// Wrapper over `ETHClient` using `Http` transport.
/// Supposed to be an actual Ethereum intermediator for the `ETHSender`.
pub struct EthereumHttpClient {
    eth_client: ETHClient<Http>,
    // We have to prevent handle from drop, since it will cause event loop termination.
    _event_loop: EventLoopHandle,
}

impl EthereumHttpClient {
    pub fn new(options: &ConfigurationOptions) -> Result<Self, failure::Error> {
        let (_event_loop, transport) = Http::new(&options.web3_url)?;

        let eth_client = ETHClient::new(
            transport,
            zksync_contract(),
            options.operator_eth_addr,
            options.operator_private_key,
            options.contract_eth_addr,
            options.chain_id,
            options.gas_price_factor,
        );

        Ok(Self {
            eth_client,
            _event_loop,
        })
    }
}

impl EthereumInterface for EthereumHttpClient {
    fn get_tx_status(&self, hash: &H256) -> Result<Option<ExecutedTxStatus>, failure::Error> {
        let receipt = block_on(
            self.eth_client
                .web3
                .eth()
                .transaction_receipt(*hash)
                .compat(),
        )?;

        match receipt {
            Some(TransactionReceipt {
                block_number: Some(tx_block_number),
                status: Some(status),
                ..
            }) => {
                let confirmations = self
                    .block_number()?
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

    fn block_number(&self) -> Result<u64, failure::Error> {
        Ok(block_on(self.eth_client.web3.eth().block_number().compat()).map(|n| n.as_u64())?)
    }

    fn send_tx(&self, signed_tx: &SignedCallResult) -> Result<(), failure::Error> {
        let hash = block_on(self.eth_client.send_raw_tx(signed_tx.raw_tx.clone()))?;
        ensure!(
            hash == signed_tx.hash,
            "Hash from signer and Ethereum node mismatch"
        );
        Ok(())
    }

    fn gas_price(&self) -> Result<U256, failure::Error> {
        block_on(self.eth_client.get_gas_price())
    }

    fn current_nonce(&self) -> Result<U256, failure::Error> {
        block_on(self.eth_client.current_nonce()).map_err(From::from)
    }

    fn sign_call_tx<P: Tokenize>(
        &self,
        func: &str,
        params: P,
        options: Options,
    ) -> Result<SignedCallResult, failure::Error> {
        block_on(self.eth_client.sign_call_tx(func, params, options))
    }
}
