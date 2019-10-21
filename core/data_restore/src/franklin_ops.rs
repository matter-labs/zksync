use crate::events::EventData;
use crate::helpers::{
    get_ethereum_transaction, get_input_data_from_ethereum_transaction, DataRestoreError,
};
use models::node::operations::{FranklinOp, TX_TYPE_BYTES_LENGTH};
use models::primitives::bytes_slice_to_uint32;

const BLOCK_NUMBER_LENGTH: usize = 32;
const FEE_ACC_LENGTH: usize = 32;
const ROOT_LENGTH: usize = 32;
const EMPTY_LENGTH: usize = 64;

/// Description of a Franklin operations block
#[derive(Debug, Clone)]
pub struct FranklinOpsBlock {
    /// Franklin block number
    pub block_num: u32,
    /// Franklin operations in block
    pub ops: Vec<FranklinOp>,
    /// Fee account
    pub fee_account: u32,
}

impl FranklinOpsBlock {
    // Get ops block from Franklin Contract event description
    pub fn get_from_event(event_data: &EventData) -> Result<Self, DataRestoreError> {
        let ops_block = FranklinOpsBlock::get_franklin_ops_block(event_data)?;
        Ok(ops_block)
    }

    /// Return Franklin operations block description
    ///
    /// # Arguments
    ///
    /// * `event_data` - Franklin Contract event description
    ///
    fn get_franklin_ops_block(
        event_data: &EventData,
    ) -> Result<FranklinOpsBlock, DataRestoreError> {
        let transaction = get_ethereum_transaction(&event_data.transaction_hash)?;
        let input_data = get_input_data_from_ethereum_transaction(&transaction)?;
        let commitment_data = &input_data
            [BLOCK_NUMBER_LENGTH + FEE_ACC_LENGTH + ROOT_LENGTH + EMPTY_LENGTH..input_data.len()];
        let fee_account = FranklinOpsBlock::get_fee_account_from_tx_input(&input_data)?;
        let ops = FranklinOpsBlock::get_franklin_ops_from_data(commitment_data)?;
        let block = FranklinOpsBlock {
            block_num: event_data.block_num,
            ops,
            fee_account: fee_account,
        };
        Ok(block)
    }

    /// Return Franklin operations vector
    ///
    /// # Arguments
    ///
    /// * `data` - Franklin Contract event input data
    ///
    pub fn get_franklin_ops_from_data(data: &[u8]) -> Result<Vec<FranklinOp>, DataRestoreError> {
        let mut current_pointer = 0;
        let mut ops = vec![];
        while current_pointer < data.len() {
            let op_type: &u8 = &data[current_pointer];

            let chunks = FranklinOp::chunks_by_op_number(op_type)
                .ok_or(DataRestoreError::WrongData("Wrong op type".to_string()))?;
            let full_size: usize = 8 * chunks;

            let pub_data_size = FranklinOp::public_data_length(op_type)
                .ok_or(DataRestoreError::WrongData("Wrong op type".to_string()))?;

            let pre = current_pointer + TX_TYPE_BYTES_LENGTH;
            let post = pre + pub_data_size;

            let op = FranklinOp::from_bytes(op_type, &data[pre..post])
                .ok_or(DataRestoreError::WrongData("Wrong data".to_string()))?;
            ops.push(op);
            current_pointer += full_size;
        }
        Ok(ops)
    }

    /// Return fee account from Ethereum transaction input data
    ///
    /// # Arguments
    ///
    /// * `input` - Ethereum transaction input
    ///
    fn get_fee_account_from_tx_input(input_data: &Vec<u8>) -> Result<u32, DataRestoreError> {
        if input_data.len() == BLOCK_NUMBER_LENGTH + FEE_ACC_LENGTH {
            return Ok(bytes_slice_to_uint32(
                &input_data[BLOCK_NUMBER_LENGTH..BLOCK_NUMBER_LENGTH + FEE_ACC_LENGTH],
            )
            .ok_or(DataRestoreError::NoData(
                "Cant convert bytes to fee account number".to_string(),
            ))?);
        } else {
            return Err(DataRestoreError::NoData(
                "No fee account data in tx".to_string(),
            ));
        }
    }
}

#[cfg(test)]
mod test {
    use crate::franklin_ops::FranklinOpsBlock;
    #[test]
    fn test_deposit() {
        let data = "0100000000000000000000000000041336c4e56f98000809101112131415161718192021222334252627000000000000";
        let decoded = hex::decode(data).expect("Decoding failed");
        let ops =
            FranklinOpsBlock::get_franklin_ops_from_data(&decoded).expect("cant get ops from data");
        println!("{:?}", ops);
    }

    #[test]
    fn test_part_exit() {
        let data = "030000000000000000000000000002c68af0bb14000000005711e991397fca8f5651c9bb6fa06b57e4a4dcc000000000";
        let decoded = hex::decode(data).expect("Decoding failed");
        let ops =
            FranklinOpsBlock::get_franklin_ops_from_data(&decoded).expect("cant get ops from data");
        println!("{:?}", ops);
    }

    #[test]
    fn test_full_exit() {
        let data = "06000002000000000000000000000000000000000000000000000000000000000000000052312ad6f01657413b2eae9287f6b9adad93d5fe000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000014cabd42a5b98000000";
        let decoded = hex::decode(data).expect("Decoding failed");
        let ops =
            FranklinOpsBlock::get_franklin_ops_from_data(&decoded).expect("cant get ops from data");
        println!("{:?}", ops);
    }

    #[test]
    fn test_transfer_to_new() {
        let data =
            "02000000000000010008091011121314151617181920212223342526280000010000000000000000";
        let decoded = hex::decode(data).expect("Decoding failed");
        let ops =
            FranklinOpsBlock::get_franklin_ops_from_data(&decoded).expect("cant get ops from data");
        println!("{:?}", ops);
    }

    #[test]
    fn test_transfer() {
        let data = "05000001000000000000010000000000";
        let decoded = hex::decode(data).expect("Decoding failed");
        let ops =
            FranklinOpsBlock::get_franklin_ops_from_data(&decoded).expect("cant get ops from data");
        println!("{:?}", ops);
    }

    #[test]
    fn test_close() {
        let data = "0400000100000000";
        let decoded = hex::decode(data).expect("Decoding failed");
        let ops =
            FranklinOpsBlock::get_franklin_ops_from_data(&decoded).expect("cant get ops from data");
        println!("{:?}", ops);
    }
}
