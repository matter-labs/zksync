use ethabi::Contract;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use web3::{
    contract::tokens::{Detokenize, Tokenize},
    contract::Options,
    transports::Http,
    types::{Address, BlockId, Filter, Log, Transaction, U64},
};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{TransactionReceipt, H160, H256, U256};

use crate::ethereum_gateway::{ExecutedTxStatus, FailureInfo, SignedCallResult};
use crate::ETHDirectClient;

#[derive(Debug, Default)]
struct MultiplexerEthereumClientInner {
    clients: Vec<(String, ETHDirectClient<PrivateKeySigner>)>,
    preferred: AtomicUsize,
}

#[derive(Debug, Default, Clone)]
pub struct MultiplexerEthereumClient {
    inner: Arc<MultiplexerEthereumClientInner>,
}

macro_rules! multiple_call {
    ($self:expr, $func:ident($($attr:expr),*)) => {
        for (name, client) in $self.clients() {
            match client.$func($($attr.clone()),*).await {
                Ok(res) => return Ok(res),
                Err(err) => vlog::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    };
}

impl MultiplexerEthereumClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_client(
        &mut self,
        name: String,
        client: ETHDirectClient<PrivateKeySigner>,
    ) -> &mut Self {
        Arc::get_mut(&mut self.inner)
            .unwrap()
            .clients
            .push((name, client));
        self
    }

    pub fn prioritize_client(&self, name: &str) -> bool {
        if let Some(idx) = self.inner.clients.iter().position(|(key, _)| key == name) {
            self.inner.preferred.swap(idx, Ordering::Acquire) != idx
        } else {
            false
        }
    }

    pub fn clients(&self) -> impl Iterator<Item = (&str, &ETHDirectClient<PrivateKeySigner>)> {
        let preferred = self.inner.preferred.load(Ordering::Relaxed);
        self.inner
            .clients
            .get(preferred)
            .into_iter()
            .chain(self.inner.clients.get(..preferred).unwrap_or(&[]).iter())
            .chain(
                self.inner
                    .clients
                    .get(1 + preferred..)
                    .unwrap_or(&[])
                    .iter(),
            )
            .map(|(name, client)| (name.as_str(), client))
    }

    pub fn create_contract(
        &self,
        address: Address,
        contract: ethabi::Contract,
    ) -> web3::contract::Contract<Http> {
        let client = self
            .clients()
            .next()
            .expect("Should be at least one client");
        client.1.create_contract(address, contract)
    }

    pub async fn pending_nonce(&self) -> Result<U256, anyhow::Error> {
        multiple_call!(self, pending_nonce());
    }

    pub async fn current_nonce(&self) -> Result<U256, anyhow::Error> {
        multiple_call!(self, current_nonce());
    }

    pub async fn block_number(&self) -> Result<U64, anyhow::Error> {
        multiple_call!(self, block_number());
    }

    pub async fn get_gas_price(&self) -> Result<U256, anyhow::Error> {
        multiple_call!(self, get_gas_price());
    }

    pub async fn sender_eth_balance(&self) -> Result<U256, anyhow::Error> {
        multiple_call!(self, sender_eth_balance());
    }

    pub async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        multiple_call!(self, sign_prepared_tx(data, options));
    }

    pub async fn sign_prepared_tx_for_addr(
        &self,
        data: Vec<u8>,
        contract_addr: H160,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        multiple_call!(
            self,
            sign_prepared_tx_for_addr(data, contract_addr, options)
        );
    }

    pub async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, anyhow::Error> {
        multiple_call!(self, send_raw_tx(tx));
    }

    pub async fn tx_receipt(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, anyhow::Error> {
        multiple_call!(self, tx_receipt(tx_hash));
    }

    pub async fn failure_reason(
        &self,
        tx_hash: H256,
    ) -> Result<Option<FailureInfo>, anyhow::Error> {
        multiple_call!(self, failure_reason(tx_hash));
    }

    pub async fn eth_balance(&self, address: Address) -> Result<U256, anyhow::Error> {
        multiple_call!(self, eth_balance(address));
    }

    pub async fn allowance(
        &self,
        token_address: Address,
        erc20_abi: Contract,
    ) -> Result<U256, anyhow::Error> {
        multiple_call!(self, allowance(token_address, erc20_abi));
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
        A: Into<Option<Address>> + Clone,
        B: Into<Option<BlockId>> + Clone,
        P: Tokenize + Clone,
    {
        multiple_call!(
            self,
            call_contract_function(func, params, from, options, block, token_address, erc20_abi)
        );
    }

    pub async fn call_main_contract_function<R, A, B, P>(
        &self,
        func: &str,
        params: P,
        from: A,
        options: Options,
        block: B,
    ) -> Result<R, anyhow::Error>
    where
        R: Detokenize + Unpin,
        A: Into<Option<Address>> + Clone,
        B: Into<Option<BlockId>> + Clone,
        P: Tokenize + Clone,
    {
        multiple_call!(
            self,
            call_main_contract_function(func, params, from, options, block)
        );
    }

    pub async fn get_tx_status(
        &self,
        hash: H256,
    ) -> Result<Option<ExecutedTxStatus>, anyhow::Error> {
        multiple_call!(self, get_tx_status(hash));
    }

    pub async fn logs(&self, filter: Filter) -> anyhow::Result<Vec<Log>> {
        multiple_call!(self, logs(filter));
    }

    pub fn encode_tx_data<P: Tokenize + Clone>(&self, func: &str, params: P) -> Vec<u8> {
        let (_, client) = self
            .clients()
            .next()
            .expect("Should be at least one client");
        client.encode_tx_data(func, params)
    }

    pub async fn get_tx(&self, hash: H256) -> Result<Option<Transaction>, anyhow::Error> {
        multiple_call!(self, get_tx(hash));
    }
}
