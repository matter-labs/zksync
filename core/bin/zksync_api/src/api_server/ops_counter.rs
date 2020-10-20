//! This module contains a structure for limiting the amount of operations
//! executed by an account.
//!
//! It is a quick workaround for a `ChangePubKey` operation not having
//! a fee, and thus a potential attack vector where attacker spams us
//! with free transaction which we must process for free.
//!
//! This module has to be removed once `ChangePubKey` operation has
//! a fee (task #668)

// Built-in deps.
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
// Workspace deps.
use zksync_types::{tx::ChangePubKey, AccountId};

const ONE_DAY: Duration = Duration::from_secs(60 * 60 * 24);
const MAX_OPS_PER_DAY: usize = 10;

/// `ChangePubKeyOpsCounter` counts amount of `ChangePubKey` operations
/// performed by each account per day.
///
/// Every 24 hours the counter and all the limitations are reset.
///
/// Within 24 hours, each account is allowed to perform no more than `MAX_OPS_PER_DAY`
/// ChangePubKey operations.
#[derive(Debug, Clone)]
pub struct ChangePubKeyOpsCounter {
    last_reset: Instant,
    account_ops: HashMap<AccountId, usize>,
}

impl Default for ChangePubKeyOpsCounter {
    fn default() -> Self {
        Self {
            last_reset: Instant::now(),
            account_ops: HashMap::new(),
        }
    }
}

impl ChangePubKeyOpsCounter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks whether the provided transaction should be executed or considered spam.
    pub fn check_allowanse(&mut self, tx: &ChangePubKey) -> Result<(), anyhow::Error> {
        // First, check if we have to reset all the stats.
        if self.last_reset.elapsed() >= ONE_DAY {
            // One day has passed, reset all the account stats.
            self.last_reset = Instant::now();
            self.account_ops = HashMap::new();
        }

        // Get the operations count for this account and check if it's beyond the limit.
        let account_ops_count = self
            .account_ops
            .entry(tx.account_id)
            .and_modify(|e| *e += 1)
            .or_insert(1);
        if *account_ops_count > MAX_OPS_PER_DAY {
            anyhow::bail!("Limit for ChangePubKey operations was reached for this account. Try again tomorrow");
        }
        Ok(())
    }
}
