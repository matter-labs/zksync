// Built-in deps
use std::time;
// External imports
use diesel::dsl::{insert_into, now, sql_query};
use diesel::prelude::*;
// Workspace imports
use models::node::BlockNumber;
use models::EncodedProof;
// Local imports
use self::records::{ActiveProver, IntegerNumber, NewProof, ProverRun, StoredProof};
use crate::StorageProcessor;

pub mod records;

/// Prover schema is capable of handling the prover-related informations,
/// such as started prover jobs, registered provers and proofs for blocks.
pub struct ProverSchema<'a>(pub &'a StorageProcessor);

impl<'a> ProverSchema<'a> {
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
        let job: Option<BlockNumber> = self
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
                diesel::sql_query(query).get_result::<Option<IntegerNumber>>(self.0.conn())
            })?
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
        proof: &EncodedProof,
    ) -> QueryResult<usize> {
        let to_store = NewProof {
            block_number: i64::from(block_number),
            proof: serde_json::to_value(proof).unwrap(),
        };
        use crate::schema::proofs::dsl::proofs;
        insert_into(proofs).values(&to_store).execute(self.0.conn())
    }

    /// Gets the stored proof for a block.
    pub fn load_proof(&self, block_number: BlockNumber) -> QueryResult<EncodedProof> {
        use crate::schema::proofs::dsl;
        let stored: StoredProof = dsl::proofs
            .filter(dsl::block_number.eq(i64::from(block_number)))
            .get_result(self.0.conn())?;
        Ok(serde_json::from_value(stored.proof).unwrap())
    }
}
