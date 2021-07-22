use ethabi::{ParamType, Token};

use super::version::ZkSyncContractVersion;
use crate::rollup_ops::RollupOpsBlock;
use zksync_types::{AccountId, BlockNumber, H256};

fn decode_commitment_parameters(input_data: Vec<u8>) -> anyhow::Result<Vec<Token>> {
    let commit_operation = ParamType::Tuple(vec![
        Box::new(ParamType::FixedBytes(32)), // bytes32 encoded_root,
        Box::new(ParamType::Bytes),          // bytes calldata _publicData,
        Box::new(ParamType::Uint(256)),      // uint256 _timestamp,
        Box::new(ParamType::Array(Box::new(ParamType::Tuple(vec![
            Box::new(ParamType::Bytes),    // bytes eht_witness
            Box::new(ParamType::Uint(32)), //uint32 public_data_offset
        ])))),
        Box::new(ParamType::Uint(32)), // uint32 _blockNumber,
        Box::new(ParamType::Uint(32)), // uint32 _feeAccount,
    ]);
    let stored_block = ParamType::Tuple(vec![
        Box::new(ParamType::Uint(32)),       // uint32 blockNumber
        Box::new(ParamType::Uint(64)),       // uint32 priorityOperations
        Box::new(ParamType::FixedBytes(32)), // bytes32  pendingOnchainOperationsHash
        Box::new(ParamType::Uint(256)),      // uint256 timestamp
        Box::new(ParamType::FixedBytes(32)), // bytes32 stateHash
        Box::new(ParamType::FixedBytes(32)), // bytes32 commitment
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
    rollup_ops_blocks_from_bytes_inner(data, ZkSyncContractVersion::V4)
}

pub(super) fn rollup_ops_blocks_from_bytes_inner(
    data: Vec<u8>,
    contract_version: ZkSyncContractVersion,
) -> anyhow::Result<Vec<RollupOpsBlock>> {
    assert!(
        i32::from(contract_version) >= 4,
        "Contract version must be greater or equal to 4"
    );

    let root_hash_argument_id = 0;
    let public_data_argument_id = 1;
    let timestamp_argument_id = 2;
    let op_block_number_argument_id = 4;
    let fee_account_argument_id = 5;

    // ID of `eth_encoded_root` in `StoredBlockInfo`.
    let previous_block_root_hash_argument_id = 4;

    let decoded_commitment_parameters = decode_commitment_parameters(data)?;
    assert_eq!(decoded_commitment_parameters.len(), 2);

    let mut previous_block_root_hash =
        if let ethabi::Token::Tuple(prev_stored) = &decoded_commitment_parameters[0] {
            if let ethabi::Token::FixedBytes(root_hash) =
                &prev_stored[previous_block_root_hash_argument_id]
            {
                H256::from_slice(&root_hash)
            } else {
                panic!("can't parse root hash param: {:#?}", prev_stored);
            }
        } else {
            panic!(
                "can't parse root hash param: {:#?}",
                decoded_commitment_parameters
            );
        };

    // Destruct deserialized parts of transaction input data for getting operations
    // Input data consists of stored block and operations
    // Transform operations to RollupBlock
    if let ethabi::Token::Array(operations) = &decoded_commitment_parameters[1] {
        let mut blocks = vec![];
        for operation in operations {
            if let ethabi::Token::Tuple(operation) = operation {
                if let (
                    ethabi::Token::FixedBytes(root_hash),
                    ethabi::Token::Uint(fee_acc),
                    ethabi::Token::Bytes(public_data),
                    ethabi::Token::Uint(block_number),
                    ethabi::Token::Uint(timestamp),
                ) = (
                    &operation[root_hash_argument_id],
                    &operation[fee_account_argument_id],
                    &operation[public_data_argument_id],
                    &operation[op_block_number_argument_id],
                    &operation[timestamp_argument_id],
                ) {
                    let ops = contract_version.get_rollup_ops_from_data(public_data.as_slice())?;
                    blocks.push(RollupOpsBlock {
                        block_num: BlockNumber(block_number.as_u32()),
                        ops,
                        fee_account: AccountId(fee_acc.as_u32()),
                        timestamp: Some(timestamp.as_u64()),
                        previous_block_root_hash,
                        contract_version: None,
                    });

                    previous_block_root_hash = H256::from_slice(&root_hash);
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "can't parse operation parameters",
                    )
                    .into());
                }
            }
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
    use super::*;
    #[test]
    fn test_decode_commitment() {
        let input_data = hex::decode(
            "45269298000000000000000000000000000000000000000000\
            00000000000000000000180000000000000000000000000000\
            000000000000000000000000000000000001c5d2460186f723\
            3c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470\
            00000000000000000000000000000000000000000000000000\
            00000060180bd21ebc71244dfd0ec72156cabe55ae2e5dd35e\
            1b0a1cffe0b52a158f27c1dd34314cebb54dbafb6885b8628c\
            a09d8f4992f4efd7f04e2dda0121896e88a5158f8100000000\
            00000000000000000000000000000000000000000000000000\
            0000e000000000000000000000000000000000000000000000\
            00000000000000000001000000000000000000000000000000\
            000000000000000000000000000000002026bb57dafd75ff97\
            f3c664c511c5e334f0266c6bd0e29e9a69f5c36152fef48100\
            00000000000000000000000000000000000000000000000000\
            0000000000c000000000000000000000000000000000000000\
            00000000000000000060183511000000000000000000000000\
            00000000000000000000000000000000000001400000000000\
            00000000000000000000000000000000000000000000000000\
            00190000000000000000000000000000000000000000000000\
            00000000000000000000000000000000000000000000000000\
            0000000000000000000000000000005a010000000e00000000\
            00000000006c6b935b8bbd4000001e65c448e0486449a0b446\
            bc9a340b933237f6e000000000000000000000000000000000\
            00000000000000000000000000000000000000000000000000\
            00000000000000000000000000000000000000000000000000\
            00000000000000000000000000000000000001000000000000\
            00000000000000000000000000000000000000000000000000\
            20000000000000000000000000000000000000000000000000\
            00000000000000400000000000000000000000000000000000\
            00000000000000000000000000000000000000000000000000\
            00000000000000000000000000000000000000000000",
        )
        .expect("Failed to decode commit tx data");
        let blocks = rollup_ops_blocks_from_bytes(input_data[4..].to_vec()).unwrap();
        assert_eq!(blocks.len(), 1);
        let block = blocks[0].clone();
        assert_eq!(block.block_num, BlockNumber(25));
        assert_eq!(block.fee_account, AccountId(0));
        assert_eq!(block.ops.len(), 5);
    }
}
