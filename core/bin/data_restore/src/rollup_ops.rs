use crate::eth_tx_helpers::{get_ethereum_transaction, get_input_data_from_ethereum_transaction};
use crate::events::BlockEvent;
use ethabi::{ParamType, Token};
use web3::{Transport, Web3};
use zksync_types::operations::ZkSyncOp;

/// Description of a Rollup operations block
#[derive(Debug, Clone)]
pub struct RollupOpsBlock {
    /// Rollup block number
    pub block_num: u32,
    /// Rollup operations in block
    pub ops: Vec<ZkSyncOp>,
    /// Fee account
    pub fee_account: u32,
}

impl RollupOpsBlock {
    /// Returns a Rollup operations block description
    ///
    /// # Arguments
    ///
    /// * `web3` - Web3 provider url
    /// * `event_data` - Rollup contract event description
    ///
    ///
    fn decode_commitment_parameters(input_data: Vec<u8>) -> anyhow::Result<Vec<Token>> {
        let commit_operation = ParamType::Tuple(vec![
            Box::new(ParamType::Uint(32)),       // uint32 _blockNumber,
            Box::new(ParamType::Uint(32)),       // uint32 _feeAccount,
            Box::new(ParamType::FixedBytes(32)), // bytes32 encoded_root,
            Box::new(ParamType::Bytes),          // bytes calldata _publicData,
            Box::new(ParamType::Uint(32)),       // uint64 _timestamp,
            Box::new(ParamType::Array(Box::new(ParamType::Tuple(vec![
                Box::new(ParamType::Uint(32)), //uint32 public_data_offset
                Box::new(ParamType::Bytes),    // bytes eht_witness
            ])))), // uint32[] calldata onchainOps
        ]);
        let stored_block = ParamType::Tuple(vec![
            Box::new(ParamType::Uint(32)),       // uint32 _block_number
            Box::new(ParamType::Uint(32)),       // uint32 _number_of_processed_prior_ops
            Box::new(ParamType::FixedBytes(32)), //bytes32  processable_ops_hash
            Box::new(ParamType::Uint(32)),       // uint256 timestamp
            Box::new(ParamType::FixedBytes(32)), // bytes32 eth_encoded_root
            Box::new(ParamType::FixedBytes(32)), // commitment
        ]);
        ethabi::decode(
            vec![stored_block, ParamType::Array(Box::new(commit_operation))].as_slice(),
            input_data.as_slice(),
        )
        .map_err(|_| {
            anyhow::Error::from(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "can't get decoded parameters from commitment transaction",
            )))
        })
    }
    fn rollup_ops_blocs_from_bytes(data: Vec<u8>) -> anyhow::Result<Vec<Self>> {
        let fee_account_argument_id = 1;
        let public_data_argument_id = 3;

        let decoded_commitment_parameters = RollupOpsBlock::decode_commitment_parameters(data)?;
        assert_eq!(decoded_commitment_parameters.len(), 2);

        if let (ethabi::Token::Tuple(block), ethabi::Token::Array(operations)) = (
            &decoded_commitment_parameters[0],
            &decoded_commitment_parameters[1],
        ) {
            let mut blocks = vec![];
            if let ethabi::Token::Uint(block_num) = block[0] {
                for operation in operations {
                    if let ethabi::Token::Tuple(operation) = operation {
                        if let (ethabi::Token::Uint(fee_acc), ethabi::Token::Bytes(public_data)) = (
                            &operation[fee_account_argument_id],
                            &operation[public_data_argument_id],
                        ) {
                            let ops =
                                RollupOpsBlock::get_rollup_ops_from_data(public_data.as_slice())?;
                            blocks.push(RollupOpsBlock {
                                block_num: block_num.as_u32(),
                                ops,
                                fee_account: fee_acc.as_u32(),
                            })
                        } else {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::NotFound,
                                "can't parse operation parameters",
                            )
                            .into());
                        }
                    }
                }
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "can't parse block parameters",
                )
                .into());
            }
            Ok(blocks)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "can't parse commitment parameters",
            )
            .into())
        }
    }
    pub async fn get_rollup_ops_block<T: Transport>(
        web3: &Web3<T>,
        event_data: &BlockEvent,
    ) -> anyhow::Result<Vec<Self>> {
        let transaction = get_ethereum_transaction(web3, &event_data.transaction_hash).await?;
        let input_data = get_input_data_from_ethereum_transaction(&transaction)?;
        RollupOpsBlock::rollup_ops_blocs_from_bytes(input_data)
    }

    /// Returns a Rollup operations vector
    ///
    /// # Arguments
    ///
    /// * `data` - ZkSync Contract event input data
    ///
    pub fn get_rollup_ops_from_data(data: &[u8]) -> Result<Vec<ZkSyncOp>, anyhow::Error> {
        let mut current_pointer = 0;
        let mut ops = vec![];
        while current_pointer < data.len() {
            let op_type: u8 = data[current_pointer];

            let pub_data_size = ZkSyncOp::public_data_length(op_type)?;

            let pre = current_pointer;
            let post = pre + pub_data_size;

            let op = ZkSyncOp::from_public_data(&data[pre..post])?;

            ops.push(op);
            current_pointer += pub_data_size;
        }
        Ok(ops)
    }
}

