// Built-in deps
use std::time;
// External imports
use diesel::{
    dsl::{insert_into, now, sql_query},
    prelude::*,
};
// Workspace imports
use models::node::BlockNumber;
use models::prover_utils::EncodedProofPlonk;
// Local imports
use self::records::{ActiveProver, IntegerNumber, NewProof, ProverRun, StoredProof};
use crate::prover::records::StorageBlockWitness;
use crate::{chain::block::BlockSchema, StorageProcessor};

pub mod records;

/// Prover schema is capable of handling the prover-related informations,
/// such as started prover jobs, registered provers and proofs for blocks.
#[derive(Debug)]
pub struct ProverSchema<'a>(pub &'a StorageProcessor);

impl<'a> ProverSchema<'a> {
    /// Returns the amount of blocks which await for proof, but have
    /// no assigned prover run.
    pub fn unstarted_jobs_count(&self) -> QueryResult<u64> {
        use crate::schema::prover_runs::dsl::*;

        self.0.conn().transaction(|| {
            let mut last_committed_block = BlockSchema(&self.0).get_last_committed_block()? as u64;

            if BlockSchema(&self.0).pending_block_exists()? {
                // Existence of the pending block means that soon there will be one more block.
                last_committed_block += 1;
            }

            let last_verified_block = BlockSchema(&self.0).get_last_verified_block()? as u64;

            let num_ongoing_jobs = prover_runs
                .filter(block_number.gt(last_verified_block as i64))
                .count()
                .first::<i64>(self.0.conn())? as u64;

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

            Ok(result)
        })
    }

    /// Returns the amount of blocks which await for proof (committed but not verified)
    pub fn pending_jobs_count(&self) -> QueryResult<u32> {
        self.0.conn().transaction(|| {
            let query = "\
            SELECT COUNT(*) as integer_value FROM operations o \
               WHERE action_type = 'COMMIT' \
                   AND block_number > \
                       (SELECT COALESCE(max(block_number),0) FROM operations WHERE action_type = 'VERIFY') \
                   AND NOT EXISTS \
                       (SELECT * FROM proofs WHERE block_number = o.block_number);";

            let block_without_proofs = diesel::sql_query(query).get_result::<IntegerNumber>(self.0.conn())?;
            Ok(block_without_proofs.integer_value as u32)
        })
    }

    /// Given the block size, chooses the next block to prove for the certain prover.
    /// Returns `None` if either there are no blocks of given size to prove, or
    /// there is already an ongoing job for non-proved block.
    pub fn prover_run_for_next_commit(
        &self,
        worker_: &str,
        prover_timeout: time::Duration,
        block_size: usize,
    ) -> QueryResult<Option<ProverRun>> {
        // Select the block to prove.
        self
            .0
            .conn()
            .transaction(|| {
                sql_query("LOCK TABLE prover_runs IN EXCLUSIVE MODE").execute(self.0.conn())?;

                // Find the block that satisfies the following criteria:
                // - Block number is greater than the index of last verified block.
                // - There is no proof for block.
                // - Either there is no ongoing job for the block, or the job exceeded the timeout.
                let query = format!(" \
                    WITH unsized_blocks AS ( \
                        SELECT * FROM operations o \
                        WHERE action_type = 'COMMIT' \
                            AND block_number > \
                                (SELECT COALESCE(max(block_number),0) FROM operations WHERE action_type = 'VERIFY') \
                            AND NOT EXISTS \
                                (SELECT * FROM proofs WHERE block_number = o.block_number) \
                            AND NOT EXISTS \
                                (SELECT * FROM prover_runs \
                                    WHERE block_number = o.block_number AND (now() - updated_at) < interval '{prover_timeout_secs} seconds') \
                    ) \
                    SELECT min(block_number) AS integer_value FROM unsized_blocks \
                    INNER JOIN blocks \
                        ON unsized_blocks.block_number = blocks.number AND blocks.block_size = {block_size} \
                    ",
                    prover_timeout_secs=prover_timeout.as_secs(), block_size=block_size
                );

                // Return the index of such a block.
                let job = diesel::sql_query(query).get_result::<Option<IntegerNumber>>(self.0.conn())?
                .map(|i| i.integer_value as BlockNumber);

                // If there is a block to prove, create a job and store it
                // in the `prover_runs` table; otherwise do nothing and return `None`.
                if let Some(block_number_) = job {
                    use crate::schema::prover_runs::dsl::*;
                    let inserted: ProverRun = insert_into(prover_runs)
                        .values(&vec![(
                            block_number.eq(i64::from(block_number_)),
                            worker.eq(worker_.to_string()),
                        )])
                        .get_result(self.0.conn())?;
                    Ok(Some(inserted))
                } else {
                    Ok(None)
                }
            })
    }

