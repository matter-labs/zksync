// Built-in deps
use std::time;
// External imports
use sqlx::Done;
// Workspace imports
use zksync_crypto::proof::EncodedProofPlonk;
use zksync_types::BlockNumber;
// Local imports
use self::records::{ActiveProver, ProverRun, StoredProof};
use crate::prover::records::StorageBlockWitness;
use crate::{chain::block::BlockSchema, QueryResult, StorageProcessor};

pub mod records;

/// Prover schema is capable of handling the prover-related informations,
/// such as started prover jobs, registered provers and proofs for blocks.
#[derive(Debug)]
pub struct ProverSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> ProverSchema<'a, 'c> {
    /// Returns the amount of blocks which await for proof, but have
    /// no assigned prover run.
    pub async fn unstarted_jobs_count(&mut self) -> QueryResult<u64> {
        let mut transaction = self.0.start_transaction().await?;

        let mut last_committed_block = BlockSchema(&mut transaction)
            .get_last_committed_block()
            .await? as u64;

        if BlockSchema(&mut transaction).pending_block_exists().await? {
            // Existence of the pending block means that soon there will be one more block.
            last_committed_block += 1;
        }

        let last_verified_block = BlockSchema(&mut transaction)
            .get_last_verified_block()
            .await? as u64;

        let num_ongoing_jobs = sqlx::query!(
            "SELECT COUNT(*) FROM prover_runs WHERE block_number > $1",
            last_verified_block as i64
        )
        .fetch_one(transaction.conn())
        .await?
        .count
        .unwrap_or(0) as u64;

        assert!(
            last_verified_block + num_ongoing_jobs <= last_committed_block,
            "There are more ongoing prover jobs than blocks without proofs. \
                Last verifier block: {}, last committed block: {}, amount of ongoing \
                prover runs: {}",
            last_verified_block,
            last_committed_block,
            num_ongoing_jobs,
        );

        let result = last_committed_block - (last_verified_block + num_ongoing_jobs);

        transaction.commit().await?;
        Ok(result)
    }

    /// Returns the amount of blocks which await for proof (committed but not verified)
    pub async fn pending_jobs_count(&mut self) -> QueryResult<u32> {
        let block_without_proofs = sqlx::query!(
                "\
            SELECT COUNT(*) as integer_value FROM operations o \
               WHERE action_type = 'COMMIT' \
                   AND block_number > \
                       (SELECT COALESCE(max(block_number),0) FROM operations WHERE action_type = 'VERIFY') \
                   AND EXISTS \
                       (SELECT * FROM block_witness WHERE block = o.block_number) \
                   AND NOT EXISTS \
                       (SELECT * FROM proofs WHERE block_number = o.block_number);"
            )
            .fetch_one(self.0.conn())
            .await?
            .integer_value
            .unwrap_or(0) as u64;

        Ok(block_without_proofs as u32)
    }

