//! Module with utilities for prover scaler service.

// Built-in deps
use std::time::{Duration, Instant};
// Workspace deps
use storage::ConnectionPool;

/// Disable the prover service after 5 minutes with no blocks to generate.
const PROVER_DISABLE_THRESHOLD: Duration = Duration::from_secs(5 * 60);

/// Scaler oracle provides information for prover scaler
/// service about required amount of provers for server
/// to operate optimally.
pub struct ScalerOracle {
    /// Last moment in time when a block for proving was
    /// available.
    ///
    /// As shutting the prover down is an expensive operation,
    /// we don't want to do it every time we have no blocks.
    /// Instead, we wait for some time to ensure that load level
    /// decreased, and only then report the scaler that it should
    /// reduce amount of provers.
    last_time_with_blocks: Instant,

    /// Database access to gather the information about amount of
    /// pending blocks.
    db: ConnectionPool,
}

impl ScalerOracle {
    pub fn new(db: ConnectionPool) -> Self {
        Self {
            last_time_with_blocks: Instant::now(),
            db,
        }
    }

    pub fn provers_required(&mut self, working_provers_count: u32) -> Result<u32, failure::Error> {
        let storage = self.db.access_storage()?;
        let jobs = storage.prover_schema().unstarted_jobs_count()?;

        if jobs != 0 {
            self.last_time_with_blocks = Instant::now();
        }

        if working_provers_count == 0 && jobs == 0 {
            // No provers active, no jobs as well => no need to start one.
            return Ok(0);
        }

        let provers_required = if self.last_time_with_blocks.elapsed() >= PROVER_DISABLE_THRESHOLD {
            // Long time no blocks => shutdown prover.
            0
        } else {
            // Either a new block has appeared or not so long without blocks => start/retain prover.
            1
        };

        Ok(provers_required)
    }
}
