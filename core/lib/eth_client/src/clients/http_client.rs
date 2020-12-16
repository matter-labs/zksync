// Built-in deps
use std::fmt;

// External uses

use web3::contract::{Contract, Options};
use web3::types::{Address, BlockNumber, Bytes, TransactionReceipt, H160, H256, U256, U64};
use web3::{transports::Http, Web3};

// Workspace uses
use crate::eth_client_trait::{ExecutedTxStatus, FailureInfo, SignedCallResult};

use web3::contract::tokens::Tokenize;
use zksync_eth_signer::{raw_ethereum_tx::RawTransaction, EthereumSigner};

/// Gas limit value to be used in transaction if for some reason
/// gas limit was not set for it.
///
/// This is an emergency value, which will not be used normally.
const FALLBACK_GAS_LIMIT: u64 = 3_000_000;

#[derive(Clone)]
pub struct ETHClient<S: EthereumSigner> {
    eth_signer: S,
    sender_account: Address,
    pub contract_addr: H160,
    contract: ethabi::Contract,
    pub chain_id: u8,
    pub gas_price_factor: f64,
    // It's public only for testkit
    // TODO avoid public
    pub web3: Web3<Http>,
}

impl<S: EthereumSigner> fmt::Debug for ETHClient<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We do not want to have a private key in the debug representation.

        f.debug_struct("ETHClient")
            .field("sender_account", &self.sender_account)
            .field("contract_addr", &self.contract_addr)
            .field("chain_id", &self.chain_id)
            .field("gas_price_factor", &self.gas_price_factor)
            .finish()
    }
}

impl<S: EthereumSigner> ETHClient<S> {
    pub fn new(
        transport: Http,
        contract: ethabi::Contract,
        operator_eth_addr: H160,
        eth_signer: S,
        contract_eth_addr: H160,
        chain_id: u8,
        gas_price_factor: f64,
    ) -> Self {
        Self {
            sender_account: operator_eth_addr,
            eth_signer,
            contract_addr: contract_eth_addr,
            chain_id,
            contract,
            gas_price_factor,
            web3: Web3::new(transport),
        }
    }
    pub fn main_contract_with_address(&self, address: Address) -> Contract<Http> {
        Contract::new(self.web3.eth(), address, self.contract.clone())
    }
    pub fn main_contract(&self) -> Contract<Http> {
        self.main_contract_with_address(self.contract_addr)
    }
    pub async fn pending_nonce(&self) -> Result<U256, anyhow::Error> {
        Ok(self
            .web3
            .eth()
            .transaction_count(self.sender_account, Some(BlockNumber::Pending))
            .await?)
    }

    pub async fn current_nonce(&self) -> Result<U256, anyhow::Error> {
        Ok(self
            .web3
            .eth()
            .transaction_count(self.sender_account, Some(BlockNumber::Latest))
            .await?)
    }

    pub async fn block_number(&self) -> Result<U64, anyhow::Error> {
        Ok(self.web3.eth().block_number().await?)
    }

    pub async fn get_gas_price(&self) -> Result<U256, anyhow::Error> {
        let mut network_gas_price = self.web3.eth().gas_price().await?;
        let percent_gas_price_factor = U256::from((self.gas_price_factor * 100.0).round() as u64);
        network_gas_price = (network_gas_price * percent_gas_price_factor) / U256::from(100);
        Ok(network_gas_price)
    }

    pub async fn balance(&self) -> Result<U256, anyhow::Error> {
        Ok(self.web3.eth().balance(self.sender_account, None).await?)
    }

