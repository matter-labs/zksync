use std::env;
use std::str::FromStr;

use rustc_hex::{FromHex};

use web3::futures::{Future};
use web3::contract::{Contract, Options, CallFuture};
use web3::types::{Address, U256, H160, H256, U128, Bytes};
use web3::transports::{EventLoopHandle, Http};

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

const CONTRACT_ADDR: &str = "CONTRACT_ADDR";
const PRIVATE_KEY: &str = "PRIVATE_KEY";
const SENDER_ACCOUNT: &str = "SENDER_ACCOUNT";
const INFURA_PROJECT_ID: &str = "INFURA_PROJECT_ID";

struct Local {
    event_loop: EventLoopHandle,
    web3:       web3::Web3<Http>,
    contract:   Contract<Http>,
    my_account: Address,
}

struct Remote {
    infura:         Infura,
    private_key:    H256,
    contract_addr:  String,
    sender_account: String,
    contract:       ethabi::Contract,
}

enum Mode {
    Local(Local),
    Remote(Remote),
}

pub struct ETHClient {
    mode: Mode
}

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

        Self::new_remote(contract_abi)
    }

    fn new_local(contract_abi: ABI) -> Self {

        let (event_loop, transport) = Http::new("http://localhost:8545").unwrap();
        let web3 = web3::Web3::new(transport);

        let accounts = web3.eth().accounts().wait().unwrap();
        let my_account = accounts[0];

        let contract = if let Ok(addr) = env::var(CONTRACT_ADDR) {
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
 
        ETHClient{ mode: Mode::Local(Local{event_loop, web3, contract, my_account}) }
    }

    fn new_remote(contract_abi: ABI) -> Self {
        
        let sender_account = env::var(SENDER_ACCOUNT).expect("`SENDER_ACCOUNT` env var must be set");
        let contract_addr = env::var(CONTRACT_ADDR).expect("`CONTRACT_ADD` env var must be set");
        let proj_id =       env::var(INFURA_PROJECT_ID).expect("`INFURA_PROJECT_ID` env var must be set");
        let private_key =   env::var(PRIVATE_KEY).expect("`PRIVATE_KEY` env var must be set");

        let infura = Infura::new(&proj_id);

        ETHClient{
            mode: Mode::Remote(Remote{
                infura,
                private_key:    H256::from_str(&private_key).unwrap(),
                contract_addr,
                sender_account,
                contract:       ethabi::Contract::load(contract_abi.0).unwrap()
            })
        }
    }

    fn call<P: Tokenize>(&self, method: &str, params: P) {
        match &self.mode {
            Mode::Local(_) => unimplemented!(),
            Mode::Remote(s) => {
                let f = s.contract.function(method).unwrap();
                let data = f.encode_input( &params.into_tokens() ).unwrap();

                let nonce = s.infura.get_nonce(&format!("0x{}", &s.sender_account)).unwrap();
                //println!("using nonce {} for {}", nonce, &s.contract_addr);

                let tx = RawTransaction{
                    nonce,
                    to:         Some(H160::from_str(&s.contract_addr).unwrap()),
                    value:      U256::zero(),
                    gas_price:  U256::from_dec_str("9000000000").unwrap(),
                    gas:        U256::from_dec_str("3000000").unwrap(),
                    data:       data,
                };

                let signed = format!("0x{}", hex::encode(tx.sign(&s.private_key)));

                //println!("\ndata: {:?}", hex::encode(&tx.data));
                //println!("\nsigned: {}", signed);

                let tx_hash = s.infura.send_raw_tx(&signed);

                println!("submitted tx: {:?}", tx_hash);
            }
        }
    }

    /// Returns tx hash
    pub fn commit_block(
        &self, 
        block_num: U32, 
        total_fees: U128, 
        tx_data_packed: Vec<u8>, 
        new_root: H256) /*-> impl Future<Item = H256, Error = web3::contract::Error>*/
    {
        match self.mode {
            Mode::Local(_) => unimplemented!(),
            Mode::Remote(_) => self.call("commitBlock", (block_num, total_fees, tx_data_packed, new_root)),
        }

        // self.contract
        //     .call("commitBlock", 
        //         (block_num, total_fees, tx_data_packed, new_root), 
        //         self.my_account, 
        //         Options::with(|opt| {
        //             opt.gas = Some(3000_000.into())
        //         }))
        //     .then(|tx| {
        //         println!("got tx: {:?}", tx);
        //         tx
        //     })
    }

    // /// Returns tx hash
    // pub fn verify_block(
    //     &self, 
    //     block_num: U32, 
    //     proof: [U256; 8]) -> impl Future<Item = H256, Error = web3::contract::Error>
    // {
    //     self.contract
    //         .call("verifyBlock", 
    //             (block_num, proof), 
    //             self.my_account, 
    //             Options::with(|opt| {
    //                 opt.gas = Some(3000_000.into())
    //             }))
    //         .then(|tx| {
    //             println!("got tx: {:?}", tx);
    //             tx
    //         })
    // }

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
    client.commit_block(block_num, total_fees, tx_data_packed, new_root);
    //assert!(client.commit_block(block_num, total_fees, tx_data_packed, new_root).wait().is_ok());

    // println!("verifying block...");
    // assert!(client.verify_block(block_num, proof).wait().is_ok());
}

