extern crate ethereum_types;
extern crate ethabi;
extern crate ethkey;

extern crate rustc_hex;
extern crate web3;

use self::web3::futures::{Future, Stream};
use self::web3::contract::{Contract, Options, CallFuture};
use self::web3::types::{Address, U256, H256, U128, Bytes};
use self::web3::transports::{EventLoopHandle, Http};
use self::rustc_hex::FromHex;

// extern crate rustc_serialize;
// extern crate serde;
// extern crate serde_derive;
// extern crate serde_json;

pub struct ETHClient {
    event_loop: EventLoopHandle,
    web3:       web3::Web3<Http>,
    contract:   Contract<Http>,
    account:    Address,
}

pub type U32 = u64; // because missing in web3::types; u64 is fine since only used for tokenization

// all methods are blocking for now
impl ETHClient {

    pub fn new() -> Self {
        // TODO: check env vars to decide local/testnet/live
        Self::new_local()
    }

    fn new_local() -> Self {
        let (event_loop, transport) = Http::new("http://localhost:8545").unwrap();
        let web3 = web3::Web3::new(transport);

        // TODO: deploy

        let contract_address = "664d79b5c0C762c83eBd0d1D1D5B048C0b53Ab58".parse().unwrap();
        let contract = Contract::from_json(
            web3.eth(),
            contract_address,
            include_bytes!("../../contracts/build/bin/contracts_Plasma_sol_Plasma.abi"),
        ).unwrap();

        let accounts = web3.eth().accounts().wait().unwrap();
        let account = accounts[0];
            
        Self{event_loop, web3, contract, account}
    }

    fn new_testnet() -> Self {
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

    pub fn commit_block(&self, block_num: U32, total_fees: U128, tx_data_packed: Vec<u8>, new_root: H256) {

        // let block_num: u64 = 0;
        // let total_fees: U128 = U128::from_dec_str("0").unwrap();
        // let txDataPacked: Vec<u8> = vec![];
        // let newRoot: H256 = H256::zero();

        let call_future = self.contract
            .call("commitBlock", (block_num, total_fees, tx_data_packed, new_root), self.account, Options::default())
            .then(|tx| {
                println!("got tx: {:?}", tx);
                Ok(()) as Result<(), ()>
            });
        call_future.wait().unwrap();
    }

    pub fn prove_block() {

    }
}

#[test]
fn test_web3() {

    let client = ETHClient::new();

    let block_num: u64 = 0;
    let total_fees: U128 = U128::from_dec_str("0").unwrap();
    let tx_data_packed: Vec<u8> = vec![];
    let new_root: H256 = H256::zero();

    println!("here");

    client.commit_block(block_num, total_fees, tx_data_packed, new_root);
}