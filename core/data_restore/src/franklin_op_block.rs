use web3::futures::Future;
use web3::types::{Transaction, TransactionId, H256};

use crate::events::EventData;
use crate::helpers::*;

/// Description of a Franklin operations block
#[derive(Debug, Clone)]
pub struct FranklinOpBlock {
    /// Franklin block number that contains transaction
    pub block_number: u32,
    /// Franklin transactions commitment data
    pub commitment_data: Vec<u8>,
}

impl FranklinOpBlock {
    /// Return optional Franklin operations block description
    ///
    /// # Arguments
    ///
    /// * `event_data` - Franklin Contract event description
    ///
    pub fn get_franklin_op_block(event_data: &EventData) -> Option<Self> {
        let transaction =
            FranklinOpBlock::get_ethereum_transaction(&event_data.transaction_hash)?;
        let commitment_data = FranklinOpBlock::get_commitment_data_from_ethereum_transaction(&transaction);
        let this = Self {
            block_number: event_data.block_num,
            commitment_data,
        };
        Some(this)
    }

    /// Return optional Ethereum transaction description
    ///
    /// # Arguments
    ///
    /// * `transaction_hash` - The identifier of the particular Ethereum transaction
    ///
    fn get_ethereum_transaction(
        &transaction_hash: &H256,
    ) -> Option<Transaction> {
        let tx_id = TransactionId::Hash(transaction_hash);
        let (_eloop, transport) =
            web3::transports::Http::new(RUNTIME_CONFIG.web3_endpoint.as_str()).ok()?;
        let web3 = web3::Web3::new(transport);
        let web3_transaction = web3.eth().transaction(tx_id).wait();
        match web3_transaction {
            Ok(tx) => tx,
            Err(_) => None,
        }
    }

    /// Return commitment data from Ethereum transaction input data
    ///
    /// # Arguments
    ///
    /// * `transaction` - Ethereum transaction description
    ///
    fn get_commitment_data_from_ethereum_transaction(transaction: &Transaction) -> Option<Vec<u8>> {
        let input_data = transaction.clone().input.0;
        if input_data.len() > 4 {
            Some(input_data[4..input_data.len()].to_vec())
        } else {
            None
        }
    }
}
