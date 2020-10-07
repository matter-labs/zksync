use super::ExecutedOps;
use futures::channel::mpsc;
use std::time::Duration;
use zksync_storage::ConnectionPool;
use zksync_types::{block::PendingBlock, BlockNumber, Operation};

#[derive(Debug)]
pub struct EventFetcher {
    miniblock_interval: Duration,
    db_pool: ConnectionPool,

    last_block: BlockNumber,
    success_operations_len: usize,
    failed_txs_len: usize,

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

            last_block: 0,
            success_operations_len: 0,
            failed_txs_len: 0,

            operations_sender,
            txs_sender,
        };

        let pending_block = fetcher.load_pending_block().await?;
        let (last_block, success_operations_len, failed_txs_len) = match pending_block {
            Some(block) => (
                block.number,
                block.success_operations.len(),
                block.failed_txs.len(),
            ),
            None => {
                let block_number = fetcher.last_committed_block().await?;
                (block_number, 0, 0)
            }
        };

        fetcher.last_block = last_block;
        fetcher.success_operations_len = success_operations_len;
        fetcher.failed_txs_len = failed_txs_len;

        Ok(fetcher)
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        let mut interval = tokio::time::interval(self.miniblock_interval);

        loop {
            interval.tick().await;

            let _pending_block = match self.load_pending_block().await {
                Ok(block) => block,
                Err(err) => {
                    log::warn!("Unable to connect to the database: {}", err);
                    continue;
                }
            };

            // match
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
}
