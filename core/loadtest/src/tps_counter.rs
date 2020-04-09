// Built-in
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Debug, Default)]
pub struct TPSCounter {
    n_txs: AtomicUsize,
}

impl TPSCounter {
    pub fn increment(&self) {
        self.n_txs.fetch_add(1, Ordering::SeqCst);
    }

    pub fn load(&self) -> usize {
        self.n_txs.load(Ordering::SeqCst)
    }
}

pub async fn run_tps_counter_printer(counter: Arc<TPSCounter>) {
    let mut check_timer = tokio::time::interval(Duration::from_secs(10));

    let mut instant = Instant::now();
    let mut last_seen_total_txs = counter.load();
    loop {
        let new_seen_total_txs = counter.load();

        let new_txs = new_seen_total_txs.saturating_sub(last_seen_total_txs);

        let tps = (new_txs as f64) / (instant.elapsed().as_millis() as f64) * 1000f64;

        log::info!("outgoing tps: {}", tps);

        last_seen_total_txs = new_seen_total_txs;
        instant = Instant::now();

        check_timer.tick().await;
    }
}
