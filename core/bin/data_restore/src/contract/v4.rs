use ethabi::{ParamType, Token};

use crate::{contract::default::get_rollup_ops_from_data, rollup_ops::RollupOpsBlock};

fn decode_commitment_parameters(input_data: Vec<u8>) -> anyhow::Result<Vec<Token>> {
    let commit_operation = ParamType::Tuple(vec![
        Box::new(ParamType::FixedBytes(32)), // bytes32 encoded_root,
        Box::new(ParamType::Bytes),          // bytes calldata _publicData,
        Box::new(ParamType::Uint(256)),      // uint64 _timestamp,
        Box::new(ParamType::Array(Box::new(ParamType::Tuple(vec![
            Box::new(ParamType::Bytes),    // bytes eht_witness
            Box::new(ParamType::Uint(32)), //uint32 public_data_offset
        ])))),
        Box::new(ParamType::Uint(32)), // uint32 _blockNumber,
        Box::new(ParamType::Uint(32)), // uint32 _feeAccount,
    ]);
    let stored_block = ParamType::Tuple(vec![
        Box::new(ParamType::Uint(32)),       // uint32 _block_number
        Box::new(ParamType::Uint(64)),       // uint32 _number_of_processed_prior_ops
        Box::new(ParamType::FixedBytes(32)), //bytes32  processable_ops_hash
        Box::new(ParamType::Uint(256)),      // uint256 timestamp
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

pub fn rollup_ops_blocks_from_bytes(data: Vec<u8>) -> anyhow::Result<Vec<RollupOpsBlock>> {
    let fee_account_argument_id = 5;
    let public_data_argument_id = 1;

    let decoded_commitment_parameters = decode_commitment_parameters(data)?;
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
                        let ops = get_rollup_ops_from_data(public_data.as_slice())?;
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

#[cfg(test)]
mod test {
    use zksync_types::ZkSyncOp;

    use super::*;
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
        let blocks = rollup_ops_blocks_from_bytes(input_data).unwrap();
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
