use web3::futures::Future;
use web3::types::{Transaction, TransactionId, H256};

use crate::events::EventData;
use crate::helpers::{DATA_RESTORE_CONFIG, DataRestoreError};

use models::node::operations::{
    TX_TYPE_BYTES_LEGTH, DepositOp, FranklinOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp,
};
use models::node::priority_ops::{Deposit, FranklinPriorityOp, FullExit};
use models::node::tx::{Close, FranklinTx, Transfer, Withdraw};
use models::node::account::{Account, AccountAddress, AccountUpdate};

const FUNC_NAME_HASH_LENGTH: usize = 4;

/// Description of a Franklin operations block
#[derive(Debug, Clone)]
pub struct FranklinOpsBlock {
    /// Franklin operations in block
    pub ops: Vec<FranklinOp>
}

impl FranklinOpsBlock {
    pub fn get_from_event(event_data: &EventData) -> Result<Self, DataRestoreError> {
        let ops_block = FranklinOpsBlock::get_franklin_ops_block(event_data)?;
        Ok(ops_block)
    }

    fn get_franklin_ops_from_data(data: &Vec<u8>) -> Result<Vec<FranklinOp>, DataRestoreError> {
        let mut current_pointer = 0;
        let mut ops = vec![];
        while (current_pointer < data.len()) {
            let op_type: &u8 = &data[current_pointer];

            let chunks = FranklinOp::chunks_by_op_number(op_type)
                .ok_or(DataRestoreError::WrongType)?;
            let full_size: usize = 8 * chunks;

            let pub_data_size = FranklinOp::public_data_length(op_type)
                .ok_or(DataRestoreError::WrongType)?;

            let pre = current_pointer + TX_TYPE_BYTES_LEGTH;
            let post = pre + pub_data_size;

            let op = FranklinOp::from_bytes(&data[pre .. post])
                .ok_or(DataRestoreError::WrongType)?;
            ops.push(op);
            current_pointer += full_size;
        }
        Ok(ops)
    }

    /// Return Franklin operations block description
    ///
    /// # Arguments
    ///
    /// * `event_data` - Franklin Contract event description
    ///
    fn get_franklin_ops_block(event_data: &EventData) -> Result<FranklinOpsBlock, DataRestoreError> {
        let transaction =
            FranklinOpsBlock::get_ethereum_transaction(&event_data.transaction_hash)?;
        let commitment_data = FranklinOpsBlock::get_commitment_data_from_ethereum_transaction(&transaction)?;
        let ops = FranklinOpsBlock::get_franklin_ops_from_data(&commitment_data)?;
        let block = FranklinOpsBlock {
            ops,
        };
        Ok(block)
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
            .map_err(|e| DataRestoreError::Unknown(e.to_string()))?
            .ok_or(DataRestoreError::NoData("No tx by this hash".to_string()))?;
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
        if input_data.len() > FUNC_NAME_HASH_LENGTH {
            return Ok(input_data[FUNC_NAME_HASH_LENGTH..input_data.len()].to_vec())
        } else {
            return Err(DataRestoreError::NoData("No commitment data in tx".to_string()))
        }
    }
}