#[cfg(test)]
mod test {
    use crate::rollup_ops::RollupOpsBlock;
    use num::BigUint;
    use zksync_types::operations::ChangePubKeyOp;
    use zksync_types::tx::{ChangePubKey, TxSignature};
    use zksync_types::{
        Close, CloseOp, Deposit, DepositOp, FullExit, FullExitOp, PubKeyHash, Transfer, TransferOp,
        TransferToNewOp, Withdraw, WithdrawOp, ZkSyncOp,
    };

    #[test]
    fn test_deposit() {
        let priority_op = Deposit {
            from: "1111111111111111111111111111111111111111".parse().unwrap(),
            token: 1,
            amount: 10u32.into(),
            to: "7777777777777777777777777777777777777777".parse().unwrap(),
        };
        let op1 = ZkSyncOp::Deposit(Box::new(DepositOp {
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
        let tx = Withdraw::new(
            3,
            "7777777777777777777777777777777777777777".parse().unwrap(),
            [9u8; 20].into(),
            1,
            20u32.into(),
            10u32.into(),
            2,
            None,
        );
        let op1 = ZkSyncOp::Withdraw(Box::new(WithdrawOp { tx, account_id: 3 }));
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
        let priority_op = FullExit {
            account_id: 11,
            eth_address: [9u8; 20].into(),
            token: 1,
        };
        let op1 = ZkSyncOp::FullExit(Box::new(FullExitOp {
            priority_op,
            withdraw_amount: Some(BigUint::from(444u32).into()),
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
        let priority_op = FullExit {
            account_id: 11,
            eth_address: [9u8; 20].into(),
            token: 1,
        };
        let op1 = ZkSyncOp::FullExit(Box::new(FullExitOp {
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
        let tx = Transfer::new(
            11,
            "7777777777777777777777777777777777777777".parse().unwrap(),
            "8888888888888888888888888888888888888888".parse().unwrap(),
            1,
            20u32.into(),
            20u32.into(),
            3,
            None,
        );
        let op1 = ZkSyncOp::TransferToNew(Box::new(TransferToNewOp {
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
        let tx = Transfer::new(
            11,
            "7777777777777777777777777777777777777777".parse().unwrap(),
            "8888888888888888888888888888888888888888".parse().unwrap(),
            1,
            20u32.into(),
            10u32.into(),
            3,
            None,
        );
        let op1 = ZkSyncOp::Transfer(Box::new(TransferOp {
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
            account: "7777777777777777777777777777777777777777".parse().unwrap(),
            nonce: 3,
            signature: TxSignature::default(),
        };
        let op1 = ZkSyncOp::Close(Box::new(CloseOp { tx, account_id: 11 }));
        let pub_data1 = op1.public_data();
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_change_pubkey_offchain() {
        let tx = ChangePubKey::new(
            11,
            "7777777777777777777777777777777777777777".parse().unwrap(),
            PubKeyHash::from_hex("sync:0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f").unwrap(),
            0,
            Default::default(),
            3,
            None,
            None,
        );
        let op1 = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp { tx, account_id: 11 }));
        let pub_data1 = op1.public_data();
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }
    #[test]
    fn test_decode_commitment() {
        let input_data = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 197, 210, 70, 1, 134, 247, 35, 60, 146, 126, 125, 178, 220, 199, 3,
            192, 229, 0, 182, 83, 202, 130, 39, 59, 123, 250, 216, 4, 93, 133, 164, 112, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            38, 26, 222, 68, 163, 255, 193, 28, 27, 138, 27, 11, 42, 14, 98, 64, 211, 104, 110,
            146, 95, 103, 112, 150, 178, 86, 154, 55, 112, 147, 24, 18, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 224, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 251, 0, 190, 245, 169, 14, 45, 82, 97, 155, 24, 225,
            167, 108, 103, 241, 222, 186, 32, 208, 18, 195, 54, 236, 68, 81, 96, 49, 89, 246, 125,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 192, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 95, 190, 144, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 13, 224, 182, 179, 167, 100, 0, 0, 13, 67, 235, 91, 138, 71, 186, 137, 0, 216, 74,
            163, 102, 86, 201, 32, 36, 233, 119, 46, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let blocks = RollupOpsBlock::rollup_ops_blocs_from_bytes(input_data).unwrap();
        assert_eq!(blocks.len(), 1);
        let block = blocks[0].clone();
        assert_eq!(block.block_num, 0);
        assert_eq!(block.fee_account, 0);
        let op = block.ops[0].clone();
        if let ZkSyncOp::Deposit(dep) = op {
            assert_eq!(dep.account_id, 1);
            assert_eq!(dep.priority_op.token, 0);
            assert_eq!(dep.priority_op.from, Default::default());
            assert_eq!(
                dep.priority_op.amount.to_string(),
                "1000000000000000000".to_string()
            );
            assert_ne!(dep.priority_op.to, Default::default());
        } else {
            panic!("Wrong type")
        }
    }
}
