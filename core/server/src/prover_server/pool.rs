// Built-in
use std::collections::{HashMap, VecDeque};
// External
// Workspace deps
use models::{node::BlockNumber, Operation};
use prover::prover_data::ProverData;

#[derive(Debug, Clone)]
struct OperationsQueue {
    operations: VecDeque<(Operation, bool)>,
    last_loaded_block: BlockNumber,
}

impl OperationsQueue {
    fn new(last_loaded_block: BlockNumber) -> Self {
        Self {
            operations: VecDeque::new(),
            last_loaded_block,
        }
    }

    /// Fills the operations queue if the amount of non-processed operations
    /// is less than `limit`.
    #[allow(dead_code)]
    fn take_next_commits_if_needed(
        &mut self,
        conn_pool: &storage::ConnectionPool,
        limit: i64,
    ) -> Result<(), String> {
        if self.operations.len() < limit as usize {
            let storage = conn_pool.access_storage().expect("failed to connect to db");
            let ops = storage
                .chain()
                .block_schema()
                .load_commits_after_block(self.last_loaded_block, limit)
                .map_err(|e| format!("failed to read commit operations: {}", e))?;

            self.operations.extend(ops);

            if let Some((op, _)) = self.operations.back() {
                self.last_loaded_block = op.block.block_number;
            }

            trace!(
                "Operations: {:?}",
                self.operations
                    .iter()
                    .map(|(op, _)| op.block.block_number)
                    .collect::<Vec<_>>()
            );
        }

        Ok(())
    }

    /// Takes the oldest non-processed operation out of the queue and whether it has a proof or not.
    /// Returns `None` if there are no non-processed operations.
    #[allow(dead_code)]
    fn take_next_operation(&mut self) -> Option<(Operation, bool)> {
        self.operations.pop_front()
    }

    /// Return block number of the next operation to take.
    #[allow(dead_code)]
    fn next_block_number(&self) -> Option<BlockNumber> {
        self.operations.front().map(|(op, _)| op.block.block_number)
    }

    // Whether queue is empty or not.
    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

#[derive(Debug)]
pub struct ProversDataPool {
    limit: i64,
    op_queue: OperationsQueue,
    prepared: HashMap<BlockNumber, ProverData>,
}

impl ProversDataPool {
    pub fn new(last_loaded_block: BlockNumber, limit: i64) -> Self {
        Self {
            limit,
            op_queue: OperationsQueue::new(last_loaded_block),
            prepared: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn get(&self, block: BlockNumber) -> Option<&ProverData> {
        self.prepared.get(&block)
    }

    pub fn clean_up(&mut self, block: BlockNumber) {
        self.prepared.remove(&block);
    }
}
