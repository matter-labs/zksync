use web3::futures::Future;
use web3::types::{Transaction, TransactionId, H256};

use crate::events::EventData;
use crate::helpers::*;

/// Franlkin operations blocks types
#[derive(Debug, Copy, Clone)]
pub enum FranklinOpBlockType {
    /// Deposit
    Deposit,
    /// Transfer
    Transfer,
    /// Full exit
    FullExit,
    /// Unknown - error type
    Unknown,
}

/// Description of a Franklin operations block
#[derive(Debug, Clone)]
pub struct FranklinOpBlock {
    /// Franlkin operation block type
    pub franklin_op_block_type: FranklinOpBlockType,
    /// Franklin block number that contains transaction
    pub block_number: u32,
    /// Corresponding Ethereum transaction
    pub ethereum_transaction: Transaction,
    /// Franklin transaction commitment data
    pub commitment_data: Vec<u8>,
}

impl FranklinOpBlock {
    /// Return optional Franklin operations block description
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration of DataRestore driver
    /// * `event_data` - Franklin Contract event description
    ///
    pub fn get_franklin_op_block(event_data: &EventData) -> Option<Self> {
        let transaction =
            FranklinOpBlock::get_ethereum_transaction(config, &event_data.transaction_hash)?;
        let input_data = FranklinOpBlock::get_input_data_from_ethereum_transaction(&transaction);
        let tx_type = FranklinOpBlock::get_franklin_op_block_type(&input_data);
        let commitment_data = FranklinOpBlock::get_commitment_data_from_input_data(&input_data)?;
        let this = Self {
            franklin_op_block_type: tx_type,
            block_number: event_data.block_num,
            ethereum_transaction: transaction,
            commitment_data,
        };
        Some(this)
    }

    /// Return optional Ethereum transaction description
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration of DataRestore driver
    /// * `transaction_hash` - The identifier of the particular Ethereum transaction
    ///
    fn get_ethereum_transaction(
        config: &DataRestoreConfig,
        &transaction_hash: &H256,
    ) -> Option<Transaction> {
        let tx_id = TransactionId::Hash(transaction_hash);
        let (_eloop, transport) =
            web3::transports::Http::new(config.web3_endpoint.as_str()).ok()?;
        let web3 = web3::Web3::new(transport);
        let web3_transaction = web3.eth().transaction(tx_id).wait();
        match web3_transaction {
            Ok(tx) => tx,
            Err(_) => None,
        }
    }

    /// Return Ethereum transaction input data
    ///
    /// # Arguments
    ///
    /// * `transaction` - Ethereum transaction description
    ///
    fn get_input_data_from_ethereum_transaction(transaction: &Transaction) -> Vec<u8> {
        transaction.clone().input.0
    }

    /// Return Optional Franklin block commitment data from Ethereum transaction input data
    ///
    /// # Arguments
    ///
    /// * `input_data` - the slice of input data
    ///
    fn get_commitment_data_from_input_data(input_data: &[u8]) -> Option<Vec<u8>> {
        if input_data.len() > 4 {
            Some(input_data[4..input_data.len()].to_vec())
        } else {
            None
        }
    }

    /// Return Franklin operations block type from Ethereum transaction input data
    ///
    /// # Arguments
    ///
    /// * `input_data` - the slice of input data
    ///
    fn get_franklin_op_block_type(input_data: &[u8]) -> FranklinOpBlockType {
        if input_data.len() <= 4 {
            return FranklinOpBlockType::Unknown;
        }
        let deposit_method_bytes: Vec<u8> = vec![83, 61, 227, 10];
        let transaction_method_bytes: Vec<u8> = vec![244, 135, 178, 142];
        let full_exit_method_bytes: Vec<u8> = vec![121, 178, 173, 112];
        let method_bytes: Vec<u8> = input_data[0..4].to_vec();
        match method_bytes {
            _ if method_bytes == deposit_method_bytes => FranklinOpBlockType::Deposit,
            _ if method_bytes == transaction_method_bytes => FranklinOpBlockType::Transfer,
            _ if method_bytes == full_exit_method_bytes => FranklinOpBlockType::FullExit,
            _ => FranklinOpBlockType::Unknown,
        }
    }
}
