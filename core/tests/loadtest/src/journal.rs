// Built-in import
use std::{
    cmp::{max, min},
    collections::HashMap,
    time::{Duration, Instant},
};
// External uses
use num::Num;
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
                    tx_lifecycle.sending_duration().as_millis(),
                    tx_lifecycle.committing_duration().as_millis(),
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

    fn print_stats(category: &str, data: &[u128]) {
        debug_assert!(data.len() >= 4);

        let data2 = data.iter().map(|&x| x as f64).collect::<Vec<_>>();

        let min: f64 = Min::min(data2.as_slice());
        let max: f64 = Max::max(data2.as_slice());
        let median = data2.median();
        let std_dev = data2.std_dev();

        log::info!(
            "Statistics for `{}`: min: {}ms, median: {}ms, max: {}ms, std_dev: {}ms",
            category,
            min,
            median,
            max,
            std_dev
        );

        let five_stats_summary = FiveSummaryStats::from_data(data.iter().copied());

        log::info!(
            "Statistics2 for `{}`: min: {}ms, median: {}ms, max: {}ms, lower_quartile: {}ms, \
            upper_quartile: {}ms, std_dev: {}ms",
            category,
            five_stats_summary.min,
            five_stats_summary.median,
            five_stats_summary.max,
            five_stats_summary.lower_quartile,
            five_stats_summary.upper_quartile,
            five_stats_summary.std_dev
        );
    }
}

struct FiveSummaryStats {
    min: u128,
    lower_quartile: u128,
    median: u128,
    upper_quartile: u128,
    max: u128,
    std_dev: f64,
}

impl FiveSummaryStats {
    fn from_data(data: impl IntoIterator<Item = u128>) -> Self {
        let mut data = data.into_iter().collect::<Vec<_>>();
        data.sort_unstable();

        assert!(data.len() >= 4);

        // Compute std dev.
        let n = data.len() as u128;
        let m = data.iter().sum::<u128>() / data.len() as u128;

        let square_sum = data.iter().fold(0, |acc, &x| {
            let d = max(x, m) - min(x, m);
            acc + d * d
        });
        let std_dev = (square_sum as f64 / n as f64).sqrt();

        // Compute five summary stats
        let idx = data.len() - 1;
        Self {
            min: data[0],
            lower_quartile: data[idx / 4],
            median: data[idx / 2],
            upper_quartile: data[idx * 3 / 4],
            max: data[idx],
            std_dev,
        }
    }
}
