use futures::{channel::mpsc::Receiver, StreamExt};

use crate::report::Report;

#[derive(Debug)]
pub struct ReportCollector {
    reports_stream: Receiver<Report>,
}

impl ReportCollector {
    pub fn new(reports_stream: Receiver<Report>) -> Self {
        Self { reports_stream }
    }

    pub async fn run(mut self) {
        while let Some(report) = self.reports_stream.next().await {
            vlog::trace!("Report: {:?}", &report);

            todo!()
        }
    }
}
