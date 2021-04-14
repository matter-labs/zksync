// Built-in deps
use std::time::Instant;
// External imports
use anyhow::format_err;
use sqlx::Done;
// Workspace imports
use zksync_types::BlockNumber;
// Local imports
use self::records::{StorageProverJobQueue, StoredAggregatedProof, StoredProof};
use crate::chain::operations::OperationsSchema;
use crate::prover::records::StorageBlockWitness;
use crate::{QueryResult, StorageProcessor};
use zksync_crypto::proof::{AggregatedProof, SingleProof};
use zksync_types::aggregated_operations::AggregatedActionType;
use zksync_types::prover::{ProverJob, ProverJobStatus, ProverJobType};

pub mod records;

/// Prover schema is capable of handling the prover-related informations,
/// such as started prover jobs, registered provers and proofs for blocks.
#[derive(Debug)]
pub struct ProverSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> ProverSchema<'a, 'c> {
    /// Returns the amount of blocks which await for proof, but have
    /// no assigned prover run.
    pub async fn pending_jobs_count(&mut self) -> QueryResult<u32> {
        let start = Instant::now();
        let pending_jobs_count = sqlx::query!(
            "SELECT COUNT(*) FROM prover_job_queue WHERE job_status != $1",
            ProverJobStatus::Done.to_number()
        )
        .fetch_one(self.0.conn())
        .await?
        .count
        .unwrap_or(0) as u32;
        metrics::histogram!("sql", start.elapsed(), "prover" => "pending_jobs_count");
        Ok(pending_jobs_count)
    }

    pub async fn add_prover_job_to_job_queue(
        &mut self,
        first_block: BlockNumber,
        last_block: BlockNumber,
        job_data: serde_json::Value,
        job_priority: i32,
        job_type: ProverJobType,
    ) -> QueryResult<()> {
        sqlx::query!(
        "
          WITH job_values as (
            SELECT $1::int4, $2::int4, $3::text, 'server_add_job', $4::int8, $5::int8, $6::jsonb
            WHERE NOT EXISTS (SELECT * FROM prover_job_queue WHERE first_block = $4 and last_block = $5 and job_type = $3 LIMIT 1)
          )
          INSERT INTO prover_job_queue (job_status, job_priority, job_type, updated_by, first_block, last_block, job_data)
          SELECT * from job_values
        ",
            ProverJobStatus::Idle.to_number(),
            job_priority,
            job_type.to_string(),
            i64::from(*first_block),
            i64::from(*last_block),
            job_data,
        ).execute(self.0.conn()).await?;
        Ok(())
    }

    pub async fn mark_stale_jobs_as_idle(&mut self) -> QueryResult<()> {
        sqlx::query!(
            "UPDATE prover_job_queue SET (job_status, updated_at, updated_by) = ($1, now(), 'server_clean_idle')
            WHERE job_status = $2 and (now() - updated_at) >= interval '120 seconds'",
            ProverJobStatus::Idle.to_number(),
            ProverJobStatus::InProgress.to_number(),
        )
        .execute(self.0.conn())
        .await?;
        Ok(())
    }

    pub async fn get_idle_prover_job_from_job_queue(&mut self) -> QueryResult<Option<ProverJob>> {
        let start = Instant::now();
        // Select the block to prove.
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!("LOCK TABLE prover_job_queue IN EXCLUSIVE MODE")
            .execute(transaction.conn())
            .await?;

        let prover_job_queue = sqlx::query_as!(
            StorageProverJobQueue,
            r#"
                SELECT * FROM prover_job_queue
                WHERE job_status = $1
                ORDER BY (job_priority, id, first_block)
                LIMIT 1
            "#,
            ProverJobStatus::Idle.to_number()
        )
        .fetch_optional(transaction.conn())
        .await?;

        let prover_job = if let Some(job) = prover_job_queue {
            sqlx::query!(
                r#"
                UPDATE prover_job_queue
                SET (job_status, updated_at, updated_by) = ($1, now(), 'server_give_job')
                WHERE id = $2;
            "#,
                ProverJobStatus::InProgress.to_number(),
                job.id,
            )
            .execute(transaction.conn())
            .await?;

            Some(ProverJob::new(
                job.id,
                BlockNumber(job.first_block as u32),
                BlockNumber(job.last_block as u32),
                job.job_data,
            ))
        } else {
            None
        };
        transaction.commit().await?;
        metrics::histogram!("sql", start.elapsed(), "prover" => "get_idle_prover_job_from_job_queue");
        Ok(prover_job)
    }

