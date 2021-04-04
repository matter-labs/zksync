use futures::{channel::mpsc::Receiver, StreamExt};

use crate::{
    report::{Report, ReportLabel},
    report_collector::metrics_collector::MetricsCollector,
};

mod metrics_collector;
mod script_collector;

#[derive(Debug, Clone, Copy)]
pub enum FinalResolution {
    TestPassed,
    TestFailed,
}

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

    pub async fn run(mut self) -> FinalResolution {
        while let Some(report) = self.reports_stream.next().await {
            vlog::trace!("Report: {:?}", &report);

            if matches!(&report.label, ReportLabel::ActionDone) {
                // We only count successfully created statistics.
                self.metrics_collector
                    .add_metric(report.action, report.time);
            }
        }

        // All the receivers are gone, it's likely the end of the test.
        // Now we can output the statistics.
        self.metrics_collector.report();

        FinalResolution::TestPassed
    }
}
