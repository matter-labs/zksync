use crate::eth_tx_helpers::{get_ethereum_transaction, get_input_data_from_ethereum_transaction};
use crate::events::BlockEvent;
use models::node::operations::FranklinOp;
use web3::{Transport, Web3};

/// Description of a Rollup operations block
#[derive(Debug, Clone)]
pub struct RollupOpsBlock {
    /// Rollup block number
    pub block_num: u32,
    /// Rollup operations in block
    pub ops: Vec<FranklinOp>,
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
    pub fn get_rollup_ops_block<T: Transport>(
        web3: &Web3<T>,
        event_data: &BlockEvent,
    ) -> Result<Self, failure::Error> {
        let transaction = get_ethereum_transaction(web3, &event_data.transaction_hash)?;
        let input_data = get_input_data_from_ethereum_transaction(&transaction)?;

        let decoded_commitment_parameters = ethabi::decode(
            vec![
                ethabi::ParamType::Uint(32),
                ethabi::ParamType::Uint(24),
                ethabi::ParamType::FixedBytes(32),
                ethabi::ParamType::Bytes,
                ethabi::ParamType::Bytes,
                ethabi::ParamType::Array(Box::new(ethabi::ParamType::Uint(32))),
            ]
            .as_slice(),
            input_data.as_slice(),
        )
        .map_err(|_| {
            failure::Error::from_boxed_compat(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "can't get decoded parameters from commitment transaction",
            )))
        })?;

        if let (ethabi::Token::Uint(fee_acc), ethabi::Token::Bytes(public_data)) = (
            &decoded_commitment_parameters[1],
            &decoded_commitment_parameters[3],
        ) {
            let ops = RollupOpsBlock::get_rollup_ops_from_data(public_data.as_slice())?;
            let fee_account = fee_acc.as_u32();

            let block = RollupOpsBlock {
                block_num: event_data.block_num,
                ops,
                fee_account,
            };
            Ok(block)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "can't parse commitment parameters",
            )
            .into())
        }
    }

    /// Returns a Rollup operations vector
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

            ops.push(op);
            current_pointer += pub_data_size;
        }
        Ok(ops)
    }
}

#[cfg(test)]
mod test {
    use crate::rollup_ops::RollupOpsBlock;
    use bigdecimal::BigDecimal;
    use models::node::operations::ChangePubKeyOp;
    use models::node::tx::{ChangePubKey, TxSignature};
    use models::node::{
        Close, CloseOp, Deposit, DepositOp, FranklinOp, FullExit, FullExitOp, PubKeyHash, Transfer,
        TransferOp, TransferToNewOp, Withdraw, WithdrawOp,
    };

    #[test]
    fn test_deposit() {
        let priority_op = Deposit {
            from: "1111111111111111111111111111111111111111".parse().unwrap(),
            token: 1,
            amount: BigDecimal::from(10),
            to: "7777777777777777777777777777777777777777".parse().unwrap(),
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
        let tx = Withdraw::new(
            "7777777777777777777777777777777777777777".parse().unwrap(),
            [9u8; 20].into(),
            1,
            BigDecimal::from(20),
            BigDecimal::from(10),
            2,
            None,
        );
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
        let priority_op = FullExit {
            account_id: 11,
            eth_address: [9u8; 20].into(),
            token: 1,
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
        let priority_op = FullExit {
            account_id: 11,
            eth_address: [9u8; 20].into(),
            token: 1,
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
        let tx = Transfer::new(
            "7777777777777777777777777777777777777777".parse().unwrap(),
            "8888888888888888888888888888888888888888".parse().unwrap(),
            1,
            BigDecimal::from(20),
            BigDecimal::from(10),
            3,
            None,
        );
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
        let tx = Transfer::new(
            "7777777777777777777777777777777777777777".parse().unwrap(),
            "8888888888888888888888888888888888888888".parse().unwrap(),
            1,
            BigDecimal::from(20),
            BigDecimal::from(10),
            3,
            None,
        );
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
            account: "7777777777777777777777777777777777777777".parse().unwrap(),
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

    #[test]
    fn test_change_pubkey_offchain() {
        let tx = ChangePubKey {
            account: "7777777777777777777777777777777777777777".parse().unwrap(),
            new_pk_hash: PubKeyHash::from_hex("sync:0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f")
                .unwrap(),
            nonce: 3,
            eth_signature: None,
        };
        let op1 = FranklinOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp { tx, account_id: 11 }));
        let pub_data1 = op1.public_data();
        let op2 = RollupOpsBlock::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }
}
