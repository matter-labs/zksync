//! Module with utilities for prover scaler service.

// Workspace deps
use crate::database_interface::DatabaseInterface;
/// Scaler oracle provides information for prover scaler
/// service about required amount of provers for server
/// to operate optimally.
#[derive(Debug)]
pub struct ScalerOracle<DB: DatabaseInterface> {
    /// Database access to gather the information about amount of pending blocks.
    db: DB,

    /// Number of idle provers running for faster up-scaling.
    idle_provers: u32,
}

impl<DB: DatabaseInterface> ScalerOracle<DB> {
    pub fn new(db: DB, idle_provers: u32) -> Self {
        Self { db, idle_provers }
    }

    /// Decides how many prover entities should be created depending on the amount of pending blocks.
    pub async fn provers_required(&mut self) -> anyhow::Result<u32> {
        // Currently the logic of this method is very simple:
        // We require a prover for each pending block or IDLE_RROVERS amount if there are not so many
        // pending jobs.

        let mut storage = self.db.acquire_connection().await?;
        let pending_jobs = self.db.pending_jobs_count(&mut storage).await?;
        let provers_required = std::cmp::max(pending_jobs, self.idle_provers);

        Ok(provers_required)
    }
}