    /// Attempts to obtain an existing prover run given block number.
    pub async fn get_existing_prover_run(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<Option<ProverRun>> {
        let prover_run = sqlx::query_as!(
            ProverRun,
            "SELECT * FROM prover_runs WHERE block_number = $1",
            i64::from(block_number),
        )
        .fetch_optional(self.0.conn())
        .await?;

        Ok(prover_run)
    }

    /// Given the block size, chooses the next block to prove for the certain prover.
    /// Returns `None` if either there are no blocks of given size to prove, or
    /// there is already an ongoing job for non-proved block.
    pub async fn prover_run_for_next_commit(
        &mut self,
        worker_: &str,
        _prover_timeout: time::Duration,
        block_size: usize,
    ) -> QueryResult<Option<ProverRun>> {
        // Select the block to prove.
        let mut transaction = self.0.start_transaction().await?;

        sqlx::query!("LOCK TABLE prover_runs IN EXCLUSIVE MODE")
            .execute(transaction.conn())
            .await?;

        // Find the block that satisfies the following criteria:
        // - Block number is greater than the index of last verified block.
        // - There is no proof for block.
        // - Either there is no ongoing job for the block, or the job exceeded the timeout.
        // Return the index of such a block.

        // TODO: Prover gone interval is hard-coded. Is it critical?
        let job = sqlx::query!(
            r#"
                WITH unsized_blocks AS (
                    SELECT * FROM operations o
                    WHERE action_type = 'COMMIT'
                        AND block_number >
                            (SELECT COALESCE(max(block_number),0) FROM operations WHERE action_type = 'VERIFY')
                        AND NOT EXISTS
                            (SELECT * FROM proofs WHERE block_number = o.block_number)
                        AND NOT EXISTS
                            (SELECT * FROM prover_runs
                                WHERE block_number = o.block_number AND (now() - updated_at) < interval '120 seconds')
                )
                SELECT min(block_number) FROM unsized_blocks
                INNER JOIN blocks
                    ON unsized_blocks.block_number = blocks.number AND blocks.block_size = $1
            "#,
            block_size as i64
            )
            .fetch_one(transaction.conn())
            .await?
            .min;

        // If there is a block to prove, create a job and store it
        // in the `prover_runs` table; otherwise do nothing and return `None`.
        let result = if let Some(block_number) = job {
            let inserted_id = sqlx::query!(
                r#"
                INSERT INTO prover_runs ( block_number, worker )
                VALUES ( $1, $2 )
                RETURNING (id)
                "#,
                block_number,
                worker_.to_string(),
            )
            .fetch_one(transaction.conn())
            .await?
            .id;

            let prover_run = sqlx::query_as!(
                ProverRun,
                "SELECT * FROM prover_runs WHERE id = $1",
                inserted_id
            )
            .fetch_one(transaction.conn())
            .await?;

            Some(prover_run)
        } else {
            None
        };

        transaction.commit().await?;

        Ok(result)
    }

    /// Updates the state of ongoing prover job.
    pub async fn record_prover_is_working(&mut self, job_id: i32) -> QueryResult<()> {
        sqlx::query!(
            "UPDATE prover_runs 
            SET updated_at = now()
            WHERE id = $1",
            job_id
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    /// Adds a prover to the database.
    pub async fn register_prover(&mut self, worker_: &str, block_size_: usize) -> QueryResult<i32> {
        let inserted_id = sqlx::query!(
            "INSERT INTO active_provers (worker, block_size)
            VALUES ($1, $2)
            RETURNING id",
            worker_.to_string(),
            block_size_ as i64
        )
        .fetch_one(self.0.conn())
        .await?
        .id;

        Ok(inserted_id)
    }

    /// Gets a prover descriptor by its numeric ID.
    pub async fn prover_by_id(&mut self, prover_id: i32) -> QueryResult<ActiveProver> {
        let prover = sqlx::query_as!(
            ActiveProver,
            "SELECT * FROM active_provers WHERE id = $1",
            prover_id
        )
        .fetch_one(self.0.conn())
        .await?;

        Ok(prover)
    }

    /// Marks the prover as stopped.
    pub async fn record_prover_stop(&mut self, prover_id: i32) -> QueryResult<()> {
        // FIXME(popzxc): It seems that it isn't actually checked if the prover has been stopped
        // anywhere. And also it doesn't seem that prover can be restored from the stopped
        // state.
        sqlx::query!(
            "UPDATE active_provers 
            SET stopped_at = now()
            WHERE id = $1",
            prover_id
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    /// Stores the proof for a block.
    pub async fn store_proof(
        &mut self,
        block_number: BlockNumber,
        proof: &EncodedProofPlonk,
    ) -> QueryResult<usize> {
        let updated_rows = sqlx::query!(
            "INSERT INTO proofs (block_number, proof)
            VALUES ($1, $2)",
            i64::from(block_number),
            serde_json::to_value(proof).unwrap()
        )
        .execute(self.0.conn())
        .await?
        .rows_affected() as usize;

        Ok(updated_rows)
    }

    /// Gets the stored proof for a block.
    pub async fn load_proof(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<Option<EncodedProofPlonk>> {
        let proof = sqlx::query_as!(
            StoredProof,
            "SELECT * FROM proofs WHERE block_number = $1",
            i64::from(block_number),
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|stored| serde_json::from_value(stored.proof).unwrap());

        Ok(proof)
    }

    /// Stores witness for a block
    pub async fn store_witness(
        &mut self,
        block: BlockNumber,
        witness: serde_json::Value,
    ) -> QueryResult<()> {
        let witness_str = serde_json::to_string(&witness).expect("Failed to serialize witness");
        sqlx::query!(
            "INSERT INTO block_witness (block, witness)
            VALUES ($1, $2)
            ON CONFLICT (block)
            DO NOTHING",
            i64::from(block),
            witness_str
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    /// Gets stored witness for a block
    pub async fn get_witness(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<Option<serde_json::Value>> {
        let block_witness = sqlx::query_as!(
            StorageBlockWitness,
            "SELECT * FROM block_witness WHERE block = $1",
            i64::from(block_number),
        )
        .fetch_optional(self.0.conn())
        .await?;

        Ok(block_witness
            .map(|w| serde_json::from_str(&w.witness).expect("Failed to deserialize witness")))
    }
}
