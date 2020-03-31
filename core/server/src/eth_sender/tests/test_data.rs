//! Data to be used in tests as the input for `ETHSender`.

// External uses
use lazy_static::lazy_static;
// Workspace uses
use models::node::{block::Block, Fr};
use models::{Action, Operation};

/// Creates a dummy operation as a test input for `ETHSender` tests.
fn get_operation(id: i64, block_number: u32, action: Action) -> Operation {
    Operation {
        id: Some(id),
        action,
        block: Block {
            block_number,
            new_root_hash: Fr::default(),
            fee_account: 0,
            block_transactions: Vec::new(),
            processed_priority_ops: (0, 0),
        },
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
