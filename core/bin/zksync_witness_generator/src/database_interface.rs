//! Module encapsulating the database interaction.
//! The essential part of this module is the trait that abstracts
//! the database interaction, so no real database is needed to run
//! the prover-server, which is required for tests.

// Built-in
use std::clone::Clone;
use std::marker::{Send, Sync};
// Workspace uses
use zksync_crypto::proof::{AggregatedProof, SingleProof};
use zksync_storage::StorageProcessor;
use zksync_types::aggregated_operations::{AggregatedActionType, AggregatedOperation};
use zksync_types::{
    block::Block,
    prover::{ProverJob, ProverJobType},
    AccountMap, AccountUpdates, BlockNumber,
};

/// Abstract database access trait.
#[async_trait::async_trait]
pub trait DatabaseInterface: Send + Sync + Clone + 'static {
    /// Returns connection to the database.
    async fn acquire_connection(&self) -> anyhow::Result<StorageProcessor<'_>>;

    /// Returns the block number with the largest last block.
    async fn load_last_block_prover_job_queue(
        &self,
        connection: &mut StorageProcessor<'_>,
        job_type: ProverJobType,
    ) -> anyhow::Result<BlockNumber>;

    /// Returns stored witness for a block.
    async fn load_witness(
        &self,
        connection: &mut StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> anyhow::Result<Option<serde_json::Value>>;

    async fn add_prover_job_to_job_queue(
        &self,
        connection: &mut StorageProcessor<'_>,
        first_block: BlockNumber,
        last_block: BlockNumber,
        job_data: serde_json::Value,
        job_priority: i32,
        job_type: ProverJobType,
    ) -> anyhow::Result<()>;

    async fn load_aggregated_op_that_affects_block(
        &self,
        connection: &mut StorageProcessor<'_>,
        aggregated_action: AggregatedActionType,
        block_number: BlockNumber,
    ) -> anyhow::Result<Option<(i64, AggregatedOperation)>>;

    async fn load_proof(
        &self,
        connection: &mut StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> anyhow::Result<Option<SingleProof>>;

    async fn mark_stale_jobs_as_idle(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<()>;

    async fn load_last_verified_block(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<BlockNumber>;

    async fn load_block(
        &self,
        connection: &mut StorageProcessor<'_>,
        block: BlockNumber,
    ) -> anyhow::Result<Option<Block>>;

    async fn load_account_tree_cache(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<Option<(BlockNumber, serde_json::Value)>>;

    async fn load_idle_prover_job_from_job_queue(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<Option<ProverJob>>;

    async fn record_prover_is_working(
        &self,
        connection: &mut StorageProcessor<'_>,
        job_id: i32,
        prover_name: &str,
    ) -> anyhow::Result<()>;

    async fn store_proof(
        &self,
        connection: &mut StorageProcessor<'_>,
        job_id: i32,
        block_number: BlockNumber,
        proof: &SingleProof,
    ) -> anyhow::Result<()>;

    async fn store_aggregated_proof(
        &self,
        connection: &mut StorageProcessor<'_>,
        job_id: i32,
        first_block: BlockNumber,
        last_block: BlockNumber,
        proof: &AggregatedProof,
    ) -> anyhow::Result<()>;

    async fn record_prover_stop(
        &self,
        connection: &mut StorageProcessor<'_>,
        prover_name: &str,
    ) -> anyhow::Result<()>;

    async fn load_committed_state(
        &self,
        connection: &mut StorageProcessor<'_>,
        block: Option<BlockNumber>,
    ) -> anyhow::Result<(BlockNumber, AccountMap)>;

    async fn load_state_diff(
        &self,
        connection: &mut StorageProcessor<'_>,
        from_block: BlockNumber,
        to_block: Option<BlockNumber>,
    ) -> anyhow::Result<Option<(BlockNumber, AccountUpdates)>>;

    async fn store_account_tree_cache(
        &self,
        connection: &mut StorageProcessor<'_>,
        block: BlockNumber,
        tree_cache: serde_json::Value,
    ) -> anyhow::Result<()>;

    async fn store_witness(
        &self,
        connection: &mut StorageProcessor<'_>,
        block: BlockNumber,
        witness: serde_json::Value,
    ) -> anyhow::Result<()>;

    async fn pending_jobs_count(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<u32>;
}
