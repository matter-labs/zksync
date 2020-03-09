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

pub struct ProverSchema<'a>(pub &'a StorageProcessor);

impl<'a> ProverSchema<'a> {
    pub fn prover_run_for_next_commit(
        &self,
        worker_: &str,
        prover_timeout: time::Duration,
        block_size: usize,
    ) -> QueryResult<Option<ProverRun>> {
        self.0.conn().transaction(|| {
            sql_query("LOCK TABLE prover_runs IN EXCLUSIVE MODE").execute(self.0.conn())?;
            let job: Option<BlockNumber> = diesel::sql_query(format!("
                WITH unsized_blocks AS (
                    SELECT * FROM operations o
                    WHERE action_type = 'COMMIT'
                        AND block_number >
                            (SELECT COALESCE(max(block_number),0) FROM operations WHERE action_type = 'VERIFY')
                        AND NOT EXISTS 
                            (SELECT * FROM proofs WHERE block_number = o.block_number)
                        AND NOT EXISTS
                            (SELECT * FROM prover_runs 
                                WHERE block_number = o.block_number AND (now() - updated_at) < interval '{} seconds')
                )
                SELECT min(block_number) AS integer_value FROM unsized_blocks
                INNER JOIN blocks 
                    ON unsized_blocks.block_number = blocks.number AND blocks.block_size = {}
                ", prover_timeout.as_secs(), block_size))
                .get_result::<Option<IntegerNumber>>(self.0.conn())?
                .map(|i| i.integer_value as BlockNumber);
            if let Some(block_number_) = job {
                use crate::schema::prover_runs::dsl::*;
                let inserted: ProverRun = insert_into(prover_runs)
                    .values(&vec![(
                        block_number.eq(i64::from(block_number_) ),
                        worker.eq(worker_.to_string())
                    )])
                    .get_result(self.0.conn())?;
                Ok(Some(inserted))
            } else {
                Ok(None)
            }
        })
    }

    pub fn record_prover_is_working(&self, job_id: i32) -> QueryResult<()> {
        use crate::schema::prover_runs::dsl::*;

        let target = prover_runs.filter(id.eq(job_id));
        diesel::update(target)
            .set(updated_at.eq(now))
            .execute(self.0.conn())
            .map(|_| ())
    }

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

    pub fn prover_by_id(&self, prover_id: i32) -> QueryResult<ActiveProver> {
        use crate::schema::active_provers::dsl::*;

        let ret: ActiveProver = active_provers
            .filter(id.eq(prover_id))
            .get_result(self.0.conn())?;
        Ok(ret)
    }

    pub fn record_prover_stop(&self, prover_id: i32) -> QueryResult<()> {
        use crate::schema::active_provers::dsl::*;

        let target = active_provers.filter(id.eq(prover_id));
        diesel::update(target)
            .set(stopped_at.eq(now))
            .execute(self.0.conn())
            .map(|_| ())
    }

    /// Store the timestamp of the prover finish and the proof
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

    pub fn load_proof(&self, block_number: BlockNumber) -> QueryResult<EncodedProof> {
        use crate::schema::proofs::dsl;
        let stored: StoredProof = dsl::proofs
            .filter(dsl::block_number.eq(i64::from(block_number)))
            .get_result(self.0.conn())?;
        Ok(serde_json::from_value(stored.proof).unwrap())
    }
}
