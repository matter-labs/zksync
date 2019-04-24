use web3::futures::Future;
use web3::types::{H256, Transaction, TransactionId};
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
    ethereum_transaction: Transaction,
    commitment_data: Vec<u8>,
}

impl FranklinTransaction {
    pub fn get_transaction(network: InfuraEndpoint, transaction_hash: &H256) -> Option<Self> {
        let transaction = FranklinTransaction::get_ethereum_transaction(network, transaction_hash)?;
        let input_data = FranklinTransaction::get_input_data_from_ethereum_transaction(&transaction);
        let tx_type = FranklinTransaction::get_transaction_type(&input_data);
        let commitment_data = FranklinTransaction::get_commitment_data_from_input_data(&input_data)?;
        let this = Self {
            network: network,
            franklin_transaction_type: tx_type,
            ethereum_transaction: transaction,
            commitment_data: commitment_data,
        };
        Some(this)
    }

    pub fn get_ethereum_transaction(network: InfuraEndpoint, transaction_hash: &H256) -> Option<Transaction> {
        let infura_endpoint = match network {
            InfuraEndpoint::Mainnet => "https://mainnet.infura.io/",
            InfuraEndpoint::Rinkeby => "https://rinkeby.infura.io/",
        };
        let (_eloop, transport) = web3::transports::Http::new(infura_endpoint).unwrap();
        let web3 = web3::Web3::new(transport);
        let tx_id = TransactionId::Hash(transaction_hash.clone());
        let web3_transaction = web3.eth().transaction(tx_id).wait();
        let tx = match web3_transaction {
            Ok(tx) => {
                println!("Transaction: {:?}", tx);
                tx
            },
            Err(e) => { 
                println!("Error: {:?}", e);
                None
            }
        };
        tx
    }

    pub fn get_input_data_from_ethereum_transaction(transaction: &Transaction) -> Vec<u8> {
        transaction.clone().input.0
    }

    pub fn get_commitment_data_from_input_data(input_data: &Vec<u8>) -> Option<Vec<u8>> {
        let input_data_contains_more_than_4_bytes = input_data.len() > 4;
        let commitment_data = match input_data_contains_more_than_4_bytes {
            true => Some(input_data[4..input_data.len()].to_vec()),
            false => None
        };
        commitment_data
    }

    pub fn get_transaction_type(input_data: &Vec<u8>) -> FranklinTransactionType {
        // let input = get_input_data_from_ethereum_transaction(transaction);
        // let input: Vec<u8> = vec![83, 61, 227, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 29, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 111, 25, 7, 2, 102, 53, 88, 204, 20, 77, 118, 217, 147, 179, 61, 64, 248, 27, 208, 31, 152, 123, 137, 105, 34, 72, 1, 186, 163, 53, 146, 121, 0];
        // let input: Vec<u8> = vec![244, 135, 178, 142, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 112, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 12, 225, 171, 26, 194, 18, 154, 106, 57, 17, 50, 20, 0, 143, 200, 115, 52, 252, 254, 163, 118, 215, 231, 75, 25, 205, 159, 236, 202, 168, 30, 197, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 72, 0, 0, 29, 0, 0, 0, 0, 190, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        // let input: Vec<u8> = vec![121, 178, 173, 112, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 114, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 160, 27, 77, 28, 230, 189, 228, 217, 217, 20, 204, 150, 46, 116, 204, 199, 133, 189, 236, 143, 91, 190, 250, 122, 84, 170, 0, 134, 31, 208, 112, 23, 37, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 19, 0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42, 248, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let input_data_contains_more_than_4_bytes = input_data.len() > 4;
        if input_data_contains_more_than_4_bytes == false {
            return FranklinTransactionType::Unknown
        }
        let deposit_method_bytes: Vec<u8> = vec![83, 61, 227, 10];
        let transaction_method_bytes: Vec<u8> = vec![244, 135, 178, 142];
        let exit_method_bytes: Vec<u8> = vec![121, 178, 173, 112];
        let method_bytes: Vec<u8> = input_data[0..4].to_vec();
        let method_type = match method_bytes {
            _ if method_bytes == deposit_method_bytes => {
                println!("Deposit");
                FranklinTransactionType::Deposit
            },
            _ if method_bytes == transaction_method_bytes => {
                println!("Transaction");
                FranklinTransactionType::Transaction
            },
            _ if method_bytes == exit_method_bytes => {
                println!("Full Exit");
                FranklinTransactionType::Exit
            },
            _ => {
                println!("Unknown");
                FranklinTransactionType::Unknown
            }
        };
        method_type
    }
}
