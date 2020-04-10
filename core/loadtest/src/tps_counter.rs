// Built-in
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

/// Small concurrent counter, allowing counting the sent txs.
#[derive(Debug, Default)]
pub struct TPSCounter {
    n_txs: AtomicUsize,
}

impl TPSCounter {
    /// Increments the transactions counter.
    pub fn increment(&self) {
        self.n_txs.fetch_add(1, Ordering::SeqCst);
    }

    /// Loads the current counter value.
    pub fn load(&self) -> usize {
        self.n_txs.load(Ordering::SeqCst)
    }
}

/// Runs the routine polling the TPS counter and reporting the current outgoing TPS.
/// Reported TPS measures the *sending* txs metric (e.g. accepting them into mempool),
/// and not the actual tx processing throughput.
/// TPS is reported only if the new txs were sent within polling interval.
pub async fn run_tps_counter_printer(counter: Arc<TPSCounter>) {
    log::info!("Starting the TPS counter routine...");

    let mut check_timer = tokio::time::interval(Duration::from_secs(1));

    let mut instant = Instant::now();
    let mut last_seen_total_txs = counter.load();
    loop {
        let new_seen_total_txs = counter.load();

        let new_txs = new_seen_total_txs.saturating_sub(last_seen_total_txs);

        let tps = (new_txs as f64) / (instant.elapsed().as_millis() as f64) * 1000f64;

        if tps > 0.001f64 {
            log::info!("Outgoing tps: {}", tps);
        } else {
            log::debug!("No txs sent, nothing to report");
        }

        last_seen_total_txs = new_seen_total_txs;
        instant = Instant::now();

        check_timer.tick().await;
    }
}
