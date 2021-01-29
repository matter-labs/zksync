//! This module handles metric export to the Prometheus server

use metrics_exporter_prometheus::PrometheusBuilder;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::delay_for;
use zksync_storage::ConnectionPool;
use zksync_types::aggregated_operations::AggregatedActionType::*;

const QUERY_INTERVAL: Duration = Duration::from_secs(60);

pub fn run_prometheus_exporter(
    connection_pool: ConnectionPool,
    port: u16,
    is_operation_counter_needed: bool,
) -> (JoinHandle<()>, Option<JoinHandle<()>>) {
    let addr = ([0, 0, 0, 0], port);
    let (recorder, exporter) = PrometheusBuilder::new()
        .listen_address(addr)
        .build_with_exporter()
        .expect("failed to install Prometheus recorder");
    metrics::set_boxed_recorder(Box::new(recorder)).expect("failed to set metrics recorder");

    let prometheus_handle = tokio::spawn(async move {
        tokio::pin!(exporter);
        loop {
            tokio::select! {
                _ = &mut exporter => {}
            }
        }
    });

    let operation_counter_handle = if is_operation_counter_needed {
        Some(tokio::spawn(async move {
            let mut storage = connection_pool
                .access_storage()
                .await
                .expect("unable to access storage");

            loop {
                let mut transaction = storage
                    .start_transaction()
                    .await
                    .expect("unable to start db transaction");
                let mut block_schema = transaction.chain().block_schema();

                for &action in &[CommitBlocks, ExecuteBlocks] {
                    for &is_confirmed in &[false, true] {
                        let result = block_schema
                            .count_aggregated_operations(action, is_confirmed)
                            .await
                            .expect("");
                        metrics::gauge!(
                            "count_operations",
                            result as f64,
                            "action" => action.to_string(),
                            "confirmed" => is_confirmed.to_string()
                        );
                    }
                }

                transaction
                    .commit()
                    .await
                    .expect("unable to commit db transaction");

                delay_for(QUERY_INTERVAL).await;
            }
        }))
    } else {
        None
    };

    (prometheus_handle, operation_counter_handle)
}