    pub async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        self.sign_prepared_tx_for_addr(data, self.contract_addr, options)
            .await
    }

    pub async fn sign_prepared_tx_for_addr(
        &self,
        data: Vec<u8>,
        contract_addr: H160,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        // fetch current gas_price
        let gas_price = match options.gas_price {
            Some(gas_price) => gas_price,
            None => self.get_gas_price().await?,
        };

        let nonce = match options.nonce {
            Some(nonce) => nonce,
            None => self.pending_nonce().await?,
        };

        let gas = match options.gas {
            Some(gas) => gas,
            None => {
                // Verbosity level is set to `error`, since we expect all the transactions to have
                // a set limit, but don't want to crush the application if for some reason in some
                // place limit was not set.
                log::error!(
                    "No gas limit was set for transaction, using the default limit: {}",
                    FALLBACK_GAS_LIMIT
                );

                U256::from(FALLBACK_GAS_LIMIT)
            }
        };

        // form and sign tx
        let tx = RawTransaction {
            chain_id: self.chain_id,
            nonce,
            to: Some(contract_addr),
            value: options.value.unwrap_or_default(),
            gas_price,
            gas,
            data,
        };

        let signed_tx = self.eth_signer.sign_transaction(tx).await?;
        let hash = self.web3.web3().sha3(Bytes(signed_tx.clone())).await?;

        Ok(SignedCallResult {
            raw_tx: signed_tx,
            gas_price,
            nonce,
            hash,
        })
    }

    pub async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, anyhow::Error> {
        Ok(self.web3.eth().send_raw_transaction(Bytes(tx)).await?)
    }

    pub async fn tx_receipt(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, anyhow::Error> {
        Ok(self.web3.eth().transaction_receipt(tx_hash).await?)
    }

    pub async fn failure_reason(
        &self,
        tx_hash: H256,
    ) -> Result<Option<FailureInfo>, anyhow::Error> {
        let transaction = self.web3.eth().transaction(tx_hash.into()).await?.unwrap();
        let receipt = self.web3.eth().transaction_receipt(tx_hash).await?.unwrap();

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
            .web3
            .eth()
            .call(call_request, receipt.block_number.map(Into::into))
            .await?;
        let revert_code = hex::encode(&encoded_revert_reason.0);
        let revert_reason = if encoded_revert_reason.0.len() >= 4 {
            let encoded_string_without_function_hash = &encoded_revert_reason.0[4..];

            ethabi::decode(
                &[ethabi::ParamType::String],
                encoded_string_without_function_hash,
            )?
            .into_iter()
            .next()
            .unwrap()
            .to_string()
            .unwrap()
        } else {
            "unknown".to_string()
        };

        Ok(Some(FailureInfo {
            gas_limit,
            gas_used,
            revert_code,
            revert_reason,
        }))
    }
    pub async fn eth_balance(&self, address: Address) -> Result<U256, anyhow::Error> {
        Ok(self.web3.eth().balance(address, None).await?)
    }
    // TODO remove it from basic interface
    pub async fn contract_balance(
        &self,
        token_address: Address,
        abi: ethabi::Contract,
        address: Address,
    ) -> Result<U256, anyhow::Error> {
        let contract = Contract::new(self.web3.eth(), token_address, abi);
        Ok(contract
            .query("balanceOf", address, None, Options::default(), None)
            .await?)
    }

    pub async fn allowance(
        &self,
        token_address: Address,
        erc20_abi: ethabi::Contract,
    ) -> Result<U256, anyhow::Error> {
        let contract = Contract::new(self.web3.eth(), token_address, erc20_abi);

        Ok(contract
            .query(
                "allowance",
                (self.sender_account, self.contract_addr),
                None,
                Options::default(),
                None,
            )
            .await?)
    }

    pub async fn get_tx_status(&self, hash: &H256) -> anyhow::Result<Option<ExecutedTxStatus>> {
        let receipt = self.tx_receipt(*hash).await?;

        match receipt {
            Some(TransactionReceipt {
                block_number: Some(tx_block_number),
                status: Some(status),
                ..
            }) => {
                let confirmations = self
                    .block_number()
                    .await?
                    .saturating_sub(tx_block_number)
                    .as_u64();
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
    pub fn contract(&self) -> &ethabi::Contract {
        &self.contract
    }

    pub fn encode_tx_data<P: Tokenize>(&self, func: &str, params: P) -> Vec<u8> {
        let f = self
            .contract()
            .function(func)
            .expect("failed to get function parameters");

        f.encode_input(&params.into_tokens())
            .expect("failed to encode parameters")
    }
}
