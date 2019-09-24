use web3::futures::Future;
use web3::types::{Transaction, TransactionId, H256};

use crate::events::EventData;
use crate::helpers::{DATA_RESTORE_CONFIG, DataRestoreError};

/// Description of a Franklin operations block
#[derive(Debug, Clone)]
pub struct FranklinOpBlock {
    /// Franklin transactions commitment data
    pub commitment_data: Vec<u8>,
}

impl FranklinOpBlock {
    pub fn get_franklin_ops(event_data: &EventData) -> Result<Vec<FranklinOp>, DataRestoreError> {
        let op_block = get_franklin_op_block(event_data)?;
        let ops = get_franklin_ops(op_block)?;
        Ok(ops)
    }

    /// Return Franklin operations block description
    ///
    /// # Arguments
    ///
    /// * `event_data` - Franklin Contract event description
    ///
    fn get_franklin_op_block(event_data: &EventData) -> Result<Self, DataRestoreError> {
        let transaction =
            FranklinOpBlock::get_ethereum_transaction(&event_data.transaction_hash)?;
        let commitment_data = FranklinOpBlock::get_commitment_data_from_ethereum_transaction(&transaction)?;
        let this = Self {
            commitment_data,
        };
        Ok(this)
    }

    /// Return Ethereum transaction description
    ///
    /// # Arguments
    ///
    /// * `transaction_hash` - The identifier of the particular Ethereum transaction
    ///
    fn get_ethereum_transaction(&transaction_hash: &H256) -> Result<Transaction, DataRestoreError> {
        let tx_id = TransactionId::Hash(transaction_hash);
        let (_eloop, transport) =
            web3::transports::Http::new(DATA_RESTORE_CONFIG.web3_endpoint.as_str())
            .map_err(|_| DataRestoreError::WrongEndpoint)?;
        let web3 = web3::Web3::new(transport);
        let web3_transaction = web3.eth().transaction(tx_id).wait()
            .map_err(|_| DataRestoreError::Unknown(e.to_string()))?;
        Ok(web3_transaction)
    }

    /// Return commitment data from Ethereum transaction input data
    ///
    /// # Arguments
    ///
    /// * `transaction` - Ethereum transaction description
    ///
    fn get_commitment_data_from_ethereum_transaction(transaction: &Transaction) -> Result<Vec<u8>, DataRestoreError> {
        let input_data = transaction.clone().input.0;
        if input_data.len() > 4 {
            Ok(input_data[4..input_data.len()].to_vec())
        } else {
            Err(DataRestoreError::NoData("No commitment data in tx".to_string())
        }
    }
}
