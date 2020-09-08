//! This module handles metric export to the Prometheus server

// External uses
use prometheus_exporter_base::{render_prometheus, MetricType, PrometheusMetric};
// Workspace uses
use models::config_options::ConfigurationOptions;
use models::ActionType;
use storage::ConnectionPool;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

#[must_use]
pub fn start_prometheus_exporter(
    connection_pool: ConnectionPool,
    config: &ConfigurationOptions,
    runtime: &Runtime,
) -> JoinHandle<()> {
    let addr = ([0, 0, 0, 0], config.prometheus_export_port).into();

    runtime.spawn(render_prometheus(addr, (), |_, _| async move {
        let storage = connection_pool.access_storage_fragile()?;
        let block_schema = storage.chain().block_schema();

        let pc = PrometheusMetric::new(
            "block_commit_unconfirmed",
            MetricType::Counter,
            "Number of commits that are unconfirmed",
        );
        let mut s = pc.render_header();
        s.push_str(&pc.render_sample(
            None,
            block_schema.count_operations(ActionType::COMMIT, false)?,
            None,
        ));

        let pc = PrometheusMetric::new(
            "block_verify_unconfirmed",
            MetricType::Counter,
            "Number of verifies that are unconfirmed",
        );
        s.push_str(&pc.render_header());
        s.push_str(&pc.render_sample(
            None,
            block_schema.count_operations(ActionType::VERIFY, false)?,
            None,
        ));

        let pc = PrometheusMetric::new(
            "block_commit_confirmed",
            MetricType::Counter,
            "Number of commits that are confirmed",
        );
        s.push_str(&pc.render_header());
        s.push_str(&pc.render_sample(
            None,
            block_schema.count_operations(ActionType::COMMIT, true)?,
            None,
        ));

        let pc = PrometheusMetric::new(
            "block_verify_confirmed",
            MetricType::Counter,
            "Number of verifies that are confirmed",
        );
        s.push_str(&pc.render_header());
        s.push_str(&pc.render_sample(
            None,
            block_schema.count_operations(ActionType::VERIFY, true)?,
            None,
        ));

        Ok(s)
    }))
}
