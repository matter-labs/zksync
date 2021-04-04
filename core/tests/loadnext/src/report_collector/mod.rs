use failure_collector::FailureCollector;
use futures::{channel::mpsc::Receiver, StreamExt};

use crate::{
    report::{Report, ReportLabel},
    report_collector::metrics_collector::MetricsCollector,
};

mod failure_collector;
mod metrics_collector;

#[derive(Debug, Clone, Copy)]
pub enum FinalResolution {
    TestPassed,
    TestFailed,
}

#[derive(Debug)]
pub struct ReportCollector {
    reports_stream: Receiver<Report>,
    metrics_collector: MetricsCollector,
    failure_collector: FailureCollector,
}

impl ReportCollector {
    pub fn new(reports_stream: Receiver<Report>) -> Self {
        Self {
            reports_stream,
            metrics_collector: MetricsCollector::new(),
            failure_collector: FailureCollector::new(),
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

            self.failure_collector.add_status(&report.label);

            // Report failure, if it exists.
            if let ReportLabel::ActionFailed { error } = &report.label {
                vlog::warn!("Operation failed: {}", error);
            }
        }

        // All the receivers are gone, it's likely the end of the test.
        // Now we can output the statistics.
        self.metrics_collector.report();
        self.failure_collector.report();

        self.final_resolution()
    }

    fn final_resolution(&self) -> FinalResolution {
        if self.failure_collector.failures() > 0 {
            FinalResolution::TestFailed
        } else {
            FinalResolution::TestPassed
        }
    }
}
