use ethabi::ParamType;

use zksync_types::{AccountId, BlockNumber, ZkSyncOp, H256};

use crate::rollup_ops::RollupOpsBlock;

pub fn rollup_ops_blocks_from_bytes(input_data: Vec<u8>) -> Result<RollupOpsBlock, anyhow::Error> {
    let block_number_argument_id = 0;
    let fee_account_argument_id = 1;
    let public_data_argument_id = 3;
    let decoded_commitment_parameters = ethabi::decode(
        vec![
            ParamType::Uint(32),                                   // uint32 _blockNumber,
            ParamType::Uint(32),                                   // uint32 _feeAccount,
            ParamType::Array(Box::new(ParamType::FixedBytes(32))), // bytes32[] _newRoots,
            ParamType::Bytes,                                      // bytes calldata _publicData,
            ParamType::Bytes,                                      // bytes calldata _ethWitness,
            ParamType::Array(Box::new(ParamType::Uint(32))), // uint32[] calldata _ethWitnessSizes
        ]
        .as_slice(),
        input_data.as_slice(),
    )
    .map_err(|_| {
        anyhow::Error::from(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded parameters from commitment transaction",
        )))
    })?;

    if let (
        ethabi::Token::Uint(block_num),
        ethabi::Token::Uint(fee_acc),
        ethabi::Token::Bytes(public_data),
    ) = (
        &decoded_commitment_parameters[block_number_argument_id],
        &decoded_commitment_parameters[fee_account_argument_id],
        &decoded_commitment_parameters[public_data_argument_id],
    ) {
        let ops = get_rollup_ops_from_data(public_data.as_slice())?;
        let fee_account = AccountId(fee_acc.as_u32());

        let block = RollupOpsBlock {
            block_num: BlockNumber(block_num.as_u32()),
            ops,
            fee_account,
            timestamp: None,
            previous_block_root_hash: H256::default(),
            contract_version: None,
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

/// Attempts to restore block operations from the public data.
/// Should be used for contracts V1-V5.
pub fn get_rollup_ops_from_data(data: &[u8]) -> Result<Vec<ZkSyncOp>, anyhow::Error> {
    parse_pub_data(
        data,
        ZkSyncOp::from_legacy_public_data,
        ZkSyncOp::legacy_public_data_length,
    )
}

pub(super) fn parse_pub_data<Parse, ParseErr, GetSize, GetSizeErr>(
    data: &[u8],
    parse: Parse,
    get_data_size: GetSize,
) -> Result<Vec<ZkSyncOp>, anyhow::Error>
where
    Parse: Fn(&[u8]) -> Result<ZkSyncOp, ParseErr>,
    ParseErr: std::error::Error + Send + Sync + 'static,
    GetSize: Fn(u8) -> Result<usize, GetSizeErr>,
    GetSizeErr: std::error::Error + Send + Sync + 'static,
{
    let mut current_pointer = 0;
    let mut ops = Vec::new();
    while current_pointer < data.len() {
        let op_type: u8 = data[current_pointer];

        let pub_data_size = get_data_size(op_type)?;

        let pre = current_pointer;
        let post = pre + pub_data_size;

        let op = parse(&data[pre..post])?;

        ops.push(op);
        current_pointer += pub_data_size;
    }
    Ok(ops)
}

#[cfg(test)]
mod test {
    use num::BigUint;

    use zksync_types::operations::ChangePubKeyOp;
    use zksync_types::tx::{ChangePubKey, TxSignature};
    use zksync_types::{
        AccountId, Close, CloseOp, Deposit, DepositOp, FullExit, FullExitOp, Nonce, PubKeyHash,
        TokenId, Transfer, TransferOp, TransferToNewOp, Withdraw, WithdrawOp, ZkSyncOp,
    };

    use crate::contract::v6;

    #[test]
    fn test_deposit() {
        let priority_op = Deposit {
            from: "1111111111111111111111111111111111111111".parse().unwrap(),
            token: TokenId(1),
            amount: 10u32.into(),
            to: "7777777777777777777777777777777777777777".parse().unwrap(),
        };
        let op1 = ZkSyncOp::Deposit(Box::new(DepositOp {
            priority_op,
            account_id: AccountId(6),
        }));
        let pub_data1 = op1.public_data();
        let op2 = v6::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_part_exit() {
        let tx = Withdraw::new(
            AccountId(3),
            "7777777777777777777777777777777777777777".parse().unwrap(),
            [9u8; 20].into(),
            TokenId(1),
            20u32.into(),
            10u32.into(),
            Nonce(2),
            Default::default(),
            None,
        );
        let op1 = ZkSyncOp::Withdraw(Box::new(WithdrawOp {
            tx,
            account_id: AccountId(3),
        }));
        let pub_data1 = op1.public_data();
        let op2 = v6::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_successfull_full_exit() {
        let priority_op = FullExit {
            account_id: AccountId(11),
            eth_address: [9u8; 20].into(),
            token: TokenId(1),
        };
        let op1 = ZkSyncOp::FullExit(Box::new(FullExitOp {
            priority_op,
            withdraw_amount: Some(BigUint::from(444u32).into()),
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        }));
        let pub_data1 = op1.public_data();
        let op2 = v6::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_failed_full_exit() {
        let priority_op = FullExit {
            account_id: AccountId(11),
            eth_address: [9u8; 20].into(),
            token: TokenId(1),
        };
        let op1 = ZkSyncOp::FullExit(Box::new(FullExitOp {
            priority_op,
            withdraw_amount: None,
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        }));
        let pub_data1 = op1.public_data();
        let op2 = v6::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_transfer_to_new() {
        let tx = Transfer::new(
            AccountId(11),
            "7777777777777777777777777777777777777777".parse().unwrap(),
            "8888888888888888888888888888888888888888".parse().unwrap(),
            TokenId(1),
            20u32.into(),
            20u32.into(),
            Nonce(3),
            Default::default(),
            None,
        );
        let op1 = ZkSyncOp::TransferToNew(Box::new(TransferToNewOp {
            tx,
            from: AccountId(11),
            to: AccountId(12),
        }));
        let pub_data1 = op1.public_data();
        let op2 = v6::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_transfer() {
        let tx = Transfer::new(
            AccountId(11),
            "7777777777777777777777777777777777777777".parse().unwrap(),
            "8888888888888888888888888888888888888888".parse().unwrap(),
            TokenId(1),
            20u32.into(),
            10u32.into(),
            Nonce(3),
            Default::default(),
            None,
        );
        let op1 = ZkSyncOp::Transfer(Box::new(TransferOp {
            tx,
            from: AccountId(11),
            to: AccountId(12),
        }));
        let pub_data1 = op1.public_data();
        let op2 = v6::get_rollup_ops_from_data(&pub_data1)
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
            nonce: Nonce(3),
            signature: TxSignature::default(),
            time_range: Default::default(),
        };
        let op1 = ZkSyncOp::Close(Box::new(CloseOp {
            tx,
            account_id: AccountId(11),
        }));
        let pub_data1 = op1.public_data();
        let op2 = v6::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }

    #[test]
    fn test_change_pubkey_offchain() {
        let tx = ChangePubKey::new(
            AccountId(11),
            "7777777777777777777777777777777777777777".parse().unwrap(),
            PubKeyHash::from_hex("sync:0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f").unwrap(),
            TokenId(0),
            Default::default(),
            Nonce(3),
            Default::default(),
            None,
            None,
        );
        let op1 = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
            tx,
            account_id: AccountId(11),
        }));
        let pub_data1 = op1.public_data();
        let op2 = v6::get_rollup_ops_from_data(&pub_data1)
            .expect("cant get ops from data")
            .pop()
            .expect("empty ops array");
        let pub_data2 = op2.public_data();
        assert_eq!(pub_data1, pub_data2);
    }
}