    /// Updates the state of ongoing prover job.
    pub fn record_prover_is_working(&self, job_id: i32) -> QueryResult<()> {
        use crate::schema::prover_runs::dsl::*;

        let target = prover_runs.filter(id.eq(job_id));
        diesel::update(target)
            .set(updated_at.eq(now))
            .execute(self.0.conn())
            .map(|_| ())
    }

    /// Adds a prover to the database.
    pub fn register_prover(&self, worker_: &str, block_size_: usize) -> QueryResult<i32> {
        use crate::schema::active_provers::dsl::*;
        let inserted: ActiveProver = insert_into(active_provers)
            .values(&vec![(
                worker.eq(worker_.to_string()),
                block_size.eq(block_size_ as i64),
            )])
            .get_result(self.0.conn())?;
        Ok(inserted.id)
    }

    /// Gets a prover descriptor by its numeric ID.
    pub fn prover_by_id(&self, prover_id: i32) -> QueryResult<ActiveProver> {
        use crate::schema::active_provers::dsl::*;

        let ret: ActiveProver = active_provers
            .filter(id.eq(prover_id))
            .get_result(self.0.conn())?;
        Ok(ret)
    }

    /// Marks the prover as stopped.
    pub fn record_prover_stop(&self, prover_id: i32) -> QueryResult<()> {
        // FIXME(popzxc): It seems that it isn't actually checked if the prover has been stopped
        // anywhere. And also it doesn't seem that prover can be restored from the stopped
        // state.

        use crate::schema::active_provers::dsl::*;

        let target = active_provers.filter(id.eq(prover_id));
        diesel::update(target)
            .set(stopped_at.eq(now))
            .execute(self.0.conn())
            .map(|_| ())
    }

    /// Stores the proof for a block.
    pub fn store_proof(
        &self,
        block_number: BlockNumber,
        proof: &EncodedProofPlonk,
    ) -> QueryResult<usize> {
        let to_store = NewProof {
            block_number: i64::from(block_number),
            proof: serde_json::to_value(proof).unwrap(),
        };
        use crate::schema::proofs::dsl::proofs;
        insert_into(proofs).values(&to_store).execute(self.0.conn())
    }

    /// Gets the stored proof for a block.
    pub fn load_proof(&self, block_number: BlockNumber) -> QueryResult<EncodedProofPlonk> {
        use crate::schema::proofs::dsl;
        let stored: StoredProof = dsl::proofs
            .filter(dsl::block_number.eq(i64::from(block_number)))
            .get_result(self.0.conn())?;
        Ok(serde_json::from_value(stored.proof).unwrap())
    }

    /// Stores witness for a block
    pub fn store_witness(&self, block: BlockNumber, witness: serde_json::Value) -> QueryResult<()> {
        use crate::schema::*;

        insert_into(block_witness::table)
            .values(&StorageBlockWitness {
                block: block as i64,
                witness,
            })
            .on_conflict(block_witness::block)
            .do_nothing()
            .execute(self.0.conn())
            .map(drop)
    }

    /// Gets stored witness for a block
    pub fn get_witness(&self, block_number: BlockNumber) -> QueryResult<Option<serde_json::Value>> {
        use crate::schema::*;
        let block_witness = block_witness::table
            .filter(block_witness::block.eq(block_number as i64))
            .first::<StorageBlockWitness>(self.0.conn())
            .optional()?;

        Ok(block_witness.map(|w| w.witness))
    }
}
