//! Module with utilities for prover scaler service.

// Workspace deps
use storage::ConnectionPool;

/// Scaler oracle provides information for prover scaler
/// service about required amount of provers for server
/// to operate optimally.
pub struct ScalerOracle {
    /// Database access to gather the information about amount of pending blocks.
    db: ConnectionPool,

    /// Number of idle provers running for faster up-scaling
    idle_provers: u32,
}

impl ScalerOracle {
    pub fn new(db: ConnectionPool, idle_provers: u32) -> Self {
        Self { db, idle_provers }
    }

    /// Decides how many prover entities should be created depending on the amount of pending blocks.
    pub fn provers_required(&mut self) -> Result<u32, failure::Error> {
        // Currently the logic of this method is very simple:
        // We require a prover for each pending block plus IDLE_RROVERS for faster upscaling.

        let storage = self.db.access_storage()?;
        let pending_jobs = storage.prover_schema().pending_jobs_count()?;
        let provers_required = pending_jobs + self.idle_provers;

        Ok(provers_required)
    }
}
