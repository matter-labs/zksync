use web3::contract::tokens::Tokenize;
use web3::contract::Options;
use web3::types::U64;
use web3::Error;

use zksync_types::{TransactionReceipt, H160, H256, U256};

// pub struct Error;

#[derive(Debug, Clone, PartialEq)]
pub struct SignedCallResult {
    pub raw_tx: Vec<u8>,
    pub gas_price: U256,
    pub nonce: U256,
    pub hash: H256,
}

#[async_trait::async_trait]
pub trait EthClientInterface {
    /// Returns the next *expected* nonce with respect to the transactions
    /// in the mempool.
    ///
    /// Note that this method may be inconsistent if used with a cluster of nodes
    /// (e.g. `infura`), since the consecutive tx send and attempt to get a pending
    /// nonce may be routed to the different nodes in cluster, and the latter node
    /// may not know about the send tx yet. Thus it is not recommended to rely on this
    /// method as on the trusted source of the latest nonce.  
    async fn pending_nonce(&self) -> Result<U256, Error>;

    /// Returns the account nonce based on the last *mined* block. Not mined transactions
    /// (which are in mempool yet) are not taken into account by this method.
    async fn current_nonce(&self) -> Result<U256, Error>;

    async fn block_number(&self) -> Result<U64, Error>;

    async fn get_gas_price(&self) -> Result<U256, anyhow::Error>;
    /// Returns the account balance.
    async fn balance(&self) -> Result<U256, Error>;

    /// Encodes the transaction data (smart contract method and its input) to the bytes
    /// without creating an actual transaction.
    fn encode_tx_data<P: Tokenize + Send>(&self, func: &str, params: P) -> Vec<u8>;

    /// Signs the transaction given the previously encoded data.
    /// Fills in gas/nonce if not supplied inside options.
    async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error>;

    /// Signs the transaction given the previously encoded data.
    /// Fills in gas/nonce if not supplied inside options.
    async fn sign_prepared_tx_for_addr(
        &self,
        data: Vec<u8>,
        contract_addr: H160,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error>;

    /// Encodes the transaction data and signs the transaction.
    /// Fills in gas/nonce if not supplied inside options.
    async fn sign_call_tx<P: Tokenize + Send>(
        &self,
        func: &str,
        params: P,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error>;
    /// Sends the transaction to the Ethereum blockchain.
    /// Transaction is expected to be encoded as the byte sequence.
    async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, anyhow::Error>;

    /// Gets the Ethereum transaction receipt.
    async fn tx_receipt(&self, tx_hash: H256) -> Result<Option<TransactionReceipt>, anyhow::Error>;
}
