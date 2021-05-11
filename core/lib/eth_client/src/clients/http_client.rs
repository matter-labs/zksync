// Built-in deps
use std::sync::Arc;
use std::{fmt, time::Instant};

// External uses
use web3::{
    contract::{
        tokens::{Detokenize, Tokenize},
        Contract, Options,
    },
    transports::Http,
    types::{
        Address, BlockId, BlockNumber, Bytes, Filter, Log, Transaction, TransactionId,
        TransactionReceipt, H160, H256, U256, U64,
    },
    Web3,
};

// Workspace uses
use zksync_eth_signer::{raw_ethereum_tx::RawTransaction, EthereumSigner};

use crate::ethereum_gateway::{ExecutedTxStatus, FailureInfo, SignedCallResult};
/// Gas limit value to be used in transaction if for some reason
/// gas limit was not set for it.
///
/// This is an emergency value, which will not be used normally.
const FALLBACK_GAS_LIMIT: u64 = 3_000_000;

struct ETHDirectClientInner<S: EthereumSigner> {
    eth_signer: S,
    sender_account: Address,
    contract_addr: H160,
    contract: ethabi::Contract,
    chain_id: u8,
    gas_price_factor: f64,
    web3: Web3<Http>,
}

#[derive(Clone)]
pub struct ETHDirectClient<S: EthereumSigner> {
    inner: Arc<ETHDirectClientInner<S>>,
}

impl<S: EthereumSigner> fmt::Debug for ETHDirectClient<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We do not want to have a private key in the debug representation.

        f.debug_struct("ETHDirectClient")
            .field("sender_account", &self.inner.sender_account)
            .field("contract_addr", &self.inner.contract_addr)
            .field("chain_id", &self.inner.chain_id)
            .field("gas_price_factor", &self.inner.gas_price_factor)
            .finish()
    }
}

