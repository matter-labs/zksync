use futures::{channel::mpsc::Receiver, StreamExt};

use crate::{report::Report, report_collector::metrics_collector::MetricsCollector};

mod metrics_collector;
mod script_collector;

#[derive(Debug)]
pub struct ReportCollector {
    reports_stream: Receiver<Report>,
    metrics_collector: MetricsCollector,
}

impl ReportCollector {
    pub fn new(reports_stream: Receiver<Report>) -> Self {
        Self {
            reports_stream,
            metrics_collector: MetricsCollector::new(),
        }
    }

    pub async fn run(mut self) {
        while let Some(report) = self.reports_stream.next().await {
            vlog::trace!("Report: {:?}", &report);

            self.metrics_collector
                .add_metric(report.action, report.time);

            todo!()
        }
    }
}
