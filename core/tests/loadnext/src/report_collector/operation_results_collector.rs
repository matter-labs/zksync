use crate::report::ReportLabel;

/// Collector that analyzes the outcomes of the performed operations.
/// Currently it's solely capable of deciding whether test was failed or not.
#[derive(Debug, Clone, Default)]
pub struct OperationResultsCollector {
    successes: u64,
    skipped: u64,
    failures: u64,
}

impl OperationResultsCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_status(&mut self, status: &ReportLabel) {
        match status {
            ReportLabel::ActionDone => self.successes += 1,
            ReportLabel::ActionSkipped { .. } => self.skipped += 1,
            ReportLabel::ActionFailed { .. } => self.failures += 1,
        }
    }

    pub fn successes(&self) -> u64 {
        self.successes
    }

    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    pub fn failures(&self) -> u64 {
        self.failures
    }

    pub fn total(&self) -> u64 {
        self.successes + self.skipped + self.failures
    }

    pub fn report(&self) {
        vlog::info!(
            "Loadtest status: {} successful operations, {} skipped, {} failures. {} actions total.",
            self.successes(),
            self.skipped(),
            self.failures(),
            self.total()
        );
    }
}
