pub mod signer;

use rustc_hex::{FromHex};
use web3::futures::{Future};
use web3::contract::{Contract, Options, CallFuture};

use reqwest::header::{CONTENT_TYPE};
use std::collections::HashMap;

use web3::contract::tokens::Tokenize;
//use web3::types::{Address, U256, H160, H256, U128};

// used
use std::env;
use std::str::FromStr;

use serde::Serialize;
use serde::de::DeserializeOwned;

use ethereum_types::{Address, U256, H160, H256, U128};

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

pub type U32 = u64; // because missing in web3::types; u64 is fine since only used for tokenization

type ABI = (&'static [u8], &'static str);

pub const TEST_PLASMA_ALWAYS_VERIFY: ABI = (
    include_bytes!("../../../contracts/bin/contracts_Plasma_sol_PlasmaTest.abi"),
    include_str!("../../../contracts/bin/contracts_Plasma_sol_PlasmaTest.bin"),
);

pub const PROD_PLASMA: ABI = (
    include_bytes!("../../../contracts/bin/contracts_Plasma_sol_Plasma.abi"),
    include_str!("../../../contracts/bin/contracts_Plasma_sol_Plasma.bin"),
);

pub struct ETHClient {
    private_key:    H256,
    contract_addr:  H160,
    sender_account: String,
    web3_url:       String,
    contract:       ethabi::Contract,
    reqwest_client: reqwest::Client,       
    chain_id:       u8,
}

fn expect_env(name: &'static str) -> String {
    env::var(name).expect(&format!("`{}` env var must be set", name))
}

/// ETH client for Plasma contract
/// All methods are blocking for now
impl ETHClient {

    pub fn new(contract_abi: ABI) -> Self {
        
        Self{
            web3_url:       env::var("WEB3_URL").unwrap_or("http://localhost:8545".to_string()),
            private_key:    H256::from_str(&expect_env("PRIVATE_KEY")).unwrap(),
            contract_addr:  H160::from_str(&expect_env("CONTRACT_ADDR")).unwrap(),
            sender_account: expect_env("SENDER_ACCOUNT"),
            chain_id:       u8::from_str(&expect_env("CHAIN_ID")).unwrap(),
            contract:       ethabi::Contract::load(contract_abi.0).unwrap(),
            reqwest_client: reqwest::Client::new(),
        }
    }

    fn call<P: Tokenize>(&self, method: &str, params: P) -> Result<H256> {

        let f = self.contract.function(method).unwrap();
        let data = f.encode_input( &params.into_tokens() ).unwrap();

        // fetch current nonce and gas_price
        let gas_price = self.get_gas_price()?;
        let nonce = self.get_nonce(&format!("0x{}", &self.sender_account))?;

        // form and sign tx
        let tx = signer::RawTransaction {
            chain_id:   self.chain_id,
            nonce,
            to:         Some(self.contract_addr.clone()),
            value:      U256::zero(),
            gas_price,
            gas:        U256::from(300_000),
            data:       data,
        };
        let signed = tx.sign(&self.private_key);

        self.send_raw_tx(&format!("0x{}", hex::encode(signed)))
    }

    /// Returns tx hash
    pub fn commit_block(
        &self, 
        block_num: U32, 
        total_fees: U128, 
        tx_data_packed: Vec<u8>, 
        new_root: H256) -> Result<H256>
    {
        self.call("commitBlock", (block_num, total_fees, tx_data_packed, new_root))
    }

    /// Returns tx hash
    pub fn verify_block(
        &self, 
        block_num: U32, 
        proof: [U256; 8]) -> Result<H256>
    {
        self.call("verifyBlock", (block_num, proof))
    }

    fn post(&self, method: &str, params: &[&str]) -> Result<String>
    {
        let request = InfuraRequest {
            id:      1,
            jsonrpc: "2.0",
            method,
            params,
        };

        let response: Result<InfuraResponse> = self.reqwest_client.post(self.web3_url.as_str())
            .header(CONTENT_TYPE, "application/json")
            .json(&request)
            .send()?
            .json()
            .map_err(|e| From::from(e));

        let r = response?;

        if let Some(result) = r.result {
            Ok(result)
        } else {
            Err(r.error
                .map( |e| From::from(e.message) )
                .unwrap_or(From::from("no result in the response body")) )
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
    jsonrpc:    &'a str,
    method:     &'a str,
    params:     &'a [&'a str],
    id:         i64,
}

#[derive(Deserialize, Debug)]
struct InfuraError {
    code:       i64,
    message:    String,
}

#[derive(Deserialize, Debug)]
struct InfuraResponse {
    jsonrpc:    String,
    id:         i64,
    error:      Option<InfuraError>,
    result:     Option<String>,
}

fn from_0x<Out>(s: &str) -> Result<Out> 
    where Out: std::str::FromStr, <Out as std::str::FromStr>::Err: std::error::Error + 'static
{
    if !s.starts_with("0x") { return Err(From::from(format!("invalid format: expected '0x{h}', got {h}", h = s))) }
    Ok(Out::from_str(&s[2..])?)
}

#[test]
fn test_eth() {

    let client = ETHClient::new(TEST_PLASMA_ALWAYS_VERIFY);

    let block_num: u64 = 1;
    let total_fees: U128 = U128::from_dec_str("0").unwrap();
    let tx_data_packed: Vec<u8> = vec![];
    let new_root: H256 = H256::zero();

    let proof: [U256; 8] = [U256::zero(); 8];

    println!("committing block...");
    let r = client.commit_block(block_num, total_fees, tx_data_packed, new_root);
    match r {
        Err(e) => println!("{:#?}", e),
        Ok(hash) => println!("https://rinkeby.etherscan.io/tx/{:?}", hash),
    };

    println!("verifying block...");
    let r = client.verify_block(block_num, proof);
    match r {
        Err(e) => println!("{:#?}", e),
        Ok(hash) => println!("https://rinkeby.etherscan.io/tx/{:?}", hash),
    };
}
