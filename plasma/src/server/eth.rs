use std::env;
use std::str::FromStr;

use rustc_hex::{FromHex};

use web3::futures::{Future};
use web3::contract::{Contract, Options, CallFuture};
use web3::types::{Address, U256, H160, H256, U128, Bytes};
use web3::transports::{EventLoopHandle, Http};

pub struct ETHClient {
    event_loop: EventLoopHandle,
    web3:       web3::Web3<Http>,
    contract:   Contract<Http>,
    my_account: Address,
}

pub type U32 = u64; // because missing in web3::types; u64 is fine since only used for tokenization

type ABI = (&'static [u8], &'static str);

pub const TEST_PLASMA_ALWAYS_VERIFY: ABI = (
    include_bytes!("../../contracts/bin/contracts_Plasma_sol_PlasmaTest.abi"),
    include_str!("../../contracts/bin/contracts_Plasma_sol_PlasmaTest.bin"),
);

pub const PROD_PLASMA: ABI = (
    include_bytes!("../../contracts/bin/contracts_Plasma_sol_Plasma.abi"),
    include_str!("../../contracts/bin/contracts_Plasma_sol_Plasma.bin"),
);

// enum Mode {
//     Infura(usize),
//     Local
// }

// all methods are blocking and panic on error for now
impl ETHClient {

    pub fn new(contract_abi: ABI) -> Self {

        // let mode = match env::var("ETH_NETWORK") {
        //     Ok(ref net) if net == "mainnet" => Mode::Infura(1),
        //     Ok(ref net) if net == "rinkeby" => Mode::Infura(4),
        //     Ok(ref net) if net == "ropsten" => Mode::Infura(43),
        //     Ok(ref net) if net == "kovan"   => Mode::Infura(42),
        //     _ => Mode::Local,
        // };
        // match mode {
        //     Mode::Local => Self::new_local(contract_abi),
        //     Mode::Infura(_) => Self::new_infura(contract_abi),
        // }

        Self::new_local(contract_abi)
    }

    fn new_local(contract_abi: ABI) -> Self {

        let (event_loop, transport) = Http::new("http://localhost:8545").unwrap();
        let web3 = web3::Web3::new(transport);

        let accounts = web3.eth().accounts().wait().unwrap();
        let my_account = accounts[0];

        let contract = if let Ok(addr) = env::var("CONTRACT_ADDR") {
             let contract_address = addr.parse().unwrap();
             Contract::from_json(
                 web3.eth(),
                 contract_address,
                 contract_abi.0,
             ).unwrap()
        } else {
            // Get the contract bytecode for instance from Solidity compiler
            let bytecode: Vec<u8> = contract_abi.1.from_hex().unwrap();

            // Deploying a contract
            Contract::deploy(web3.eth(), contract_abi.0)
                .unwrap()
                .confirmations(0)
                .options(Options::with(|opt| {
                    opt.gas = Some(6000_000.into())
                }))
                .execute(bytecode, (), my_account,
                )
                .expect("Correct parameters are passed to the constructor.")
                .wait()
                .unwrap()
        };

        //println!("contract: {:?}", contract);
 
        ETHClient{event_loop, web3, contract, my_account}
    }

    fn new_infura(contract_abi: ABI) -> Self {

        unimplemented!()

        // TODO: change to infura
        // let (_eloop, transport) = web3::transports::Http::new("http://localhost:8545").unwrap();
        // let web3 = web3::Web3::new(transport);

        // TODO: read contract addr from env var
        // let contract_address = "664d79b5c0C762c83eBd0d1D1D5B048C0b53Ab58".parse().unwrap();
        // let contract = Contract::from_json(
        //     web3.eth(),
        //     contract_address,
        //     include_bytes!("../../contracts/build/bin/contracts_Plasma_sol_Plasma.abi"),
        // ).unwrap();

        // TODO: read account and secret from env var
    }

    /// Returns tx hash
    pub fn commit_block(
        &self, 
        block_num: U32, 
        total_fees: U128, 
        tx_data_packed: Vec<u8>, 
        new_root: H256) -> impl Future<Item = H256, Error = web3::contract::Error>
    {
        self.contract
            .call("commitBlock", 
                (block_num, total_fees, tx_data_packed, new_root), 
                self.my_account, 
                Options::with(|opt| {
                    opt.gas = Some(3000_000.into())
                }))
            .then(|tx| {
                println!("got tx: {:?}", tx);
                tx
            })
    }

    /// Returns tx hash
    pub fn verify_block(
        &self, 
        block_num: U32, 
        proof: [U256; 8]) -> impl Future<Item = H256, Error = web3::contract::Error>
    {
        self.contract
            .call("verifyBlock", 
                (block_num, proof), 
                self.my_account, 
                Options::with(|opt| {
                    opt.gas = Some(3000_000.into())
                }))
            .then(|tx| {
                println!("got tx: {:?}", tx);
                tx
            })
    }

    pub fn sign(&self) {

        //self.web3.eth().
    }
}

#[test]
fn test_web3() {

    let client = ETHClient::new(TEST_PLASMA_ALWAYS_VERIFY);

    let block_num: u64 = 0;
    let total_fees: U128 = U128::from_dec_str("0").unwrap();
    let tx_data_packed: Vec<u8> = vec![];
    let new_root: H256 = H256::zero();

    let proof: [U256; 8] = [U256::zero(); 8];

    println!("committing block...");
    assert!(client.commit_block(block_num, total_fees, tx_data_packed, new_root).wait().is_ok());
    println!("verifying block...");
    assert!(client.verify_block(block_num, proof).wait().is_ok());
}

