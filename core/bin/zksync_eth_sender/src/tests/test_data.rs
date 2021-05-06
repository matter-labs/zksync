//! Data to be used in tests as the input for `ETHSender`.

use std::time::SystemTime;
// External uses
use chrono::DateTime;
use lazy_static::lazy_static;
// Workspace uses
use zksync_basic_types::H256;
use zksync_types::{
    aggregated_operations::{
        AggregatedActionType, AggregatedOperation, BlocksCommitOperation, BlocksExecuteOperation,
        BlocksProofOperation,
    },
    block::Block,
    AccountId, Address, BlockNumber, ExecutedOperations, ExecutedPriorityOp, Fr, FullExit,
    FullExitOp, PriorityOp, TokenId, ZkSyncOp, ZkSyncPriorityOp,
};

/// Creates a dummy operation as a test input for `ETHSender` tests.
fn gen_aggregated_operation(
    id: i64,
    block_number: BlockNumber,
    action: AggregatedActionType,
) -> (i64, AggregatedOperation) {
    // Create full exit operation for non-zero return data.
    let executed_full_exit_op = {
        let priority_op = FullExit {
            account_id: AccountId(0),
            eth_address: Address::zero(),
            token: TokenId(0),
        };
        ExecutedOperations::PriorityOp(Box::new(ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id: 0,
                data: ZkSyncPriorityOp::FullExit(priority_op.clone()),
                deadline_block: 0,
                eth_hash: H256::zero(),
                eth_block: 0,
            },
            op: ZkSyncOp::FullExit(Box::new(FullExitOp {
                priority_op,
                withdraw_amount: None,
                creator_account_id: None,
                creator_address: None,
                serial_id: None,
                content_hash: None,
            })),
            block_index: 0,
            created_at: DateTime::from(SystemTime::UNIX_EPOCH),
        }))
    };
    let block = Block::new(
        block_number,
        Fr::default(),
        AccountId(0),
        vec![executed_full_exit_op],
        (0, 0),
        50,
        1_000_000.into(),
        1_500_000.into(),
        H256::default(),
        0,
    );

    let aggregated_operation = match action {
        AggregatedActionType::CommitBlocks => {
            AggregatedOperation::CommitBlocks(BlocksCommitOperation {
                last_committed_block: block.clone(),
                blocks: vec![block],
            })
        }
        AggregatedActionType::CreateProofBlocks => {
            panic!("Proof creation should never be sent to Ethereum");
        }
        AggregatedActionType::PublishProofBlocksOnchain => {
            AggregatedOperation::PublishProofBlocksOnchain(BlocksProofOperation {
                blocks: vec![block],
                proof: Default::default(),
            })
        }
        AggregatedActionType::ExecuteBlocks => {
            AggregatedOperation::ExecuteBlocks(BlocksExecuteOperation {
                blocks: vec![block],
            })
        }
    };
    println!("{:?}", aggregated_operation.get_block_range());

    (id, aggregated_operation)
}

lazy_static! {
    pub static ref COMMIT_BLOCKS_OPERATIONS: Vec<(i64, AggregatedOperation)> = (1..=10)
        .map(|id| gen_aggregated_operation(
            id,
            BlockNumber(id as u32),
            AggregatedActionType::CommitBlocks
        ))
        .collect();
    pub static ref PUBLISH_PROOF_BLOCKS_ONCHAIN_OPERATIONS: Vec<(i64, AggregatedOperation)> = (11
        ..=20)
        .map(|id| gen_aggregated_operation(
            id,
            BlockNumber((id - 10) as u32),
            AggregatedActionType::PublishProofBlocksOnchain
        ))
        .collect();
    pub static ref EXECUTE_BLOCKS_OPERATIONS: Vec<(i64, AggregatedOperation)> = (21..=30)
        .map(|id| gen_aggregated_operation(
            id,
            BlockNumber((id - 20) as u32),
            AggregatedActionType::ExecuteBlocks
        ))
        .collect();
}

pub fn commit_blocks_operation(idx: usize) -> (i64, AggregatedOperation) {
    assert!(
        idx < COMMIT_BLOCKS_OPERATIONS.len(),
        "Index {} is out of bounds for commit blocks operations",
        idx
    );

    COMMIT_BLOCKS_OPERATIONS[idx].clone()
}

pub fn publish_proof_blocks_onchain_operations(idx: usize) -> (i64, AggregatedOperation) {
    assert!(
        idx < PUBLISH_PROOF_BLOCKS_ONCHAIN_OPERATIONS.len(),
        "Index {} is out of bounds for publish proof blocks onchain operations",
        idx
    );

    PUBLISH_PROOF_BLOCKS_ONCHAIN_OPERATIONS[idx].clone()
}

pub fn execute_blocks_operations(idx: usize) -> (i64, AggregatedOperation) {
    assert!(
        idx < EXECUTE_BLOCKS_OPERATIONS.len(),
        "Index {} is out of bounds for execute blocks onchain operations",
        idx
    );

    EXECUTE_BLOCKS_OPERATIONS[idx].clone()
}
