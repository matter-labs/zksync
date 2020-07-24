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
use crate::prover::records::{
    MultiproofBlockItem, NewMultiblockProof, ProverMultiblockRun, StoredMultiblockProof,
};
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

    pub fn multiblock_job_exists(
        &self,
        blocks_batch_timeout_: time::Duration,
        max_block_batch_size_: usize,
    ) -> QueryResult<bool> {
        self
            .0
            .conn()
            .transaction(|| {
                let first_unverified_block_query = format!(" \
                    SELECT COALESCE(min(block_to + 1),0) AS integer_value FROM multiblock_proofs proof1 \
                        WHERE NOT EXISTS ( \
                            SELECT * FROM multiblock_proofs proof2 \
                            WHERE \
                                proof2.block_from <= proof1.block_to + 1 AND proof1.block_to + 1 <= proof2.block_to \
                        ) \
                    ",
                );

                let first_unverified_block_ = if self.load_multiblock_proof(1).is_ok() {
                    diesel::sql_query(first_unverified_block_query).get_result::<Option<IntegerNumber>>(self.0.conn())?
                        .map(|index| index.integer_value).unwrap_or_default()
                } else {
                    1
                };

                let query = format!(" \
                    WITH suitable_blocks AS ( \
                        SELECT * FROM operations o \
                        WHERE action_type = 'COMMIT' \
                            AND block_number >= '{first_unverified_block}'
                            AND EXISTS \
                                (SELECT * FROM proofs WHERE block_number = o.block_number) \
                    ) \
                    SELECT \
                        block_number, \
                        ((now() - created_at) > interval '{blocks_batch_timeout} seconds') as blocks_batch_timeout_passed, \
                        (EXISTS (SELECT * FROM multiblock_proofs WHERE block_from <= block_number AND block_to >= block_number)) as multiblock_already_generated \
                    FROM suitable_blocks \
                    order by suitable_blocks.block_number \
                    ",
                    first_unverified_block=first_unverified_block_,
                    blocks_batch_timeout=blocks_batch_timeout_.as_secs()
                );

                let blocks = diesel::sql_query(query).load::<MultiproofBlockItem>(self.0.conn())?;
                if !blocks.is_empty() {
                    let mut batch_size = 1;
                    while batch_size < max_block_batch_size_ && batch_size + 1 <= blocks.len()
                        && blocks[batch_size].block_number == blocks[batch_size - 1].block_number + 1
                        && blocks[batch_size].multiblock_already_generated == false {
                        batch_size += 1;
                    }
                    if batch_size == max_block_batch_size_ || blocks[0].blocks_batch_timeout_passed {
                        return Ok(true);
                    }
                }

                Ok(false)
            })
    }

    /// Сhooses the next multiblock to prove for the certain prover.
    /// Returns `None` if either there are no multiblocks to prove, or
    /// there is already an ongoing job for non-proved multiblocks.
    pub fn prover_multiblock_run(
        &self,
        worker_: &str,
        prover_timeout: time::Duration,
        blocks_batch_timeout_: time::Duration,
        max_block_batch_size_: usize,
    ) -> QueryResult<Option<ProverMultiblockRun>> {
        // Select multiblock to prove.
        self
            .0
            .conn()
            .transaction(|| {
                sql_query("LOCK TABLE prover_multiblock_runs IN EXCLUSIVE MODE").execute(self.0.conn())?;

                let first_unverified_block_query = format!(" \
                    SELECT COALESCE(min(block_to + 1),0) AS integer_value FROM multiblock_proofs proof1 \
                        WHERE NOT EXISTS ( \
                            SELECT * FROM multiblock_proofs proof2 \
                            WHERE \
                                proof2.block_from <= proof1.block_to + 1 AND proof1.block_to + 1 <= proof2.block_to \
                        ) \
                    ",
                );

                let first_unverified_block_ = if self.load_multiblock_proof(1).is_ok() {
                    diesel::sql_query(first_unverified_block_query).get_result::<Option<IntegerNumber>>(self.0.conn())?
                        .map(|index| index.integer_value).unwrap_or_default()
                } else {
                    1
                };

                let query = format!(" \
                    WITH suitable_blocks AS ( \
                        SELECT * FROM operations o \
                        WHERE action_type = 'COMMIT' \
                            AND block_number >= '{first_unverified_block}'
                            AND EXISTS \
                                (SELECT * FROM proofs WHERE block_number = o.block_number) \
                            AND NOT EXISTS \
                                (SELECT * FROM prover_multiblock_runs \
                                    WHERE block_number_from <= o.block_number \
                                    AND block_number_to >= o.block_number \
                                    AND (now() - updated_at) < interval '{prover_timeout_secs} seconds') \
                    ) \
                    SELECT \
                        block_number, \
                        ((now() - created_at) > interval '{blocks_batch_timeout} seconds') as blocks_batch_timeout_passed, \
                        (EXISTS (SELECT * FROM multiblock_proofs WHERE block_from <= block_number AND block_to >= block_number)) as multiblock_already_generated \
                    FROM suitable_blocks \
                    order by suitable_blocks.block_number \
                    ",
                    first_unverified_block=first_unverified_block_,
                    prover_timeout_secs=prover_timeout.as_secs(), blocks_batch_timeout=blocks_batch_timeout_.as_secs()
                );

                let blocks = diesel::sql_query(query).load::<MultiproofBlockItem>(self.0.conn())?;
                if !blocks.is_empty() {
                    let mut batch_size = 1;
                    while batch_size < max_block_batch_size_ && batch_size + 1 <= blocks.len()
                        && blocks[batch_size].block_number == blocks[batch_size - 1].block_number + 1
                        && blocks[batch_size].multiblock_already_generated == false {
                        batch_size += 1;
                    }
                    if batch_size == max_block_batch_size_ || blocks[0].blocks_batch_timeout_passed {
                        // we found a job for prover
                        use crate::schema::prover_multiblock_runs::dsl::*;
                        let inserted: ProverMultiblockRun = insert_into(prover_multiblock_runs)
                            .values(&vec![(
                                block_number_from.eq(i64::from(blocks[0].block_number)),
                                block_number_to.eq(i64::from(blocks[batch_size - 1].block_number)),
                                worker.eq(worker_.to_string()),
                            )])
                            .get_result(self.0.conn())?;
                        return Ok(Some(inserted));
                    }
                }

                Ok(None)
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

    /// Updates the state of ongoing prover multiblock job.
    pub fn record_prover_multiblock_is_working(&self, job_id: i32) -> QueryResult<()> {
        use crate::schema::prover_multiblock_runs::dsl::*;

        let target = prover_multiblock_runs.filter(id.eq(job_id));
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

    /// Stores the multiblock proof.
    pub fn store_multiblock_proof(
        &self,
        block_from: BlockNumber,
        block_to: BlockNumber,
        proof: &EncodedProofPlonk,
    ) -> QueryResult<()> {
        let to_store = NewMultiblockProof {
            block_from: i64::from(block_from),
            block_to: i64::from(block_to),
            proof: serde_json::to_value(proof).unwrap(),
        };
        use crate::schema::multiblock_proofs::dsl::multiblock_proofs;
        insert_into(multiblock_proofs)
            .values(to_store)
            .execute(self.0.conn())?;
        Ok(())
    }

    /// Gets the stored proof for a block.
    pub fn load_proof(&self, block_number: BlockNumber) -> QueryResult<EncodedProofPlonk> {
        use crate::schema::proofs::dsl;
        let stored: StoredProof = dsl::proofs
            .filter(dsl::block_number.eq(i64::from(block_number)))
            .get_result(self.0.conn())?;
        Ok(serde_json::from_value(stored.proof).unwrap())
    }

    /// Gets the stored multiblock proof.
    pub fn load_multiblock_proof(
        &self,
        block_from: BlockNumber,
    ) -> QueryResult<((BlockNumber, BlockNumber), EncodedProofPlonk)> {
        use crate::schema::multiblock_proofs::dsl;
        let stored: StoredMultiblockProof = dsl::multiblock_proofs
            .filter(dsl::block_from.eq(i64::from(block_from)))
            .get_result(self.0.conn())?;
        Ok((
            (
                stored.block_from as BlockNumber,
                stored.block_to as BlockNumber,
            ),
            serde_json::from_value(stored.proof).unwrap(),
        ))
    }
}
