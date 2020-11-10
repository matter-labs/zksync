//! This module handles metric export to the Prometheus server

use std::io::{Error, ErrorKind};
// External uses
use prometheus_exporter_base::{render_prometheus, MetricType, PrometheusMetric};
// Workspace uses
use tokio::task::JoinHandle;
use zksync_config::ConfigurationOptions;
use zksync_storage::ConnectionPool;
use zksync_types::ActionType;

fn convert_err(err: anyhow::Error) -> std::io::Error {
    // Prometheus required `failure::Error`, so we convert anyhow to the type
    // which can be converted into `failure::Error`.
    Error::new(ErrorKind::Other, err.to_string())
}

#[must_use]
pub fn run_prometheus_exporter(
    connection_pool: ConnectionPool,
    config: &ConfigurationOptions,
) -> JoinHandle<()> {
    let addr = ([0, 0, 0, 0], config.prometheus_export_port).into();

    tokio::spawn(render_prometheus(addr, (), |_, _| async move {
        let mut storage = connection_pool.access_storage().await?;
        let mut transaction = storage.start_transaction().await.map_err(convert_err)?;
        let mut block_schema = transaction.chain().block_schema();

        let pc = PrometheusMetric::new(
            "block_commit_unconfirmed",
            MetricType::Counter,
            "Number of commits that are unconfirmed",
        );
        let mut s = pc.render_header();
        s.push_str(
            &pc.render_sample(
                None,
                block_schema
                    .count_operations(ActionType::COMMIT, false)
                    .await
                    .map_err(convert_err)?,
                None,
            ),
        );

        let pc = PrometheusMetric::new(
            "block_verify_unconfirmed",
            MetricType::Counter,
            "Number of verifies that are unconfirmed",
        );
        s.push_str(&pc.render_header());
        s.push_str(
            &pc.render_sample(
                None,
                block_schema
                    .count_operations(ActionType::VERIFY, false)
                    .await
                    .map_err(convert_err)?,
                None,
            ),
        );

        let pc = PrometheusMetric::new(
            "block_commit_confirmed",
            MetricType::Counter,
            "Number of commits that are confirmed",
        );
        s.push_str(&pc.render_header());
        s.push_str(
            &pc.render_sample(
                None,
                block_schema
                    .count_operations(ActionType::COMMIT, true)
                    .await
                    .map_err(convert_err)?,
                None,
            ),
        );

        let pc = PrometheusMetric::new(
            "block_verify_confirmed",
            MetricType::Counter,
            "Number of verifies that are confirmed",
        );
        s.push_str(&pc.render_header());
        s.push_str(
            &pc.render_sample(
                None,
                block_schema
                    .count_operations(ActionType::VERIFY, true)
                    .await
                    .map_err(convert_err)?,
                None,
            ),
        );

        transaction.commit().await.map_err(convert_err)?;

        Ok(s)
    }))
}
