mod signer;

use reqwest::header::{CONTENT_TYPE};

use web3::contract::tokens::Tokenize;
//use web3::types::{Address, U256, H160, H256, U128};

// used
use std::env;
use std::str::FromStr;

use ethereum_types::{U256, H160, H256};

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

pub type U32 = u64; // because missing in web3::types; u64 is fine since only used for tokenization

type ABI = (&'static [u8], &'static str);

pub const TEST_PLASMA_ALWAYS_VERIFY: ABI = (
    include_bytes!("../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.abi"),
    include_str!("../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.bin"),
);

pub const PROD_PLASMA: ABI = (
    include_bytes!("../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.abi"),
    include_str!("../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.bin"),
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMeta{
    pub addr:   String,
    pub nonce:  u32,
}

pub struct ETHClient {
    private_key:    H256,
    contract_addr:  H160,
    sender_account: String,
    web3_url:       String,
    contract:       ethabi::Contract,
    reqwest_client: reqwest::Client,       
    chain_id:       u8,
    nonce:          U256
}

/// ETH client for Plasma contract
/// All methods are blocking for now
impl ETHClient {

    pub fn new(contract_abi: ABI) -> Self {

        let mut this = Self{
            web3_url:       env::var("WEB3_URL").unwrap_or("http://localhost:8545".to_string()),
            private_key:    H256::from_str(&env::var("PRIVATE_KEY").unwrap_or("aa8564af9bef22f581e99125d1829b76c45d08e4f6f0b74d586911f4318b6776".to_string())).unwrap(),
            contract_addr:  H160::from_str(&env::var("CONTRACT_ADDR").unwrap_or("616e08c733fe20e99bf70c5088635694d5e25c54".to_string())).unwrap(),
            sender_account: env::var("SENDER_ACCOUNT").unwrap_or("e5d0efb4756bd5cdd4b5140d3d2e08ca7e6cf644".to_string()),
            chain_id:       u8::from_str(&env::var("CHAIN_ID").unwrap_or("4".to_string())).unwrap(),
            contract:       ethabi::Contract::load(contract_abi.0).unwrap(),
            reqwest_client: reqwest::Client::new(),
            nonce:          U256::zero(),
        };

        // TODO: review nonce handling
        this.nonce = this.get_nonce(&format!("0x{}", &this.sender_account)).unwrap();
        println!("Starting with nonce = {}", this.nonce);

        this
    }

    pub fn default_account(&self) -> String {
        format!("0x{}", self.sender_account)
    }

    pub fn call<P: Tokenize>(&mut self, method: &str, _meta: TxMeta, params: P) -> Result<H256> {

        let f = self.contract.function(method).unwrap();
        let data = f.encode_input( &params.into_tokens() ).unwrap();

        // fetch current nonce and gas_price
        let gas_price = self.get_gas_price()?;
        // let nonce = self.get_nonce(&format!("0x{}", &self.sender_account))?;

        // TODO: use meta instead
        let nonce = self.nonce.clone();
        let mut new_nonce = self.nonce;
        new_nonce = new_nonce + U256::one();
        self.nonce = new_nonce;

        println!("Sending with nonce = {}", nonce);

        // form and sign tx
        let tx = signer::RawTransaction {
            chain_id:   self.chain_id,
            nonce,
            to:         Some(self.contract_addr.clone()),
            value:      U256::zero(),
            gas_price,
            gas:        U256::from(3_000_000),
            data:       data,
        };

        // TODO: use meta to pick the signing key

        let signed = tx.sign(&self.private_key);
        let raw_tx_hex = format!("0x{}", hex::encode(signed));
        // println!("Raw transaction = {}", raw_tx_hex);
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

    // let mut client = ETHClient::new(TEST_PLASMA_ALWAYS_VERIFY);

    // let block_num: u64 = 1;
    // let total_fees: U128 = U128::from_dec_str("0").unwrap();
    // let tx_data_packed: Vec<u8> = vec![];
    // let new_root: H256 = H256::zero();

    // let proof: [U256; 8] = [U256::zero(); 8];

    // println!("committing block...");
    // let r = client.commit_block(block_num, total_fees, tx_data_packed, new_root);
    // match r {
    //     Err(e) => println!("{:#?}", e),
    //     Ok(hash) => println!("https://rinkeby.etherscan.io/tx/{:?}", hash),
    // };

    // println!("verifying block...");
    // let r = client.verify_block(block_num, proof);
    // match r {
    //     Err(e) => println!("{:#?}", e),
    //     Ok(hash) => println!("https://rinkeby.etherscan.io/tx/{:?}", hash),
    // };
}
