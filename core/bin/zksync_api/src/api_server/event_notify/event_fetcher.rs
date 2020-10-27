use super::ExecutedOps;
use futures::{channel::mpsc, SinkExt};
use std::time::Duration;
use zksync_storage::ConnectionPool;
use zksync_types::{
    block::ExecutedOperations, block::PendingBlock, ActionType, BlockNumber, Operation,
};

/// Simple awaiter for the database futures, which will add a log entry upon DB failure
/// and execute `on_exit` statement.
macro_rules! await_db {
    ($e:expr, $on_exit:expr) => {
        match $e.await {
            Ok(res) => res,
            Err(err) => {
                log::warn!("Unable to connect to the database: {}", err);
                $on_exit
            }
        };
    };
}

/// Event fetcher is an actor which polls the database from time to time in order to see
/// whether new blocks were committed or verified.
///
/// Once tha new data is available, it is sent to the `OperationNotifier`, which broadcasts it
/// to the subscribers.
#[derive(Debug)]
pub struct EventFetcher {
    miniblock_interval: Duration,
    db_pool: ConnectionPool,

    last_committed_block: BlockNumber,
    last_verified_block: BlockNumber,
    pending_block: Option<PendingBlock>,

    operations_sender: mpsc::Sender<Operation>,
    txs_sender: mpsc::Sender<ExecutedOps>,
}

impl EventFetcher {
    pub async fn new(
        db_pool: ConnectionPool,
        miniblock_interval: Duration,
        operations_sender: mpsc::Sender<Operation>,
        txs_sender: mpsc::Sender<ExecutedOps>,
    ) -> anyhow::Result<Self> {
        let mut fetcher = EventFetcher {
            miniblock_interval,
            db_pool,

            last_committed_block: 0,
            last_verified_block: 0,
            pending_block: None,

            operations_sender,
            txs_sender,
        };

        let pending_block = fetcher.load_pending_block().await?;
        let last_committed_block = fetcher.last_committed_block().await?;
        let last_verified_block = fetcher.last_verified_block().await?;

        fetcher.last_committed_block = last_committed_block;
        fetcher.last_verified_block = last_verified_block;
        if let Some(block) = pending_block {
            // We only want to set this field if the pending block is actually the latest block (ahead of last committed one).
            if block.number > fetcher.last_committed_block {
                fetcher.pending_block = Some(block);
            }
        }

        Ok(fetcher)
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        let mut interval = tokio::time::interval(self.miniblock_interval);

        loop {
            interval.tick().await;

            // 1. Update last verified block.
            let last_verified_block = await_db!(self.last_verified_block(), continue);
            if last_verified_block > self.last_verified_block {
                self.send_operations(
                    self.last_verified_block,
                    last_verified_block,
                    ActionType::VERIFY,
                )
                .await;
                self.last_verified_block = last_verified_block;
            }

            // 2. Update last committed block.
            let last_committed_block = await_db!(self.last_committed_block(), continue);
            if last_committed_block > self.last_committed_block {
                self.send_operations(
                    self.last_committed_block,
                    last_committed_block,
                    ActionType::COMMIT,
                )
                .await;
                self.last_committed_block = last_committed_block;
            }

            // 3. Update pending block (it may contain new executed txs).
            let pending_block = await_db!(self.load_pending_block(), continue);
            if let Some(pending_block) = pending_block {
                // We're only interested in the pending blocks **newer** than the last committed blocks;
                if let Some(executed_ops) = self.update_pending_block(pending_block) {
                    self.txs_sender.send(executed_ops).await.unwrap_or_default();
                }
            }
        }
    }

    fn update_pending_block(&mut self, new: PendingBlock) -> Option<ExecutedOps> {
        if new.number <= self.last_committed_block {
            // Outdated block, we're not interested in it.
            return None;
        }

        let (last_success_len, last_errors_len) = if let Some(current) = &self.pending_block {
            if current.number == new.number {
                (current.success_operations.len(), current.failed_txs.len())
            } else {
                // New block is newer, consider all its operations
                (0, 0)
            }
        } else {
            // We have no pending block.
            (0, 0)
        };

        if new.success_operations.len() == last_success_len
            && new.failed_txs.len() == last_errors_len
        {
            // No change in the block.
            return None;
        }

        let mut executed_ops = ExecutedOps {
            block_number: new.number,
            operations: Vec::new(),
        };

        if new.success_operations.len() > last_success_len {
            for tx in &new.success_operations[last_success_len..] {
                executed_ops.operations.push(tx.clone());
            }
        }

        if new.failed_txs.len() > last_errors_len {
            for tx in &new.failed_txs[last_errors_len..] {
                executed_ops
                    .operations
                    .push(ExecutedOperations::Tx(Box::new(tx.clone())));
            }
        }

        self.pending_block = Some(new);
        Some(executed_ops)
    }

    async fn send_operations(
        &mut self,
        current_last_block: BlockNumber,
        new_last_operation: BlockNumber,
        action: ActionType,
    ) {
        // There may be more than one block in the gap.
        for block_idx in (current_last_block + 1)..=new_last_operation {
            let operation = await_db!(self.load_operation(block_idx, action), continue);
            self.operations_sender
                .send(operation)
                .await
                .unwrap_or_default();
        }
    }

    async fn load_pending_block(&mut self) -> anyhow::Result<Option<PendingBlock>> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .expect("Can't get access to the storage");
        let pending_block = storage.chain().block_schema().load_pending_block().await?;

        Ok(pending_block)
    }

    async fn last_committed_block(&mut self) -> anyhow::Result<BlockNumber> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .expect("Can't get access to the storage");

        let last_block = storage
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await?;

        Ok(last_block)
    }

    async fn last_verified_block(&mut self) -> anyhow::Result<BlockNumber> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .expect("Can't get access to the storage");

        let last_block = storage
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await?;

        Ok(last_block)
    }

    async fn load_operation(
        &mut self,
        block_number: BlockNumber,
        action_type: ActionType,
    ) -> anyhow::Result<Operation> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .expect("Can't get access to the storage");

        let op = storage
            .chain()
            .operations_schema()
            .get_operation(block_number, action_type)
            .await
            .expect("Operation must exist");

        op.into_op(&mut storage).await
    }
}
