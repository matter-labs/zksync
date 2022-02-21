//! This module handles metric export to the Prometheus server

use metrics_exporter_prometheus::PrometheusBuilder;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use zksync_storage::{ConnectionPool, QueryResult};
use zksync_types::aggregated_operations::AggregatedActionType::*;

const QUERY_INTERVAL: Duration = Duration::from_secs(30);

pub fn run_operation_counter(connection_pool: ConnectionPool) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(e) = prometheus_exporter_iteration(connection_pool.clone()).await {
                vlog::error!("Prometheus error: {}", e);
            }
            sleep(QUERY_INTERVAL).await;
        }
    })
}

async fn prometheus_exporter_iteration(connection_pool: ConnectionPool) -> QueryResult<()> {
    let mut storage = connection_pool.access_storage().await?;
    let mut transaction = storage.start_transaction().await?;

    let mut block_schema = transaction.chain().block_schema();

    for &action in &[CommitBlocks, ExecuteBlocks] {
        for &is_confirmed in &[false, true] {
            let result = block_schema
                .count_aggregated_operations(action, is_confirmed)
                .await?;
            metrics::gauge!(
                "count_operations",
                result as f64,
                "action" => action.to_string(),
                "confirmed" => is_confirmed.to_string()
            );
        }
    }

    let rejected_txs = block_schema.count_rejected_txs().await?;

    metrics::gauge!("stored_rejected_txs", rejected_txs as f64);

    let mempool_size = transaction
        .chain()
        .mempool_schema()
        .get_mempool_size()
        .await?;
    metrics::gauge!("mempool_size", mempool_size as f64);

    transaction.commit().await?;
    Ok(())
}

pub fn run_prometheus_exporter(port: u16) -> JoinHandle<()> {
    let addr = ([0, 0, 0, 0], port);
    let (recorder, exporter) = PrometheusBuilder::new()
        .listen_address(addr)
        .build_with_exporter()
        .expect("failed to install Prometheus recorder");
    metrics::set_boxed_recorder(Box::new(recorder)).expect("failed to set metrics recorder");

    tokio::spawn(async move {
        tokio::pin!(exporter);
        loop {
            tokio::select! {
                _ = &mut exporter => {}
            }
        }
    })
}