#[test]
fn test_sign() {

    let client = ETHClient::new(TEST_PLASMA_ALWAYS_VERIFY);
    //client.sign();

}

//use ethereum_tx_sign::RawTransaction;

use web3::contract::tokens::Tokenize;

#[test]
fn test_abi() {
    
    let proj_id = env::var("INFURA_PROJECT_ID").expect("`INFURA_PROJECT_ID` env var must be set");
    let infura = Infura::new(proj_id.as_str());

    let c = ethabi::Contract::load(TEST_PLASMA_ALWAYS_VERIFY.0).unwrap();
    let f = c.function("commitBlock").unwrap();
    
    let block_num: U32 = 0;
    let total_fees: U128 = U128::from_dec_str("200").unwrap();
    let tx_data_packed: Vec<u8> = vec![];
    let new_root: H256 = H256::from_str("a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0").unwrap();

    let data = f.encode_input( &(block_num, total_fees, tx_data_packed, new_root).into_tokens() ).unwrap();

    let nonce = infura.get_nonce("0xb4aaffeaacb27098d9545a3c0e36924af9eedfe0").unwrap();

    let tx = RawTransaction{
        nonce:      U256::from(nonce),
        to:         Some(H160::zero()),
        value:      U256::zero(),
        gas_price:  U256::from_dec_str("9000000000").unwrap(),
        gas:        U256::from_dec_str("3000000").unwrap(),
        data:       hex::decode(TEST_PLASMA_ALWAYS_VERIFY.1).unwrap(),
    };

    let pk = env::var("PRIVATE_KEY").expect("`PRIVATE_KEY` env var expected");
    let pkey = H256::from_str(&pk).unwrap();
    let signed = format!("0x{}", hex::encode(tx.sign(&pkey)));

    println!("\ndata: {:?}", hex::encode(&tx.data));
    //println!("tx: {:?}", hex::encode(&tx));
    println!("\nsigned: {}", signed);

    // let r = infura.get_nonce("0xc94770007dda54cF92009BFF0dE90c06F603a09f");
    // assert!(r.is_ok());
    // assert_eq!(r.unwrap(), U256::from(147)); // TODO: pick a stable address

    let tx_hash = infura.send_raw_tx(&signed);
        //"0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675");
    println!("submitted tx: {:#?}", tx_hash);


    // let (event_loop, transport) = Http::new("http://localhost:8545").unwrap();
    // let web3 = web3::Web3::new(transport);

    // web3.eth().send_raw_transaction(Bytes::from(sig)).then(|r| {
    //     println!("{:#?}", r); 
    //     r
    // }).wait();
}

use reqwest::header::{CONTENT_TYPE};
use std::collections::HashMap;
use serde::Serialize;
use serde::de::DeserializeOwned;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

#[derive(Serialize, Debug)]
struct InfuraRequest<'a, P: Serialize> {
    jsonrpc:    &'a str,
    method:     &'a str,
    params:     &'a P,
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

struct Infura {
    url: String,
}

impl Infura {

