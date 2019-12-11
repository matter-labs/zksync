use crate::events::EventData;
use crate::helpers::{get_ethereum_transaction, get_input_data_from_ethereum_transaction};
use failure::{ensure, format_err};
use models::node::operations::FranklinOp;
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
    pub fn get_from_event(event_data: &EventData) -> Result<Self, failure::Error> {
        let ops_block = FranklinOpsBlock::get_franklin_ops_block(event_data)?;
        Ok(ops_block)
    }

    /// Return Franklin operations block description
    ///
    /// # Arguments
    ///
    /// * `event_data` - Franklin Contract event description
    ///
    fn get_franklin_ops_block(event_data: &EventData) -> Result<FranklinOpsBlock, failure::Error> {
        let transaction = get_ethereum_transaction(&event_data.transaction_hash)?;
        let input_data = get_input_data_from_ethereum_transaction(&transaction)?;
        let commitment_data = &input_data
            [BLOCK_NUMBER_LENGTH + FEE_ACC_LENGTH + ROOT_LENGTH + EMPTY_LENGTH..input_data.len()];
        let fee_account = FranklinOpsBlock::get_fee_account_from_tx_input(&input_data)?;
        let ops = FranklinOpsBlock::get_franklin_ops_from_data(commitment_data)?;
        let block = FranklinOpsBlock {
            block_num: event_data.block_num,
            ops,
            fee_account,
        };
        Ok(block)
    }

    /// Return Franklin operations vector
    ///
    /// # Arguments
    ///
    /// * `data` - Franklin Contract event input data
    ///
    pub fn get_franklin_ops_from_data(data: &[u8]) -> Result<Vec<FranklinOp>, failure::Error> {
        let mut current_pointer = 0;
        let mut ops = vec![];
        while current_pointer < data.len() {
            let op_type: u8 = data[current_pointer];

            let pub_data_size = FranklinOp::public_data_length(op_type)?;

            let pre = current_pointer;
            let post = pre + pub_data_size;

            let op = FranklinOp::from_public_data(&data[pre..post])?;
            ops.push(op);
            current_pointer += pub_data_size;
        }
        Ok(ops)
    }

    /// Return fee account from Ethereum transaction input data
    ///
    /// # Arguments
    ///
    /// * `input` - Ethereum transaction input
    ///
    fn get_fee_account_from_tx_input(input_data: &[u8]) -> Result<u32, failure::Error> {
        ensure!(input_data.len() == BLOCK_NUMBER_LENGTH + FEE_ACC_LENGTH, "No fee account data in tx");
        Ok(bytes_slice_to_uint32(
            &input_data[BLOCK_NUMBER_LENGTH..BLOCK_NUMBER_LENGTH + FEE_ACC_LENGTH],
        )
        .ok_or(format_err!("Cant convert bytes to fee account number"))?)
    }
}

#[cfg(test)]
mod test {
    use crate::franklin_ops::FranklinOpsBlock;
    use bigdecimal::BigDecimal;
    use models::node::tx::TxSignature;
    use models::node::{
        AccountAddress, Close, CloseOp, Deposit, DepositOp, FranklinOp, FullExit, FullExitOp,
        Transfer, TransferOp, TransferToNewOp, Withdraw, WithdrawOp,
    };
    use models::params::{
        SIGNATURE_R_BIT_WIDTH_PADDED, SIGNATURE_S_BIT_WIDTH_PADDED, SUBTREE_HASH_WIDTH_PADDED,
    };

    #[test]
    fn test_deposit() {
        let priority_op = Deposit {
            sender: [9u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(10),
            account: AccountAddress::from_hex("0x7777777777777777777777777777777777777777")
                .unwrap(),
        };
        let op1 = FranklinOp::Deposit(Box::new(DepositOp {
            priority_op,
            account_id: 6,
        }));
        let pub_data1 = op1.public_data();
        let op2 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_part_exit() {
        let tx = Withdraw {
            account: AccountAddress::from_hex("0x7777777777777777777777777777777777777777")
                .unwrap(),
            eth_address: [9u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(20),
            fee: BigDecimal::from(10),
            nonce: 2,
            signature: TxSignature::default(),
        };
        let op1 = FranklinOp::Withdraw(Box::new(WithdrawOp { tx, account_id: 3 }));
        let pub_data1 = op1.public_data();
        let op2 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_successfull_full_exit() {
        let packed_pubkey = Box::new([7u8; SUBTREE_HASH_WIDTH_PADDED / 8]);
        let signature_r = Box::new([8u8; SIGNATURE_R_BIT_WIDTH_PADDED / 8]);
        let signature_s = Box::new([9u8; SIGNATURE_S_BIT_WIDTH_PADDED / 8]);
        let priority_op = FullExit {
            account_id: 11,
            packed_pubkey,
            eth_address: [9u8; 20].into(),
            token: 1,
            nonce: 3,
            signature_r,
            signature_s,
        };
        let op1 = FranklinOp::FullExit(Box::new(FullExitOp {
            priority_op,
            withdraw_amount: Some(BigDecimal::from(444)),
        }));
        let pub_data1 = op1.public_data();
        let op2 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_failed_full_exit() {
        let packed_pubkey = Box::new([7u8; SUBTREE_HASH_WIDTH_PADDED / 8]);
        let signature_r = Box::new([8u8; SIGNATURE_R_BIT_WIDTH_PADDED / 8]);
        let signature_s = Box::new([9u8; SIGNATURE_S_BIT_WIDTH_PADDED / 8]);
        let priority_op = FullExit {
            account_id: 11,
            packed_pubkey,
            eth_address: [9u8; 20].into(),
            token: 1,
            nonce: 3,
            signature_r,
            signature_s,
        };
        let op1 = FranklinOp::FullExit(Box::new(FullExitOp {
            priority_op,
            withdraw_amount: None,
        }));
        let pub_data1 = op1.public_data();
        let op2 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_transfer_to_new() {
        let tx = Transfer {
            from: AccountAddress::from_hex("0x7777777777777777777777777777777777777777").unwrap(),
            to: AccountAddress::from_hex("0x8888888888888888888888888888888888888888").unwrap(),
            token: 1,
            amount: BigDecimal::from(20),
            fee: BigDecimal::from(10),
            nonce: 3,
            signature: TxSignature::default(),
        };
        let op1 = FranklinOp::TransferToNew(Box::new(TransferToNewOp {
            tx,
            from: 11,
            to: 12,
        }));
        let pub_data1 = op1.public_data();
        let op2 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_transfer() {
        let tx = Transfer {
            from: AccountAddress::from_hex("0x7777777777777777777777777777777777777777").unwrap(),
            to: AccountAddress::from_hex("0x8888888888888888888888888888888888888888").unwrap(),
            token: 1,
            amount: BigDecimal::from(20),
            fee: BigDecimal::from(10),
            nonce: 3,
            signature: TxSignature::default(),
        };
        let op1 = FranklinOp::Transfer(Box::new(TransferOp {
            tx,
            from: 11,
            to: 12,
        }));
        let pub_data1 = op1.public_data();
        let op2 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_close() {
        let tx = Close {
            account: AccountAddress::from_hex("0x7777777777777777777777777777777777777777")
                .unwrap(),
            nonce: 3,
            signature: TxSignature::default(),
        };
        let op1 = FranklinOp::Close(Box::new(CloseOp { tx, account_id: 11 }));
        let pub_data1 = op1.public_data();
        let op2 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }
}
