extern crate ethereum_types;
extern crate ethabi;
extern crate ethkey;
extern crate web3;

// extern crate rustc_serialize;
// extern crate serde;
// extern crate serde_derive;
// extern crate serde_json;

use self::web3::futures::Future;

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

    println!("Accounts: {:?}", accounts);
}