impl<S: EthereumSigner> ETHDirectClient<S> {
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
            inner: Arc::new(ETHDirectClientInner {
                sender_account: operator_eth_addr,
                eth_signer,
                contract_addr: contract_eth_addr,
                chain_id,
                contract,
                gas_price_factor,
                web3: Web3::new(transport),
            }),
        }
    }

    pub fn main_contract_with_address(&self, address: Address) -> Contract<Http> {
        Contract::new(self.inner.web3.eth(), address, self.inner.contract.clone())
    }

    pub fn main_contract(&self) -> Contract<Http> {
        self.main_contract_with_address(self.inner.contract_addr)
    }

    pub fn create_contract(&self, address: Address, contract: ethabi::Contract) -> Contract<Http> {
        Contract::new(self.inner.web3.eth(), address, contract)
    }

    pub async fn pending_nonce(&self) -> Result<U256, anyhow::Error> {
        let start = Instant::now();
        let count = self
            .inner
            .web3
            .eth()
            .transaction_count(self.inner.sender_account, Some(BlockNumber::Pending))
            .await?;
        metrics::histogram!("eth_client.direct.pending_nonce", start.elapsed());
        Ok(count)
    }

    pub async fn current_nonce(&self) -> Result<U256, anyhow::Error> {
        let start = Instant::now();
        let nonce = self
            .inner
            .web3
            .eth()
            .transaction_count(self.inner.sender_account, Some(BlockNumber::Latest))
            .await?;
        metrics::histogram!("eth_client.direct.current_nonce", start.elapsed());
        Ok(nonce)
    }

    pub async fn block(
        &self,
        id: BlockId,
    ) -> Result<Option<web3::types::Block<H256>>, anyhow::Error> {
        let start = Instant::now();
        let block = self.inner.web3.eth().block(id).await?;
        metrics::histogram!("eth_client.direct.block", start.elapsed());
        Ok(block)
    }

    pub async fn block_number(&self) -> Result<U64, anyhow::Error> {
        let start = Instant::now();
        let block_number = self.inner.web3.eth().block_number().await?;
        metrics::histogram!("eth_client.direct.current_nonce", start.elapsed());
        Ok(block_number)
    }

    pub async fn get_gas_price(&self) -> Result<U256, anyhow::Error> {
        let start = Instant::now();
        let mut network_gas_price = self.inner.web3.eth().gas_price().await?;
        let percent_gas_price_factor =
            U256::from((self.inner.gas_price_factor * 100.0).round() as u64);
        network_gas_price = (network_gas_price * percent_gas_price_factor) / U256::from(100);
        metrics::histogram!("eth_client.direct.get_gas_price", start.elapsed());
        Ok(network_gas_price)
    }

    pub async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        self.sign_prepared_tx_for_addr(data, self.inner.contract_addr, options)
            .await
    }

    pub async fn sign_prepared_tx_for_addr(
        &self,
        data: Vec<u8>,
        contract_addr: H160,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        // fetch current gas_price
        let start = Instant::now();

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
                vlog::error!(
                    "No gas limit was set for transaction, using the default limit: {}",
                    FALLBACK_GAS_LIMIT
                );

                U256::from(FALLBACK_GAS_LIMIT)
            }
        };

        // form and sign tx
        let tx = RawTransaction {
            chain_id: self.inner.chain_id,
            nonce,
            to: Some(contract_addr),
            value: options.value.unwrap_or_default(),
            gas_price,
            gas,
            data,
        };

        let signed_tx = self.inner.eth_signer.sign_transaction(tx).await?;
        let hash = self
            .inner
            .web3
            .web3()
            .sha3(Bytes(signed_tx.clone()))
            .await?;

        metrics::histogram!(
            "eth_client.direct.sign_prepared_tx_for_addr",
            start.elapsed()
        );
        Ok(SignedCallResult {
            raw_tx: signed_tx,
            gas_price,
            nonce,
            hash,
        })
    }

    pub async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, anyhow::Error> {
        let start = Instant::now();
        let tx = self
            .inner
            .web3
            .eth()
            .send_raw_transaction(Bytes(tx))
            .await?;
        metrics::histogram!("eth_client.direct.send_raw_tx", start.elapsed());
        Ok(tx)
    }

    pub async fn tx_receipt(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, anyhow::Error> {
        let start = Instant::now();
        let receipt = self.inner.web3.eth().transaction_receipt(tx_hash).await?;
        metrics::histogram!("eth_client.direct.tx_receipt", start.elapsed());
        Ok(receipt)
    }

    pub async fn failure_reason(
        &self,
        tx_hash: H256,
    ) -> Result<Option<FailureInfo>, anyhow::Error> {
        let start = Instant::now();
        let transaction = self.inner.web3.eth().transaction(tx_hash.into()).await?;
        let receipt = self.inner.web3.eth().transaction_receipt(tx_hash).await?;

        match (transaction, receipt) {
            (Some(transaction), Some(receipt)) => {
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
                    .inner
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

                metrics::histogram!("eth_client.direct.failure_reason", start.elapsed());
                Ok(Some(FailureInfo {
                    revert_code,
                    revert_reason,
                    gas_used,
                    gas_limit,
                }))
            }
            _ => Ok(None),
        }
    }

    pub async fn eth_balance(&self, address: Address) -> Result<U256, anyhow::Error> {
        let start = Instant::now();
        let balance = self.inner.web3.eth().balance(address, None).await?;
        metrics::histogram!("eth_client.direct.eth_balance", start.elapsed());
        Ok(balance)
    }

    pub async fn sender_eth_balance(&self) -> Result<U256, anyhow::Error> {
        self.eth_balance(self.inner.sender_account).await
    }

    pub async fn allowance(
        &self,
        token_address: Address,
        erc20_abi: ethabi::Contract,
    ) -> Result<U256, anyhow::Error> {
        let start = Instant::now();
        let res = self
            .call_contract_function(
                "allowance",
                (self.inner.sender_account, self.inner.contract_addr),
                None,
                Options::default(),
                None,
                token_address,
                erc20_abi,
            )
            .await?;
        metrics::histogram!("eth_client.direct.allowance", start.elapsed());
        Ok(res)
    }

    pub async fn call_main_contract_function<R, A, P, B>(
        &self,
        func: &str,
        params: P,
        from: A,
        options: Options,
        block: B,
    ) -> Result<R, anyhow::Error>
    where
        R: Detokenize + Unpin,
        A: Into<Option<Address>>,
        B: Into<Option<BlockId>>,
        P: Tokenize,
    {
        self.call_contract_function(
            func,
            params,
            from,
            options,
            block,
            self.inner.contract_addr,
            self.inner.contract.clone(),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn call_contract_function<R, A, B, P>(
        &self,
        func: &str,
        params: P,
        from: A,
        options: Options,
        block: B,
        token_address: Address,
        erc20_abi: ethabi::Contract,
    ) -> Result<R, anyhow::Error>
    where
        R: Detokenize + Unpin,
        A: Into<Option<Address>>,
        B: Into<Option<BlockId>>,
        P: Tokenize,
    {
        let start = Instant::now();
        let contract = Contract::new(self.inner.web3.eth(), token_address, erc20_abi);
        let res = contract.query(func, params, from, options, block).await?;
        metrics::histogram!("eth_client.direct.call_contract_function", start.elapsed());
        Ok(res)
    }

    pub async fn get_tx_status(&self, hash: H256) -> anyhow::Result<Option<ExecutedTxStatus>> {
        let start = Instant::now();

        let receipt = self.tx_receipt(hash).await?;
        let res: Result<Option<ExecutedTxStatus>, anyhow::Error> = match receipt {
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
        };
        metrics::histogram!("eth_client.direct.get_tx_status", start.elapsed());
        Ok(res?)
    }

    pub async fn logs(&self, filter: Filter) -> anyhow::Result<Vec<Log>> {
        let start = Instant::now();
        let logs = self.inner.web3.eth().logs(filter).await?;
        metrics::histogram!("eth_client.direct.logs", start.elapsed());
        Ok(logs)
    }

    pub fn contract(&self) -> &ethabi::Contract {
        &self.inner.contract
    }

    pub fn contract_addr(&self) -> H160 {
        self.inner.contract_addr
    }

    pub fn chain_id(&self) -> u8 {
        self.inner.chain_id
    }

    pub fn gas_price_factor(&self) -> f64 {
        self.inner.gas_price_factor
    }

    pub fn encode_tx_data<P: Tokenize>(&self, func: &str, params: P) -> Vec<u8> {
        let f = self
            .contract()
            .function(func)
            .expect("failed to get function parameters");

        f.encode_input(&params.into_tokens())
            .expect("failed to encode parameters")
    }

    pub fn get_web3_transport(&self) -> &Http {
        self.inner.web3.transport()
    }

    pub async fn get_tx(&self, hash: H256) -> Result<Option<Transaction>, anyhow::Error> {
        let tx = self
            .inner
            .web3
            .eth()
            .transaction(TransactionId::Hash(hash))
            .await?;
        Ok(tx)
    }
}
