#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

use futures::Future;
use std::env;
use std::str::FromStr;
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
    min_gas_price: usize,
    pub web3: Web3<T>,
}

pub struct CallResult {
    pub gas_price: U256,
    pub hash: H256,
    pub nonce: U256,
}

impl<T: Transport> ETHClient<T> {
    pub fn new(transport: T, contract_abi: String) -> Self {
        Self {
            private_key: H256::from_str(
                &env::var("OPERATOR_PRIVATE_KEY").expect("OPERATOR_PRIVATE_KEY"),
            )
            .expect("private key must be correct"),
            contract_addr: H160::from_str(
                &env::var("CONTRACT_ADDR")
                    .map(|s| s[2..].to_string())
                    .expect("CONTRACT_ADDR"),
            )
            .expect("contract address must be correct"),
            sender_account: H160::from_str(
                &env::var("OPERATOR_ETH_ADDRESS")
                    .map(|s| s[2..].to_string())
                    .expect("OPERATOR_ETH_ADDRESS"),
            )
            .expect("operator eth address"),
            chain_id: u8::from_str(&env::var("CHAIN_ID").unwrap_or_else(|_| "4".to_string()))
                .expect("chain id must be correct"),
            contract: ethabi::Contract::load(contract_abi.as_bytes())
                .expect("contract must be loaded correctly"),
            gas_price_factor: usize::from_str(
                &env::var("GAS_PRICE_FACTOR").unwrap_or_else(|_| "2".to_string()),
            )
            .expect("GAS_PRICE_FACTOR not set"),
            min_gas_price: usize::from_str(
                &env::var("MIN_GAS_PRICE").unwrap_or_else(|_| "1".to_string()),
            )
            .expect("MIN_GAS_PRICE not set"),
            web3: Web3::new(transport),
        }
    }

    pub fn current_nonce(&self) -> impl Future<Item = U256, Error = Error> {
        self.web3
            .eth()
            .transaction_count(self.sender_account, Some(BlockNumber::Latest))
    }

    pub fn pending_nonce(&self) -> impl Future<Item = U256, Error = Error> {
        self.web3
            .eth()
            .transaction_count(self.sender_account, Some(BlockNumber::Pending))
    }

    pub fn call<P: Tokenize>(
        &mut self,
        func: &str,
        params: P,
        options: Options,
    ) -> impl Future<Item = CallResult, Error = Error> {
        let f = self
            .contract
            .function(func)
            .expect("failed to get function parameters");
        let data = f
            .encode_input(&params.into_tokens())
            .expect("failed to encode parameters");

        // fetch current gas_price
        let mut gas_price = options.gas_price.unwrap_or_else(|| {
            let network_gas_price = self.web3.eth().gas_price().wait().expect("get gas error");
            network_gas_price * U256::from(self.gas_price_factor)
        });
        let min_gas_price = U256::from(self.min_gas_price) * U256::from_str("3B9ACA00").unwrap(); // gwei x 10^9

        let nonce = options
            .nonce
            .unwrap_or_else(|| self.pending_nonce().wait().expect("get nonce error"));

        if gas_price < min_gas_price {
            gas_price = min_gas_price;
        }

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

        let signed = tx.sign(&self.private_key);
        self.web3
            .eth()
            .send_raw_transaction(Bytes(signed))
            .map(move |hash| CallResult {
                nonce,
                gas_price,
                hash,
            })
    }
}