    pub fn new(project_id: &str) -> Self {
        // TODO: parametrize network
        Self{url: format!(r#"https://rinkeby.infura.io/v3/{}"#, project_id)}
    }

    fn post<P: Serialize>(&self, method: &str, params: &P) -> Result<String>
    {
        let client = reqwest::Client::new();

        let request = InfuraRequest {
            jsonrpc:    "2.0",
            id:         1,
            method,
            params,
        };

        let response: Result<InfuraResponse> = client.post(self.url.as_str())
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
        let empty: [u8; 0] = [];
        let result = self.post("eth_gasPrice", &empty)?;

        // TODO: code below is ugly, find or implement "0x" strings parser
        if !result.starts_with("0x") { return Err(From::from("invalid result")) }
        Ok(U256::from_str(&result.as_str()[2..])?)
    }

    /// Get nonce for an address
    pub fn get_nonce(&self, addr: &str) -> Result<U256> {
        let result = self.post("eth_getTransactionCount", &[addr, "latest"])?;

        // TODO: code below is ugly, find or implement "0x" strings parser
        if !result.starts_with("0x") { return Err(From::from("invalid result")) }
        Ok(U256::from_str(&result.as_str()[2..])?)
    }

    pub fn send_raw_tx(&self, tx: &str) -> Result<H256> {
        let result = self.post("eth_sendRawTransaction", &[tx])?;

        //println!("{:?}", result);

        // TODO: code below is ugly, find or implement "0x" strings parser
        if !result.starts_with("0x") { return Err(From::from("invalid result")) }
        //println!("{}", result);
        Ok(H256::from_str(&result.as_str()[2..])?)
    }
}


#[test]
fn test_infura() {
    let proj_id = env::var("INFURA_PROJECT_ID").expect("`INFURA_PROJECT_ID` env var must be set");
    let infura = Infura::new(proj_id.as_str());

    let nonce = infura.get_nonce("0xb4aaffeaacb27098d9545a3c0e36924af9eedfe0");
    println!("nonce: {:#?}", nonce);
    // assert!(r.is_ok());
    // assert_eq!(r.unwrap(), U256::from(147)); // TODO: pick a stable address

    let gas_price = infura.get_gas_price();
    println!("gas price: {:#?}", gas_price);

    let tx_hash = infura.send_raw_tx("0xd46e8dd67c5d32be8d46e8dd67c5d32be8058bb8eb970870f072445675058bb8eb970870f072445675");
    println!("submitted tx: {:#?}", tx_hash);

}


// ============

extern crate rlp;
extern crate tiny_keccak;
extern crate secp256k1;

//use ethereum_types::{H160, H256, U256};
use self::rlp::RlpStream;
use self::tiny_keccak::keccak256;
use self::secp256k1::key::SecretKey;
use self::secp256k1::Message;
use self::secp256k1::Secp256k1;

const CHAIN_ID: u8 = 4;

/// Description of a Transaction, pending or in the chain.
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct RawTransaction {
    /// Nonce
    pub nonce: U256,
    /// Recipient (None when contract creation)
    pub to: Option<H160>,
    /// Transfered value
    pub value: U256,
    /// Gas Price
    #[serde(rename = "gasPrice")]
    pub gas_price: U256,
    /// Gas amount
    pub gas: U256,
    /// Input data
    pub data: Vec<u8>
}

impl RawTransaction {
    /// Signs and returns the RLP-encoded transaction
    pub fn sign(&self, private_key: &H256) -> Vec<u8> {
        let hash = self.hash();
        let sig = ecdsa_sign(&hash, &private_key.0);
        let mut tx = RlpStream::new(); 
        tx.begin_unbounded_list();
        self.encode(&mut tx);
        tx.append(&sig.v); 
        tx.append(&sig.r); 
        tx.append(&sig.s); 
        tx.complete_unbounded_list();
        tx.out()
    }

    fn hash(&self) -> Vec<u8> {
        let mut hash = RlpStream::new(); 
        hash.begin_unbounded_list();
        self.encode(&mut hash);
        hash.append(&mut vec![CHAIN_ID]);
        hash.append(&mut U256::zero());
        hash.append(&mut U256::zero());
        hash.complete_unbounded_list();
        keccak256_hash(&hash.out())
    }

    fn encode(&self, s: &mut RlpStream) {
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas);
        if let Some(ref t) = self.to {
            s.append(t);
        } else {
            s.append(&vec![]);
        }
        s.append(&self.value);
        s.append(&self.data);
    }
}

fn keccak256_hash(bytes: &[u8]) -> Vec<u8> {
    keccak256(bytes).into_iter().cloned().collect()
}

fn ecdsa_sign(hash: &[u8], private_key: &[u8]) -> EcdsaSig {
    let s = Secp256k1::signing_only();
    let msg = Message::from_slice(hash).unwrap();
    let key = SecretKey::from_slice(&s, private_key).unwrap();
    let (v, sig_bytes) = s.sign_recoverable(&msg, &key).serialize_compact(&s);

    println!("V m8 {:?}", v);

    EcdsaSig {
        v: vec![v.to_i32() as u8 + CHAIN_ID * 2 + 35],
        r: sig_bytes[0..32].to_vec(),
        s: sig_bytes[32..64].to_vec(),
    }
}

pub struct EcdsaSig {
    v: Vec<u8>,
    r: Vec<u8>,
    s: Vec<u8>
}

// mod test {

//     #[test]
//     fn test_signs_transaction() {
//         use std::io::Read;
//         use std::fs::File;
//         use ethereum_types::*;
//         use raw_transaction::RawTransaction;
//         use serde_json;

//         #[derive(Deserialize)]
//         struct Signing {
//             signed: Vec<u8>,
//             private_key: H256 
//         }

//         let mut file = File::open("./test/test_txs.json").unwrap();
//         let mut f_string = String::new();
//         file.read_to_string(&mut f_string).unwrap();
//         let txs: Vec<(RawTransaction, Signing)> = serde_json::from_str(&f_string).unwrap();

//         for (tx, signed) in txs.into_iter() {
//             assert_eq!(signed.signed, tx.sign(&signed.private_key));
//         }
//     }
// }

