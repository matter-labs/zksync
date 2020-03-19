#[macro_use]
extern crate serde_derive;

use futures::compat::Future01CompatExt;
use web3::contract::tokens::Tokenize;
use web3::contract::Options;
use web3::types::{Address, BlockNumber, Bytes};
use web3::types::{H160, H256, U256};
use web3::{Error, Transport, Web3};

pub mod signer;

pub struct ETHClient<T: Transport> {
    private_key: H256,
    pub sender_account: Address,
    pub contract_addr: H160,
    pub contract: ethabi::Contract,
    pub chain_id: u8,
    pub gas_price_factor: usize,
    pub web3: Web3<T>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignedCallResult {
    pub raw_tx: Vec<u8>,
    pub gas_price: U256,
    pub nonce: U256,
    pub hash: H256,
}

impl<T: Transport> ETHClient<T> {
    pub fn new(
        transport: T,
        contract: ethabi::Contract,
        operator_eth_addr: H160,
        operator_pk: H256,
        contract_eth_addr: H160,
        chain_id: u8,
        gas_price_factor: usize,
    ) -> Self {
        Self {
            sender_account: operator_eth_addr,
            private_key: operator_pk,
            contract_addr: contract_eth_addr,
            chain_id,
            contract,
            gas_price_factor,
            web3: Web3::new(transport),
        }
    }

    /// Returns the next *expected* nonce with respect to the transactions
    /// in the mempool.
    ///
    /// Note that this method may be inconsistent if used with a cluster of nodes
    /// (e.g. `infura`), since the consecutive tx send and attempt to get a pending
    /// nonce may be routed to the different nodes in cluster, and the latter node
    /// may not know about the send tx yet. Thus it is not recommended to rely on this
    /// method as on the trusted source of the latest nonce.  
    pub async fn pending_nonce(&self) -> Result<U256, Error> {
        self.web3
            .eth()
            .transaction_count(self.sender_account, Some(BlockNumber::Pending))
            .compat()
            .await
    }

    /// Returns the account nonce based on the last *mined* block. Not mined transactions
    /// (which are in mempool yet) are not taken into account by this method.
    pub async fn current_nonce(&self) -> Result<U256, Error> {
        self.web3
            .eth()
            .transaction_count(self.sender_account, Some(BlockNumber::Latest))
            .compat()
            .await
    }

    pub async fn block_number(&self) -> Result<U256, Error> {
        self.web3.eth().block_number().compat().await
    }

    pub async fn get_gas_price(&self) -> Result<U256, failure::Error> {
        let mut network_gas_price = self.web3.eth().gas_price().compat().await?;
        network_gas_price *= U256::from(self.gas_price_factor);
        Ok(network_gas_price)
    }

    /// Encodes the transaction data (smart contract method and its input) to the bytes
    /// without creating an actual transaction.
    pub fn encode_tx_data<P: Tokenize>(&self, func: &str, params: P) -> Vec<u8> {
        let f = self
            .contract
            .function(func)
            .expect("failed to get function parameters");
        f.encode_input(&params.into_tokens())
            .expect("failed to encode parameters")
    }

    /// Signs the transaction given the previously encoded data.
    /// Fills in gas/nonce if not supplied inside options.
    pub async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, failure::Error> {
        // fetch current gas_price
        let gas_price = match options.gas_price {
            Some(gas_price) => gas_price,
            None => self.get_gas_price().await?,
        };

        let nonce = match options.nonce {
            Some(nonce) => nonce,
            None => self.pending_nonce().await?,
        };

        // form and sign tx
        let tx = signer::RawTransaction {
            chain_id: self.chain_id,
            nonce,
            to: Some(self.contract_addr),
            value: options.value.unwrap_or_default(),
            gas_price,
            gas: options.gas.unwrap_or_else(|| U256::from(3_000_000)),
            data,
        };

        let signed_tx = tx.sign(&self.private_key);
        let hash = self
            .web3
            .web3()
            .sha3(Bytes(signed_tx.clone()))
            .compat()
            .await?;

        Ok(SignedCallResult {
            raw_tx: signed_tx,
            gas_price,
            nonce,
            hash,
        })
    }

    /// Encodes the transaction data and signs the transaction.
    /// Fills in gas/nonce if not supplied inside options.
    pub async fn sign_call_tx<P: Tokenize>(
        &self,
        func: &str,
        params: P,
        options: Options,
    ) -> Result<SignedCallResult, failure::Error> {
        let f = self
            .contract
            .function(func)
            .expect("failed to get function parameters");
        let data = f
            .encode_input(&params.into_tokens())
            .expect("failed to encode parameters");

        self.sign_prepared_tx(data, options).await
    }

    /// Sends the transaction to the Ethereum blockchain.
    /// Transaction is expected to be encoded as the byte sequence.
    pub async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, failure::Error> {
        Ok(self
            .web3
            .eth()
            .send_raw_transaction(Bytes(tx))
            .compat()
            .await?)
    }
}
