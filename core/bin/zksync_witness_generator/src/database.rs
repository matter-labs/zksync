//! Module encapsulating the database interaction.
//! The essential part of this module is the trait that abstracts
//! the database interaction, so no real database is needed to run
//! the prover-server, which is required for tests.

// Built-in
use std::clone::Clone;
// Workspace uses
use zksync_crypto::proof::{AggregatedProof, SingleProof};
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::{
    aggregated_operations::{AggregatedActionType, AggregatedOperation},
    block::Block,
    prover::{ProverJob, ProverJobType},
    AccountMap, AccountUpdates, BlockNumber,
};
// Local uses
use crate::DatabaseInterface;

/// The actual database wrapper.
/// This structure uses `StorageProcessor` to interact with an existing database.
#[derive(Debug, Clone)]
pub struct Database {
    /// Connection to the database.
    db_pool: ConnectionPool,
}

impl Database {
    pub fn new(db_pool: ConnectionPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait::async_trait]
impl DatabaseInterface for Database {
    async fn acquire_connection(&self) -> anyhow::Result<StorageProcessor<'_>> {
        let connection = self.db_pool.access_storage().await?;

        Ok(connection)
    }

    async fn load_last_block_prover_job_queue(
        &self,
        connection: &mut StorageProcessor<'_>,
        job_type: ProverJobType,
    ) -> anyhow::Result<BlockNumber> {
        let result = connection
            .prover_schema()
            .get_last_block_prover_job_queue(job_type)
            .await?;

        Ok(result)
    }

    async fn load_witness(
        &self,
        connection: &mut StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let witness = connection.prover_schema().get_witness(block_number).await?;

        Ok(witness)
    }

    async fn add_prover_job_to_job_queue(
        &self,
        connection: &mut StorageProcessor<'_>,
        first_block: BlockNumber,
        last_block: BlockNumber,
        job_data: serde_json::Value,
        job_priority: i32,
        job_type: ProverJobType,
    ) -> anyhow::Result<()> {
        connection
            .prover_schema()
            .add_prover_job_to_job_queue(first_block, last_block, job_data, job_priority, job_type)
            .await?;

        Ok(())
    }

    async fn load_aggregated_op_that_affects_block(
        &self,
        connection: &mut StorageProcessor<'_>,
        aggregated_action: AggregatedActionType,
        block_number: BlockNumber,
    ) -> anyhow::Result<Option<(i64, AggregatedOperation)>> {
        let op = connection
            .chain()
            .operations_schema()
            .get_aggregated_op_that_affects_block(aggregated_action, block_number)
            .await?;

        Ok(op)
    }

    async fn load_proof(
        &self,
        connection: &mut StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> anyhow::Result<Option<SingleProof>> {
        let proof = connection.prover_schema().load_proof(block_number).await?;

        Ok(proof)
    }

    async fn mark_stale_jobs_as_idle(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<()> {
        connection.prover_schema().mark_stale_jobs_as_idle().await?;

        Ok(())
    }

    async fn load_last_verified_block(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<BlockNumber> {
        let block = connection
            .chain()
            .block_schema()
            .get_last_verified_block()
            .await?;

        Ok(block)
    }

    async fn load_block(
        &self,
        connection: &mut StorageProcessor<'_>,
        block: BlockNumber,
    ) -> anyhow::Result<Option<Block>> {
        let block = connection.chain().block_schema().get_block(block).await?;

        Ok(block)
    }

    async fn load_account_tree_cache(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<Option<(BlockNumber, serde_json::Value)>> {
        let tree_cache = connection
            .chain()
            .block_schema()
            .get_account_tree_cache()
            .await?;

        Ok(tree_cache)
    }

    async fn load_idle_prover_job_from_job_queue(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<Option<ProverJob>> {
        let proof = connection
            .prover_schema()
            .get_idle_prover_job_from_job_queue()
            .await?;

        Ok(proof)
    }

    async fn record_prover_is_working(
        &self,
        connection: &mut StorageProcessor<'_>,
        job_id: i32,
        prover_name: &str,
    ) -> anyhow::Result<()> {
        connection
            .prover_schema()
            .record_prover_is_working(job_id, prover_name)
            .await?;

        Ok(())
    }

    async fn store_proof(
        &self,
        connection: &mut StorageProcessor<'_>,
        job_id: i32,
        block_number: BlockNumber,
        proof: &SingleProof,
    ) -> anyhow::Result<()> {
        connection
            .prover_schema()
            .store_proof(job_id, block_number, proof)
            .await?;

        Ok(())
    }

    async fn store_aggregated_proof(
        &self,
        connection: &mut StorageProcessor<'_>,
        job_id: i32,
        first_block: BlockNumber,
        last_block: BlockNumber,
        proof: &AggregatedProof,
    ) -> anyhow::Result<()> {
        connection
            .prover_schema()
            .store_aggregated_proof(job_id, first_block, last_block, proof)
            .await?;

        Ok(())
    }

    async fn record_prover_stop(
        &self,
        connection: &mut StorageProcessor<'_>,
        prover_name: &str,
    ) -> anyhow::Result<()> {
        connection
            .prover_schema()
            .record_prover_stop(prover_name)
            .await?;

        Ok(())
    }

    async fn load_committed_state(
        &self,
        connection: &mut StorageProcessor<'_>,
        block: Option<BlockNumber>,
    ) -> anyhow::Result<(BlockNumber, AccountMap)> {
        let result = connection
            .chain()
            .state_schema()
            .load_committed_state(block)
            .await?;

        Ok(result)
    }

    async fn load_state_diff(
        &self,
        connection: &mut StorageProcessor<'_>,
        from_block: BlockNumber,
        to_block: Option<BlockNumber>,
    ) -> anyhow::Result<Option<(BlockNumber, AccountUpdates)>> {
        let result = connection
            .chain()
            .state_schema()
            .load_state_diff(from_block, to_block)
            .await?;

        Ok(result)
    }

    async fn store_account_tree_cache(
        &self,
        connection: &mut StorageProcessor<'_>,
        block: BlockNumber,
        tree_cache: serde_json::Value,
    ) -> anyhow::Result<()> {
        connection
            .chain()
            .block_schema()
            .store_account_tree_cache(block, tree_cache)
            .await?;

        Ok(())
    }

    async fn store_witness(
        &self,
        connection: &mut StorageProcessor<'_>,
        block: BlockNumber,
        witness: serde_json::Value,
    ) -> anyhow::Result<()> {
        connection
            .prover_schema()
            .store_witness(block, witness)
            .await?;

        Ok(())
    }

    async fn pending_jobs_count(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<u32> {
        let count = connection.prover_schema().pending_jobs_count().await?;

        Ok(count)
    }
}
