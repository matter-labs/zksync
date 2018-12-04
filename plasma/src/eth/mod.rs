extern crate ethereum_types;
extern crate ethabi;
extern crate ethkey;

extern crate rustc_hex;
extern crate web3;

use self::web3::futures::{Future, Stream};
use self::web3::contract::{Contract, Options, CallFuture};
use self::web3::types::{Address, U256, H256};
use self::rustc_hex::FromHex;

// extern crate rustc_serialize;
// extern crate serde;
// extern crate serde_derive;
// extern crate serde_json;

pub struct ETHNode {

}

impl ETHNode {

    pub fn new() -> Self {
        ETHNode{}
    }

    pub fn commit_block() {

    }

    pub fn prove_block() {

    }
}

#[test]
fn test_web3() {
    let (_eloop, transport) = web3::transports::Http::new("http://localhost:8545").unwrap();
    let web3 = web3::Web3::new(transport);
    let accounts = web3.eth().accounts().wait().unwrap();

    //println!("Accounts: {:?}", accounts);

    let contract_address = "b036057AC77eBb41DCE9751796Fbaf72dCC83FdF".parse().unwrap();
    let contract = Contract::from_json(
        web3.eth(),
        contract_address,
        include_bytes!("../../contracts/build/bin/contracts_Plasma_sol_Plasma.abi"),
    ).unwrap();

    //println!("{:?}", contract);

    let r: Result<(), ()> = Ok(());

    let call_future = contract
        .call("test", (), accounts[0], Options::default())
        .then(|tx| {
            println!("got tx: {:?}", tx);
            Ok(()) as Result<(), ()>
        });
    call_future.wait();
}