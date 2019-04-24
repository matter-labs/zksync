use web3::futures::Future;
use web3::types::{H256, TransactionReceipt, Transaction, TransactionId};
use helpers::*;

#[derive(Debug, Copy, Clone)]
pub enum FranklinTransactionType {
    Deposit,
    Transaction,
    Exit,
    Unknown
}

pub struct FranklinTransaction {
    network: InfuraEndpoint,
    franklin_transaction_type: FranklinTransactionType,
    ethereum_transaction: Option<Transaction>,
    ethereum_transaction_receipt: Option<TransactionReceipt>,
    ethereum_transaction_input: Vec<u8>,

}

impl FranklinTransaction {
    pub fn new_empty(on: InfuraEndpoint) -> Self {
        let this = Self {
            network: on,
            franklin_transaction_type: FranklinTransactionType::Unknown,
            ethereum_transaction: None,
            ethereum_transaction_receipt: None,
            ethereum_transaction_input: vec![],
        };
        this
    }

    fn get_ethereum_transaction_receipt(&mut self, transaction_hash: &H256) {
        let infura_endpoint = match self.network.clone() {
            InfuraEndpoint::Mainnet => "https://mainnet.infura.io/",
            InfuraEndpoint::Rinkeby => "https://rinkeby.infura.io/",
        };
        let (_eloop, transport) = web3::transports::Http::new(infura_endpoint).unwrap();
        let web3 = web3::Web3::new(transport);
        let web3_receipt = web3.eth().transaction_receipt(transaction_hash.clone()).wait();
        match web3_receipt {
            Ok(receipt) => {
                println!("Receipt: {:?}", receipt);
                self.ethereum_transaction_receipt = receipt;
            },
            Err(e) => { 
                println!("Error: {:?}", e);
                self.ethereum_transaction_receipt = None;
            }
        };
    }

    fn get_ethereum_transaction(&mut self, transaction_hash: &H256) {
        let infura_endpoint = match self.network.clone() {
            InfuraEndpoint::Mainnet => "https://mainnet.infura.io/",
            InfuraEndpoint::Rinkeby => "https://rinkeby.infura.io/",
        };
        let (_eloop, transport) = web3::transports::Http::new(infura_endpoint).unwrap();
        let web3 = web3::Web3::new(transport);
        let tx_id = TransactionId::Hash(transaction_hash.clone());
        let web3_transaction = web3.eth().transaction(tx_id).wait();
        match web3_transaction {
            Ok(tx) => {
                println!("Transaction: {:?}", tx);
                self.ethereum_transaction = tx;
            },
            Err(e) => { 
                println!("Error: {:?}", e);
                self.ethereum_transaction = None;
            }
        };
    }

    fn get_input_data_from_ethereum_transaction(&mut self) {
        match self.ethereum_transaction.clone() {
            Some(tx) => {
                self.ethereum_transaction_input = tx.input.0;
            },
            None => { 
                println!("No ethereum tx");
                self.ethereum_transaction_input = vec![];
            }
        };
    }

    pub fn check_transaction_type(&mut self) {
        // let input = get_input_data_from_ethereum_transaction(transaction);
        let input: Vec<u8> = vec![83, 61, 227, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 29, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 111, 25, 7, 2, 102, 53, 88, 204, 20, 77, 118, 217, 147, 179, 61, 64, 248, 27, 208, 31, 152, 123, 137, 105, 34, 72, 1, 186, 163, 53, 146, 121, 0];
        // let input: Vec<u8> = vec![244, 135, 178, 142, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 112, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 12, 225, 171, 26, 194, 18, 154, 106, 57, 17, 50, 20, 0, 143, 200, 115, 52, 252, 254, 163, 118, 215, 231, 75, 25, 205, 159, 236, 202, 168, 30, 197, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 72, 0, 0, 29, 0, 0, 0, 0, 190, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        // let input: Vec<u8> = vec![121, 178, 173, 112, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 114, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 160, 27, 77, 28, 230, 189, 228, 217, 217, 20, 204, 150, 46, 116, 204, 199, 133, 189, 236, 143, 91, 190, 250, 122, 84, 170, 0, 134, 31, 208, 112, 23, 37, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 19, 0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42, 248, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let deposit_method_bytes: Vec<u8> = vec![83, 61, 227, 10];
        let transaction_method_bytes: Vec<u8> = vec![244, 135, 178, 142];
        let exit_method_bytes: Vec<u8> = vec![121, 178, 173, 112];
        let method_bytes: Vec<u8> = input[0..4].to_vec();
        match method_bytes {
            _ if method_bytes == deposit_method_bytes => {
                println!("Deposit");
                self.franklin_transaction_type = FranklinTransactionType::Deposit;
            },
            _ if method_bytes == transaction_method_bytes => {
                println!("Transaction");
                self.franklin_transaction_type = FranklinTransactionType::Transaction;
            },
            _ if method_bytes == exit_method_bytes => {
                println!("Full Exit");
                self.franklin_transaction_type = FranklinTransactionType::Exit;
            },
            _ => {
                println!("Unknown");
                self.franklin_transaction_type = FranklinTransactionType::Unknown;
            }
        };
    }
}
