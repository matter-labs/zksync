use crate::events::BlockEvent;
use crate::helpers::{get_ethereum_transaction, get_input_data_from_ethereum_transaction};
use failure::format_err;
use models::node::operations::FranklinOp;
use models::primitives::bytes_slice_to_uint32;

use models::params::{
    INPUT_DATA_BLOCK_NUMBER_BYTES_WIDTH, INPUT_DATA_EMPTY_BYTES_WIDTH,
    INPUT_DATA_FEE_ACC_BYTES_WIDTH, INPUT_DATA_FEE_ACC_BYTES_WIDTH_WITH_EMPTY_OFFSET,
    INPUT_DATA_ROOT_BYTES_WIDTH,
};

/// Description of a Franklin operations block
#[derive(Debug, Clone)]
pub struct RollupOpsBlock {
    /// Franklin block number
    pub block_num: u32,
    /// Franklin operations in block
    pub ops: Vec<FranklinOp>,
    /// Fee account
    pub fee_account: u32,
}

impl RollupOpsBlock {
    /// Return Franklin operations block description
    ///
    /// # Arguments
    ///
    /// * `event_data` - Franklin Contract event description
    ///
    pub fn get_rollup_ops_block(
        web3_url: &String,
        event_data: &BlockEvent,
    ) -> Result<Self, failure::Error> {
        let transaction = get_ethereum_transaction(web3_url, &event_data.transaction_hash)?;
        let input_data = get_input_data_from_ethereum_transaction(&transaction)?;
        // info!("New ops block input_data: {:?}", &input_data);
        let commitment_data = &input_data[INPUT_DATA_BLOCK_NUMBER_BYTES_WIDTH
            + INPUT_DATA_FEE_ACC_BYTES_WIDTH_WITH_EMPTY_OFFSET
            + INPUT_DATA_ROOT_BYTES_WIDTH
            + INPUT_DATA_EMPTY_BYTES_WIDTH
            ..input_data.len()];
        // info!("New ops block commitment_data: {:?}", &commitment_data);
        let fee_account = RollupOpsBlock::get_fee_account_from_tx_input(&input_data)?;
        let ops = RollupOpsBlock::get_rollup_ops_from_data(commitment_data)?;
        let block = RollupOpsBlock {
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
    pub fn get_rollup_ops_from_data(data: &[u8]) -> Result<Vec<FranklinOp>, failure::Error> {
        let mut current_pointer = 0;
        let mut ops = vec![];
        while current_pointer < data.len() {
            let op_type: u8 = data[current_pointer];

            let pub_data_size = FranklinOp::public_data_length(op_type)?;

            let pre = current_pointer;
            let post = pre + pub_data_size;

            let op = FranklinOp::from_public_data(&data[pre..post])?;
            // info!("New op: {:?}", &op);

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
        // ensure!(
        //     input_data.len()
        //         == INPUT_DATA_BLOCK_NUMBER_BYTES_WIDTH + INPUT_DATA_FEE_ACC_BYTES_WIDTH,
        //     "No fee account data in tx"
        // );
        Ok(bytes_slice_to_uint32(
            &input_data[INPUT_DATA_BLOCK_NUMBER_BYTES_WIDTH
                + INPUT_DATA_FEE_ACC_BYTES_WIDTH_WITH_EMPTY_OFFSET
                - INPUT_DATA_FEE_ACC_BYTES_WIDTH
                ..INPUT_DATA_BLOCK_NUMBER_BYTES_WIDTH
                    + INPUT_DATA_FEE_ACC_BYTES_WIDTH_WITH_EMPTY_OFFSET],
        )
        .ok_or_else(|| format_err!("Cant convert bytes to fee account number"))?)
    }
}

#[cfg(test)]
mod test {
    use crate::rollup_ops::RollupOpsBlock;
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
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
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
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
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
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
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
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
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
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
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
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
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
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }
}