    /// Updates the state of ongoing prover job.
    pub async fn record_prover_is_working(
        &mut self,
        job_id: i32,
        prover_name: &str,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "UPDATE prover_job_queue
            SET (updated_at, updated_by) = (now(), $1)
            WHERE id = $2",
            prover_name.to_string(),
            job_id,
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "record_prover_is_working");
        Ok(())
    }

    /// Marks the prover as stopped.
    pub async fn record_prover_stop(&mut self, prover_name: &str) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "UPDATE prover_job_queue
            SET (updated_at, job_status) = (now(), $1)
            WHERE updated_by = $2 and job_status = $3",
            ProverJobStatus::Idle.to_number(),
            prover_name,
            ProverJobStatus::InProgress.to_number()
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "record_prover_stop");
        Ok(())
    }

    /// Stores the proof for a block.
    pub async fn store_proof(
        &mut self,
        job_id: i32,
        block_number: BlockNumber,
        proof: &SingleProof,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        let updated_rows = sqlx::query!(
            "UPDATE prover_job_queue
            SET (updated_at, job_status, updated_by) = (now(), $1, 'server_finish_job')
            WHERE id = $2 AND job_type = $3",
            ProverJobStatus::Done.to_number(),
            job_id,
            ProverJobType::SingleProof.to_string()
        )
        .execute(transaction.conn())
        .await?
        .rows_affected();

        if updated_rows != 1 {
            return Err(format_err!("Missing job for stored proof"));
        }

        sqlx::query!(
            "INSERT INTO proofs (block_number, proof)
            VALUES ($1, $2)",
            i64::from(*block_number),
            serde_json::to_value(proof).unwrap()
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "store_proof");
        Ok(())
    }

    /// Stores the aggregated proof for blocks.
    pub async fn store_aggregated_proof(
        &mut self,
        job_id: i32,
        first_block: BlockNumber,
        last_block: BlockNumber,
        proof: &AggregatedProof,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        let updated_rows = sqlx::query!(
            "UPDATE prover_job_queue
            SET (updated_at, job_status, updated_by) = (now(), $1, 'server_finish_job')
            WHERE id = $2 AND job_type = $3",
            ProverJobStatus::Done.to_number(),
            job_id,
            ProverJobType::AggregatedProof.to_string()
        )
        .execute(transaction.conn())
        .await?
        .rows_affected() as usize;

        if updated_rows != 1 {
            return Err(format_err!("Missing job for stored aggregated proof"));
        }

        sqlx::query!(
            "INSERT INTO aggregated_proofs (first_block, last_block, proof)
            VALUES ($1, $2, $3)",
            i64::from(*first_block),
            i64::from(*last_block),
            serde_json::to_value(proof).unwrap()
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "store_aggregated_proof");
        Ok(())
    }

    /// Gets the stored proof for a block.
    pub async fn load_proof(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<Option<SingleProof>> {
        let start = Instant::now();
        let proof = sqlx::query_as!(
            StoredProof,
            "SELECT * FROM proofs WHERE block_number = $1",
            i64::from(*block_number),
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|stored| serde_json::from_value(stored.proof).unwrap());

        metrics::histogram!("sql", start.elapsed(), "prover" => "load_proof");
        Ok(proof)
    }

    /// Gets the stored proof for a block.
    pub async fn load_aggregated_proof(
        &mut self,
        first_block: BlockNumber,
        last_block: BlockNumber,
    ) -> QueryResult<Option<AggregatedProof>> {
        let start = Instant::now();
        let proof = sqlx::query_as!(
            StoredAggregatedProof,
            "SELECT * FROM aggregated_proofs WHERE first_block = $1 and last_block = $2",
            i64::from(*first_block),
            i64::from(*last_block)
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|stored| serde_json::from_value(stored.proof).unwrap());

        metrics::histogram!("sql", start.elapsed(), "prover" => "load_aggregated_proof");
        Ok(proof)
    }

    /// Stores witness for a block
    pub async fn store_witness(
        &mut self,
        block: BlockNumber,
        witness: serde_json::Value,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let witness_str = serde_json::to_string(&witness).expect("Failed to serialize witness");
        sqlx::query!(
            "INSERT INTO block_witness (block, witness)
            VALUES ($1, $2)
            ON CONFLICT (block)
            DO NOTHING",
            i64::from(*block),
            witness_str
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "store_witness");
        Ok(())
    }

    /// Gets stored witness for a block.
    pub async fn get_witness(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<Option<serde_json::Value>> {
        let start = Instant::now();
        let block_witness = sqlx::query_as!(
            StorageBlockWitness,
            "SELECT * FROM block_witness WHERE block = $1",
            i64::from(*block_number),
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "get_witness");
        Ok(block_witness
            .map(|w| serde_json::from_str(&w.witness).expect("Failed to deserialize witness")))
    }

    pub async fn get_last_block_prover_job_queue(
        &mut self,
        action_type: ProverJobType,
    ) -> QueryResult<BlockNumber> {
        let last_block = sqlx::query!(
            "SELECT max(last_block) from prover_job_queue
            WHERE job_type = $1",
            action_type.to_string(),
        )
        .fetch_one(self.0.conn())
        .await?
        .max;

        let result = if let Some(last_block) = last_block {
            BlockNumber(last_block as u32)
        } else {
            // this branch executes when prover job queue is empty
            match action_type {
                ProverJobType::SingleProof => {
                    OperationsSchema(self.0)
                        .get_last_block_by_aggregated_action(
                            AggregatedActionType::CreateProofBlocks,
                            None,
                        )
                        .await?
                }
                ProverJobType::AggregatedProof => {
                    OperationsSchema(self.0)
                        .get_last_block_by_aggregated_action(
                            AggregatedActionType::PublishProofBlocksOnchain,
                            None,
                        )
                        .await?
                }
            }
        };

        Ok(result)
    }

    // Removes witnesses for blocks with number greater than `last_block`
    pub async fn remove_witnesses(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM block_witness WHERE block > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "remove_witnesses");
        Ok(())
    }

    // Removes proofs for blocks with number greater than `last_block`
    pub async fn remove_proofs(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM proofs WHERE block_number > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "remove_proofs");
        Ok(())
    }

    // Removes aggregated proofs for blocks with number greater than `last_block`
    pub async fn remove_aggregated_proofs(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM aggregated_proofs WHERE last_block > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "remove_aggregated_proofs");
        Ok(())
    }

    // Removes blocks with number greater than `last_block` from prover job queue
    pub async fn remove_prover_jobs(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;
        sqlx::query!(
            "DELETE FROM prover_job_queue WHERE first_block > $1",
            *last_block as i64
        )
        .execute(transaction.conn())
        .await?;

        sqlx::query!(
            "UPDATE prover_job_queue SET last_block = $1 WHERE last_block > $1",
            *last_block as i64
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;

        metrics::histogram!("sql", start.elapsed(), "prover" => "remove_prover_jobs");
        Ok(())
    }
}
