#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

use ethereum_types::{H160, H256, U256};
use hex;
use models::TxMeta;
use reqwest;
use reqwest::header::CONTENT_TYPE;
use std::env;
use std::str::FromStr;
use web3::contract::tokens::Tokenize;

pub mod signer;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub type U32 = u64; // because missing in web3::types; u64 is fine since only used for tokenization

pub struct ETHClient {
    private_key: H256,
    contract_addr: H160,
    sender_account: String,
    web3_url: String,
    contract: ethabi::Contract,
    reqwest_client: reqwest::Client,
    chain_id: u8,
    gas_price_factor: usize,
    min_gas_price: usize,
}

/// ETH client for Plasma contract
/// All methods are blocking for now
impl ETHClient {
    pub fn new(contract_abi: String) -> Self {
        Self {
            web3_url: env::var("WEB3_URL").unwrap_or_else(|_| "http://localhost:8545".to_string()),
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
            sender_account: env::var("OPERATOR_ETH_ADDRESS")
                .map(|s| s[2..].to_string())
                .expect("OPERATOR_ETH_ADDRESS"),
            chain_id: u8::from_str(&env::var("CHAIN_ID").unwrap_or_else(|_| "4".to_string()))
                .expect("chain id must be correct"),
            contract: ethabi::Contract::load(contract_abi.as_bytes())
                .expect("contract must be loaded correctly"),
            reqwest_client: reqwest::Client::new(),
            gas_price_factor: usize::from_str(
                &env::var("GAS_PRICE_FACTOR").unwrap_or_else(|_| "2".to_string()),
            )
            .expect("GAS_PRICE_FACTOR not set"),
            min_gas_price: usize::from_str(
                &env::var("MIN_GAS_PRICE").unwrap_or_else(|_| "1".to_string()),
            )
            .expect("MIN_GAS_PRICE not set"),
        }
    }

    pub fn current_sender(&self) -> String {
        self.sender_account.clone()
    }

    pub fn current_nonce(&self) -> Result<u32> {
        self.get_nonce(&format!("0x{}", self.sender_account))
            .map(|nonce| nonce.as_u32())
    }

    pub fn default_account(&self) -> String {
        format!("0x{}", self.sender_account)
    }

    pub fn call<P: Tokenize>(&mut self, method: &str, meta: TxMeta, params: P) -> Result<H256> {
        let f = self
            .contract
            .function(method)
            .expect("failed to get function parameters");
        let data = f
            .encode_input(&params.into_tokens())
            .expect("failed to encode parameters");

        // fetch current gas_price
        let orig_gas_price = self.get_gas_price()?;
        let mut gas_price = orig_gas_price * U256::from(self.gas_price_factor);
        let min_gas_price = U256::from(self.min_gas_price) * U256::from_str("3B9ACA00").unwrap(); // gwei x 10^9

        if gas_price < min_gas_price {
            gas_price = min_gas_price;
        }
        info!(
            "Sending tx: gas price = {}, min = {}, factored = {}, nonce = {}",
            orig_gas_price, min_gas_price, gas_price, meta.nonce
        );

        // form and sign tx
        let tx = signer::RawTransaction {
            chain_id: self.chain_id,
            nonce: U256::from(meta.nonce),
            to: Some(self.contract_addr),
            value: U256::zero(),
            gas_price,
            gas: U256::from(3_000_000),
            data,
        };

        // TODO: use meta.addr to pick the signing key
        let signed = tx.sign(&self.private_key);
        let raw_tx_hex = format!("0x{}", hex::encode(signed));
        self.send_raw_tx(&raw_tx_hex)
    }

    /// Returns tx hash
    // pub fn commit_block(
    //     & mut self,
    //     block_num: U32,
    //     total_fees: U128,
    //     tx_data_packed: Vec<u8>,
    //     new_root: H256) -> Result<H256>
    // {
    //     self.call("commitBlock", (block_num, total_fees, tx_data_packed, new_root))
    // }

    // /// Returns tx hash
    // pub fn verify_block(
    //     & mut self,
    //     block_num: U32,
    //     proof: [U256; 8]) -> Result<H256>
    // {
    //     self.call("verifyBlock", (block_num, proof))
    // }

    fn post(&self, method: &str, params: &[&str]) -> Result<String> {
        let request = InfuraRequest {
            id: 1,
            jsonrpc: "2.0",
            method,
            params,
        };

        let response: Result<InfuraResponse> = self
            .reqwest_client
            .post(self.web3_url.as_str())
            .header(CONTENT_TYPE, "application/json")
            .json(&request)
            .send()?
            .json()
            .map_err(From::from);

        let r = response?;

        if let Some(result) = r.result {
            Ok(result)
        } else {
            Err(r
                .error
                .map(|e| From::from(e.message))
                .unwrap_or_else(|| From::from("no result in the response body")))
        }
    }

    /// Get current gas price
    pub fn get_gas_price(&self) -> Result<U256> {
        from_0x(&self.post("eth_gasPrice", &[])?)
    }

    /// Get nonce for an address
    pub fn get_nonce(&self, addr: &str) -> Result<U256> {
        from_0x(&self.post("eth_getTransactionCount", &[addr, "latest"])?)
    }

    pub fn send_raw_tx(&self, tx: &str) -> Result<H256> {
        from_0x(&self.post("eth_sendRawTransaction", &[tx])?)
    }
}

#[derive(Serialize, Debug)]
struct InfuraRequest<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    params: &'a [&'a str],
    id: i64,
}

#[derive(Deserialize, Debug)]
struct InfuraError {
    code: i64,
    message: String,
}

#[derive(Deserialize, Debug)]
struct InfuraResponse {
    jsonrpc: String,
    id: i64,
    error: Option<InfuraError>,
    result: Option<String>,
}

fn from_0x<Out>(s: &str) -> Result<Out>
where
    Out: std::str::FromStr,
    <Out as std::str::FromStr>::Err: std::error::Error + 'static,
{
    if !s.starts_with("0x") {
        return Err(From::from(format!(
            "invalid format: expected '0x{h}', got {h}",
            h = s
        )));
    }
    Ok(Out::from_str(&s[2..])?)
}
