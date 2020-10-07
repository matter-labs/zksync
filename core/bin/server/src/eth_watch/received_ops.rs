// Built-in deps
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
// Workspace deps
use zksync_types::PriorityOp;

pub const SECS_IN_HOUR: u64 = 3600;

/// Duplicate of the `PRIORITY_EXPIRATION` constant value from `Config.sol`.
pub const PRIORITY_EXPIRATION_DAYS: u64 = 6;

/// Interval for assuming that stored priority operation is outdated and should
/// be removed from the queue.
///
/// This value must be greater than `PRIORITY_EXPIRATION` constant from the
/// `Config.sol` contract. Currently the value is 3 days (value from contract)
/// + 2 hours (just for the safety).
pub const PRIORITY_OP_EXPIRATION: Duration =
    Duration::from_secs(PRIORITY_EXPIRATION_DAYS * 24 * SECS_IN_HOUR + 2 * SECS_IN_HOUR);

/// Received `PriorityOp` with additional metainformation required
/// for efficient management of the operations queue.
#[derive(Debug, Clone)]
pub struct ReceivedPriorityOp {
    op: PriorityOp,
    received_at: Instant,
}

impl ReceivedPriorityOp {
    pub fn is_outdated(&self) -> bool {
        self.received_at.elapsed() >= PRIORITY_OP_EXPIRATION
    }
}

impl From<PriorityOp> for ReceivedPriorityOp {
    fn from(op: PriorityOp) -> Self {
        Self {
            op,
            received_at: Instant::now(),
        }
    }
}

impl AsRef<PriorityOp> for ReceivedPriorityOp {
    fn as_ref(&self) -> &PriorityOp {
        &self.op
    }
}

/// Goes through provided operations queue, retaining only ones that are
/// not outdated.
pub fn sift_outdated_ops(
    ops: &HashMap<u64, ReceivedPriorityOp>,
) -> HashMap<u64, ReceivedPriorityOp> {
    ops.iter()
        .filter_map(|(id, op)| {
            if !op.is_outdated() {
                Some((*id, op.clone()))
            } else {
                None
            }
        })
        .collect()
}