#[test]
fn test_sign() {

    let client = ETHClient::new(TEST_PLASMA_ALWAYS_VERIFY);
    client.sign();

}

use ethereum_tx_sign::RawTransaction;

use web3::contract::tokens::Tokenize;

#[test]
fn test_abi() {
    let c = ethabi::Contract::load(TEST_PLASMA_ALWAYS_VERIFY.0).unwrap();
    let f = c.function("commitBlock").unwrap();
    
    let block_num: U32 = 0;
    let total_fees: U128 = U128::from_dec_str("200").unwrap();
    let tx_data_packed: Vec<u8> = vec![];
    let new_root: H256 = H256::from_str("a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0").unwrap();

    let data = f.encode_input( &(block_num, total_fees, tx_data_packed, new_root).into_tokens() ).unwrap();

    let tx = RawTransaction{
        nonce:      U256::from_dec_str("3").unwrap(),
        to:         Some(H160::from_str("78630527A240340Ce9ba25f0e3CBA815afB4D138").unwrap()),
        value:      U256::from_dec_str("0").unwrap(),
        gas_price:  U256::from_dec_str("9000000000").unwrap(),
        gas:        U256::from_dec_str("1000000").unwrap(),
        data:       data.clone(),
    };

    let pkey = H256::from_str("90fc60c0a06f4fc50153f240f8715a72cbb9e92465c40aee843e0faceba92136").unwrap();
    let sig = tx.sign(&pkey);

    println!("data: {:?}", data);
    println!("tx: {:?}", tx);
    println!("sig: {:?}", hex::encode(&sig));

    let (event_loop, transport) = Http::new("http://localhost:8545").unwrap();
    let web3 = web3::Web3::new(transport);

    web3.eth().send_raw_transaction(Bytes::from(sig)).then(|r| {
        println!("{:#?}", r); 
        r
    }).wait();
}

use reqwest::header::{CONTENT_TYPE};
use std::collections::HashMap;
//use serde::{Serialize, DeserializeOwned};
use serde::de::DeserializeOwned;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

#[derive(Serialize, Debug)]
struct InfuraRequest<'a> {
    jsonrpc:    &'a str,
    method:     &'a str,
    params:     &'a[&'a str],
    id:         &'a str,
}

struct Infura {
    url: String,
}

impl Infura {

    pub fn new(project_id: &str) -> Self {
        Self{url: format!(r#"https://mainnet.infura.io/v3/{}"#, project_id)}
    }

    fn post<O>(&self, method: &str, params: &[&str]) -> Result<O> 
        where O: DeserializeOwned
    {
        let client = reqwest::Client::new();

        let request = InfuraRequest {
            jsonrpc:    "2.0",
            id:         "1",
            method,
            params,
        };

        client.post(self.url.as_str())
            .header(CONTENT_TYPE, "application/json")
            .json(&request)
            .send()?
            .json()
            .map_err(|e| From::from(e))
    }

    pub fn getTransactionCount(&self, addr: &str) -> Result<u32> {
        let params = vec!(addr, "latest");
        let response: HashMap<String, String> = self.post("eth_getTransactionCount", params.as_slice())?;
        let result = response.get("result").ok_or("no result")?;
        // TODO: code below is ugly, find or implement "0x" strings parser
        if !result.starts_with("0x") { return Err(From::from("invalid result")) }
        Ok(u32::from_str_radix(&result.as_str()[2..], 16)?)
    }
}

// fn query_params() -> Result<()> {

//     let infura_id = env::var("INFURA_PROJECT_ID").expect("`INFURA_PROJECT_ID` env var must be set");

//     let map = InfuraRequest {
//         jsonrpc:    "2.0",
//         method:     "eth_getTransactionCount",
//         params:     vec!("0xc94770007dda54cF92009BFF0dE90c06F603a09f", "latest").as_sl,
//         id:         "1",
//     };

//     // let mut map = HashMap::new();
//     // map.insert("jsonrpc", "2.0");
//     // map.insert("method", "eth_getTransactionCount");
//     // map.insert("params", vec!("0xc94770007dda54cF92009BFF0dE90c06F603a09f", "latest"));
//     // map.insert("id", "1");

//     let client = reqwest::Client::new();
//     let url = format!("https://mainnet.infura.io/v3/{}", infura_id);
//     let res = client.post(url.as_str())
//         .header(CONTENT_TYPE, "application/json")
//         .json(&map)
//         .send()?
//         .json()?;
    
//     let r: HashMap<String, String> = res;

//     let count = u32::from_str_radix(&r["result"].as_str()[2..], 16)?;

//     println!("{:?}, {}", r, count);
//     Ok(())
// }

#[test]
fn test_infura() {


//    let r = query_params();

/*    let body = {
        reqwest::get(format!(r#"curl https://mainnet.infura.io/v3/{} \
            -X POST \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"eth_getTransactionCount","params": ["0xc94770007dda54cF92009BFF0dE90c06F603a09f", "latest"],"id":1}'"#, 
            infura_id)).unwrap()
            .text().unwrap()
    };
*/

    let proj_id = env::var("INFURA_PROJECT_ID").expect("`INFURA_PROJECT_ID` env var must be set");
    let infura = Infura::new(proj_id.as_str());
    let r = infura.getTransactionCount("0xc94770007dda54cF92009BFF0dE90c06F603a09f");
    println!("body = {:?}", r);
}