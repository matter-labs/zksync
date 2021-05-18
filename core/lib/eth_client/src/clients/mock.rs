use std::sync::Arc;

use anyhow::Error;
use ethabi::{Address, Contract};
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;
use web3::contract::tokens::{Detokenize, Tokenize};
use web3::contract::Options;
use web3::transports::Http;
use web3::types::{BlockId, Filter, Log, Transaction, U64};

use zksync_types::{TransactionReceipt, H160, H256, U256};

use crate::{
    ethereum_gateway::{ExecutedTxStatus, FailureInfo},
    SignedCallResult,
};

#[derive(Debug)]
struct MockEthereumInner {
    block_number: u64,
    gas_price: U256,
    tx_statuses: Arc<RwLock<HashMap<H256, ExecutedTxStatus>>>,
    sent_txs: Arc<RwLock<HashSet<Vec<u8>>>>,
}

/// Mock Ethereum client is capable of recording all the incoming requests for the further analysis.
#[derive(Debug, Default, Clone)]
pub struct MockEthereum {
    inner: Arc<MockEthereumInner>,
}

impl Default for MockEthereumInner {
    fn default() -> Self {
        Self {
            block_number: 1,
            gas_price: 100.into(),
            tx_statuses: Default::default(),
            sent_txs: Default::default(),
        }
    }
}

impl MockEthereum {
    /// A fake `sha256` hasher, which calculates an `std::hash` instead.
    /// This is done for simplicity and it's also much faster.
    pub fn fake_sha256(data: &[u8]) -> H256 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        hasher.write(data);

        let result = hasher.finish();

