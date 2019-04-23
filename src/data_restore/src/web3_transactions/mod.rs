use web3::futures::Future;
use web3::types::{H256, TransactionReceipt};
use helpers::*;

pub fn get_transaction_receipt(on: InfuraEndpoint, transaction_hash: &H256) -> Option<TransactionReceipt> {
    let infura_endpoint = match on {
        InfuraEndpoint::Mainnet => "https://mainnet.infura.io/",
        InfuraEndpoint::Rinkeby => "https://rinkeby.infura.io/",
    };
    let (_eloop, transport) = web3::transports::Http::new(infura_endpoint).unwrap();
    let web3 = web3::Web3::new(transport);
    let web3_receipt = web3.eth().transaction_receipt(transaction_hash.clone()).wait();
    let result: Option<TransactionReceipt> = match web3_receipt {
        Ok(receipt) => {
            println!("Receipt: {:?}", receipt);
            receipt
        },
        Err(e) => { 
            println!("Error: {:?}", e);
            None
        }
    };
    result
}