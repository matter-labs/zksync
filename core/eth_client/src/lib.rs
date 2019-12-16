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
    contract_addr: H160,
    contract: ethabi::Contract,
    chain_id: u8,
    gas_price_factor: usize,
    pub web3: Web3<T>,
}

#[derive(Debug, Clone)]
pub struct SignedCallResult {
    pub raw_tx: Vec<u8>,
    pub gas_price: U256,
    pub nonce: U256,
    pub hash: H256,
}

impl<T: Transport> ETHClient<T> {
    pub fn new(
        transport: T,
        contract_abi: String,
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
            contract: ethabi::Contract::load(contract_abi.as_bytes())
                .expect("contract must be loaded correctly"),
            gas_price_factor,
            web3: Web3::new(transport),
        }
    }

    pub async fn current_nonce(&self) -> Result<U256, Error> {
        self.web3
            .eth()
            .transaction_count(self.sender_account, Some(BlockNumber::Latest))
            .compat()
            .await
    }

    pub async fn pending_nonce(&self) -> Result<U256, Error> {
        self.web3
            .eth()
            .transaction_count(self.sender_account, Some(BlockNumber::Pending))
            .compat()
            .await
    }

    pub async fn get_gas_price(&self) -> Result<U256, failure::Error> {
        let mut network_gas_price = self.web3.eth().gas_price().compat().await?;
        network_gas_price *= U256::from(self.gas_price_factor);
        Ok(network_gas_price)
    }

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
            value: U256::zero(),
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

    pub async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, failure::Error> {
        Ok(self
            .web3
            .eth()
            .send_raw_transaction(Bytes(tx))
            .compat()
            .await?)
    }
}
