//! Additional methods gathering the information required
//! by ticker for operating.

// External deps
use async_trait::async_trait;
// Workspace deps
use zksync_storage::ConnectionPool;
use zksync_types::aggregated_operations::AggregatedActionType;
use zksync_types::Address;
// Local deps

/// Api responsible for querying for TokenPrices
#[async_trait]
pub trait FeeTickerInfo {
    /// Check whether account exists in the zkSync network or not.
    /// Returns `true` if account does not yet exist in the zkSync network.
    async fn is_account_new(&mut self, address: Address) -> bool;

    async fn blocks_in_future_aggregated_operations(
        &mut self,
    ) -> BlocksInFutureAggregatedOperations;
}

#[derive(Clone)]
pub struct TickerInfo {
    db: ConnectionPool,
}

impl TickerInfo {
    pub fn new(db: ConnectionPool) -> Self {
        Self { db }
    }
}

#[derive(Debug, Clone)]
pub struct BlocksInFutureAggregatedOperations {
    pub blocks_to_commit: u32,
    pub blocks_to_prove: u32,
    pub blocks_to_execute: u32,
}

#[async_trait]
impl FeeTickerInfo for TickerInfo {
    async fn is_account_new(&mut self, address: Address) -> bool {
        let mut storage = self
            .db
            .access_storage()
            .await
            .expect("Unable to establish connection to db");

        let account_state = storage
            .chain()
            .account_schema()
            .account_state_by_address(address)
            .await
            .expect("Unable to query account state from the database");

        // If account is `Some(_)` then it's not new.
        account_state.committed.is_none()
    }

    async fn blocks_in_future_aggregated_operations(
        &mut self,
    ) -> BlocksInFutureAggregatedOperations {
        let mut storage = self
            .db
            .access_storage()
            .await
            .expect("Unable to establish connection to db");

        let last_block = storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await
            .expect("Unable to query account state from the database");
        let last_committed_block = storage
            .chain()
            .operations_schema()
            .get_last_block_by_aggregated_action(AggregatedActionType::CommitBlocks, None)
            .await
            .expect("Unable to query block from the database");
        let last_proven_block = storage
            .chain()
            .operations_schema()
            .get_last_block_by_aggregated_action(
                AggregatedActionType::PublishProofBlocksOnchain,
                None,
            )
            .await
            .expect("Unable to query block state from the database");
        let last_executed_block = storage
            .chain()
            .operations_schema()
            .get_last_block_by_aggregated_action(AggregatedActionType::ExecuteBlocks, None)
            .await
            .expect("Unable to query block from the database");
        BlocksInFutureAggregatedOperations {
            blocks_to_commit: *last_block - *last_committed_block,
            blocks_to_prove: *last_block - *last_proven_block,
            blocks_to_execute: *last_block - *last_executed_block,
        }
    }
}