        H256::from_low_u64_ne(result)
    }

    /// Checks that there was a request to send the provided transaction.
    pub async fn assert_sent(&self, tx: &[u8]) {
        assert!(
            self.inner.sent_txs.read().await.contains(tx),
            "Transaction {:?} was not sent",
            tx
        );
    }

    /// Adds an response for the sent transaction for `ETHSender` to receive.
    pub async fn add_execution(&mut self, hash: &H256, status: &ExecutedTxStatus) {
        self.inner
            .tx_statuses
            .write()
            .await
            .insert(*hash, status.clone());
    }

    /// Increments the blocks by a provided `confirmations` and marks the sent transaction
    /// as a success.
    pub async fn add_successfull_execution(&mut self, tx_hash: H256, confirmations: u64) {
        Arc::get_mut(&mut self.inner).unwrap().block_number += confirmations;

        let status = ExecutedTxStatus {
            confirmations,
            success: true,
            receipt: None,
        };
        self.inner.tx_statuses.write().await.insert(tx_hash, status);
    }

    /// Same as `add_successfull_execution`, but marks the transaction as a failure.
    pub async fn add_failed_execution(&mut self, hash: &H256, confirmations: u64) {
        Arc::get_mut(&mut self.inner).unwrap().block_number += confirmations;

        let status = ExecutedTxStatus {
            confirmations,
            success: false,
            receipt: Some(Default::default()),
        };
        self.inner.tx_statuses.write().await.insert(*hash, status);
    }
    pub async fn get_tx_status(&self, hash: H256) -> anyhow::Result<Option<ExecutedTxStatus>> {
        Ok(self.inner.tx_statuses.read().await.get(&hash).cloned())
    }

    pub async fn block_number(&self) -> anyhow::Result<U64> {
        Ok(self.inner.block_number.into())
    }

    pub async fn set_block_number(&mut self, val: U64) -> anyhow::Result<U64> {
        Arc::get_mut(&mut self.inner).unwrap().block_number = val.as_u64();
        Ok(self.inner.block_number.into())
    }

    pub async fn get_gas_price(&self) -> anyhow::Result<U256> {
        Ok(self.inner.gas_price)
    }

    pub async fn set_gas_price(&mut self, val: U256) -> anyhow::Result<U256> {
        Arc::get_mut(&mut self.inner).unwrap().gas_price = val;
        Ok(self.inner.gas_price)
    }

    pub async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, anyhow::Error> {
        // Cut hash of transaction
        let mut hash: [u8; 32] = Default::default();
        hash.copy_from_slice(&tx[..32]);
        self.inner.sent_txs.write().await.insert(hash.to_vec());
        Ok(H256::from(hash))
    }

    pub async fn sign_prepared_tx(
        &self,
        raw_tx: Vec<u8>,
        options: Options,
    ) -> anyhow::Result<SignedCallResult> {
        let gas_price = options.gas_price.unwrap_or(self.inner.gas_price);
        let nonce = options.nonce.expect("Nonce must be set for every tx");

        // Nonce and gas_price are appended to distinguish the same transactions
        // with different gas by their hash in tests.
        let mut data_for_hash = raw_tx.clone();
        data_for_hash.append(&mut ethabi::encode(gas_price.into_tokens().as_ref()));
        data_for_hash.append(&mut ethabi::encode(nonce.into_tokens().as_ref()));
        let hash = Self::fake_sha256(data_for_hash.as_ref()); // Okay for test purposes.
                                                              // Concatenate raw_tx plus hash for test purposes
        let mut new_raw_tx = hash.as_bytes().to_vec();
        new_raw_tx.extend(raw_tx);
        Ok(SignedCallResult {
            raw_tx: new_raw_tx,
            gas_price,
            nonce,
            hash,
        })
    }

    pub async fn failure_reason(
        &self,
        _tx_hash: H256,
    ) -> Result<Option<FailureInfo>, anyhow::Error> {
        Ok(None)
    }

    pub async fn pending_nonce(&self) -> Result<U256, Error> {
        unreachable!()
    }

    pub async fn current_nonce(&self) -> Result<U256, Error> {
        unreachable!()
    }

    pub async fn sender_eth_balance(&self) -> Result<U256, Error> {
        unreachable!()
    }

    pub async fn sign_prepared_tx_for_addr(
        &self,
        _data: Vec<u8>,
        _contract_addr: H160,
        _options: Options,
    ) -> Result<SignedCallResult, Error> {
        unreachable!()
    }

    pub async fn tx_receipt(&self, _tx_hash: H256) -> Result<Option<TransactionReceipt>, Error> {
        unreachable!()
    }

    pub async fn eth_balance(&self, _address: Address) -> Result<U256, Error> {
        unreachable!()
    }

    pub async fn contract_balance(
        &self,
        _token_address: Address,
        _abi: Contract,
        _address: Address,
    ) -> Result<U256, Error> {
        unreachable!()
    }

    pub async fn allowance(
        &self,
        _token_address: Address,
        _erc20_abi: Contract,
    ) -> Result<U256, Error> {
        unreachable!()
    }
    pub fn contract(&self) -> &Contract {
        unreachable!()
    }

    pub fn encode_tx_data<P: Tokenize>(&self, _func: &str, params: P) -> Vec<u8> {
        ethabi::encode(params.into_tokens().as_ref())
    }

    pub async fn call_main_contract_function<R, A, P, B>(
        &self,
        _func: &str,
        _params: P,
        _from: A,
        _options: Options,
        _block: B,
    ) -> Result<R, anyhow::Error>
    where
        R: Detokenize + Unpin,
        A: Into<Option<Address>>,
        B: Into<Option<BlockId>>,
        P: Tokenize,
    {
        todo!()
    }

    pub async fn logs(&self, _filter: Filter) -> anyhow::Result<Vec<Log>> {
        todo!()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn call_contract_function<R, A, B, P>(
        &self,
        _func: &str,
        _params: P,
        _from: A,
        _options: Options,
        _block: B,
        _token_address: Address,
        _erc20_abi: ethabi::Contract,
    ) -> Result<R, anyhow::Error>
    where
        R: Detokenize + Unpin,
        A: Into<Option<Address>>,
        B: Into<Option<BlockId>>,
        P: Tokenize,
    {
        todo!()
    }

    pub fn create_contract(
        &self,
        _address: Address,
        _contract: ethabi::Contract,
    ) -> web3::contract::Contract<Http> {
        unreachable!()
    }

    pub async fn get_tx(&self, _hash: H256) -> Result<Option<Transaction>, anyhow::Error> {
        unreachable!()
    }
}
