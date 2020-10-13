// Built-in import
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
// External uses
use statrs::statistics::{Max, Median, Min, Statistics};
// Workspace uses
use zksync_types::tx::TxHash;
// Local uses

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Stats {
    pub created: u64,
    pub executed: u64,
    pub verified: u64,
    pub errored: u64,
}

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Summary {
    pub txs: Stats,
    pub ops: Stats,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct TxLifecycle {
    pub created_at: Instant,
    pub sent_at: Instant,
    pub committed_at: Instant,
    pub verified_at: Instant,
}

impl TxLifecycle {
    pub fn sending_duration(&self) -> Duration {
        self.sent_at.duration_since(self.created_at)
    }

    pub fn committing_duration(&self) -> Duration {
        self.committed_at.duration_since(self.sent_at)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Journal {
    txs: HashMap<TxHash, Result<TxLifecycle, String>>,
}

impl Journal {
    pub fn record_tx(&mut self, tx_hash: TxHash, tx_result: Result<TxLifecycle, anyhow::Error>) {
        self.txs
            .insert(tx_hash, tx_result.map_err(|e| e.to_string()));
    }

    pub fn clear(&mut self) {
        self.txs.clear()
    }

    pub fn print_summary(&self) {
        let result = self
            .txs
            .iter()
            .map(|(tx_hash, result)| match result {
                Ok(tx_lifecycle) => Ok((
                    tx_lifecycle.sending_duration().as_millis() as f64,
                    tx_lifecycle.committing_duration().as_millis() as f64,
                )),
                Err(err) => Err((*tx_hash, err.to_owned())),
            })
            .collect::<Result<Vec<(_, _)>, (TxHash, String)>>();

        match result {
            Ok(txs) => {
                let (sending, committing): (Vec<_>, Vec<_>) = txs.into_iter().unzip();

                Self::print_stats("sending", &sending);
                Self::print_stats("committing", &committing)
            }

            Err((tx_hash, err)) => log::error!(
                "An error occured while processing a transaction {}: {}",
                tx_hash.to_string(),
                err
            ),
        }
    }

    fn print_stats(category: &str, data: &[f64]) {
        debug_assert!(data.len() >= 4);

        let min = Min::min(data);
        let max = Max::max(data);
        let median = data.median();
        let std_dev = data.std_dev();

        log::info!(
            "Statistics for `{}`: min: {}ms, median: {}ms, max: {}ms, std_dev: {}ms",
            category,
            min,
            median,
            max,
            std_dev
        );

        let mut sorted_data = data.iter().map(|&x| x as u64).collect::<Vec<_>>();
        sorted_data.sort_unstable();

        let idx = sorted_data.len() - 1;

        let min = sorted_data[0];
        let lower_quartile = sorted_data[idx / 4];
        let median = sorted_data[idx / 2];
        let upper_quartile = sorted_data[idx * 3 / 4];
        let max = sorted_data[idx];

        log::info!(
            "Statistics2 for `{}`: min: {}ms, median: {}ms, max: {}ms, lower_quartile: {}ms, upper_quartile: {}ms",
            category,
            min,
            median,
            max,
            lower_quartile,
            upper_quartile,
        );
    }
}
