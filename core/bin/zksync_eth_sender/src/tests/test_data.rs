//! Data to be used in tests as the input for `ETHSender`.

use std::time::SystemTime;
// External uses
use chrono::DateTime;
use lazy_static::lazy_static;
// Workspace uses
use zksync_types::{
    block::Block, Address, ExecutedOperations, ExecutedPriorityOp, Fr, FullExit, FullExitOp,
    PriorityOp, ZkSyncOp, ZkSyncPriorityOp,
};
use zksync_types::{Action, Operation};

/// Creates a dummy operation as a test input for `ETHSender` tests.
fn get_operation(id: i64, block_number: u32, action: Action) -> Operation {
    // Create full exit operation for non-zero return data.
    let executed_full_exit_op = {
        let priority_op = FullExit {
            account_id: 0,
            eth_address: Address::zero(),
            token: 0,
        };
        ExecutedOperations::PriorityOp(Box::new(ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id: 0,
                data: ZkSyncPriorityOp::FullExit(priority_op.clone()),
                deadline_block: 0,
                eth_hash: Vec::new(),
                eth_block: 0,
            },
            op: ZkSyncOp::FullExit(Box::new(FullExitOp {
                priority_op,
                withdraw_amount: None,
            })),
            block_index: 0,
            created_at: DateTime::from(SystemTime::UNIX_EPOCH),
        }))
    };
    Operation {
        id: Some(id),
        action,
        block: Block::new(
            block_number,
            Fr::default(),
            0,
            vec![executed_full_exit_op],
            (0, 0),
            50,
            1_000_000.into(),
            1_500_000.into(),
        ),
        accounts_updated: Vec::new(),
    }
}

lazy_static! {
    pub static ref COMMIT_OPERATIONS: Vec<Operation> = (1..10)
        .map(|id| get_operation(id, id as u32, Action::Commit))
        .collect();
    pub static ref VERIFY_OPERATIONS: Vec<Operation> = (11..20)
        .map(|id| get_operation(
            id,
            (id - 10) as u32,
            Action::Verify {
                proof: Default::default()
            }
        ))
        .collect();
}

pub fn commit_operation(idx: usize) -> Operation {
    assert!(
        idx < COMMIT_OPERATIONS.len(),
        format!("Index {} is out of bounds for commit operations", idx)
    );

    COMMIT_OPERATIONS[idx].clone()
}

pub fn verify_operation(idx: usize) -> Operation {
    assert!(
        idx < VERIFY_OPERATIONS.len(),
        format!("Index {} is out of bounds for verify operations", idx)
    );

    VERIFY_OPERATIONS[idx].clone()
}
