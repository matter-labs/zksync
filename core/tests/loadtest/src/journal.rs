// Built-in import
use std::{
    cmp::{max, min},
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};
// External uses
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync_types::tx::TxHash;
// Local uses

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct TxLifecycle {
    pub created_at: Instant,
    pub sent_at: Instant,
    pub committed_at: Instant,
    pub verified_at: Instant,
}

impl TxLifecycle {
    pub fn send_duration(&self) -> Duration {
        self.sent_at.duration_since(self.created_at)
    }

    pub fn commit_duration(&self) -> Duration {
        self.committed_at.duration_since(self.sent_at)
    }

    pub fn verify_duration(&self) -> Duration {
        self.verified_at.duration_since(self.committed_at)
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

    pub fn five_stats_summary(&self) -> anyhow::Result<BTreeMap<String, FiveSummaryStats>> {
        let mut sending = Vec::new();
        let mut committing = Vec::new();
        let mut verifying = Vec::new();

        for (tx_hash, tx_result) in &self.txs {
            let tx_lifecycle = tx_result.as_ref().map_err(|err| {
                anyhow::anyhow!(
                    "An error occured while processing a transaction {}: {}",
                    tx_hash.to_string(),
                    err
                )
            })?;

            sending.push(tx_lifecycle.send_duration().as_micros());
            committing.push(tx_lifecycle.commit_duration().as_micros());
            verifying.push(tx_lifecycle.verify_duration().as_micros());
        }

        Ok([
            ("sending", sending),
            ("committing", committing),
            ("verifying", verifying),
        ]
        .iter()
        .map(|(category, data)| (category.to_string(), FiveSummaryStats::from_data(data)))
        .collect())
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Sample {
    pub started_at: Instant,
    pub finished_at: Instant,
}

impl Sample {
    pub fn duration(&self) -> Duration {
        self.finished_at.duration_since(self.started_at)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct FiveSummaryStats {
    pub min: u128,
    pub lower_quartile: u128,
    pub median: u128,
    pub upper_quartile: u128,
    pub max: u128,
    pub std_dev: f64,
}

impl FiveSummaryStats {
    pub fn from_data<'a, I>(data: I) -> Self
    where
        I: IntoIterator<Item = &'a u128>,
    {
        let mut data = data.into_iter().copied().collect::<Vec<_>>();
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

    pub fn from_samples<'a, I>(samples: I) -> Self
    where
        I: IntoIterator<Item = &'a Sample>,
    {
        let data = samples
            .into_iter()
            .map(|s| s.duration().as_micros())
            .collect::<Vec<_>>();

        Self::from_data(&data)
    }
}
