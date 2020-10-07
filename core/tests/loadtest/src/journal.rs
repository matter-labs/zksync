// Built-in import
use std::time::Instant;
// External uses
// Workspace uses
// Local uses

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Stats {
    pub created: u64,
    pub executed: u64,
    pub verified: u64,
    pub errored: u64,
}

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Summary {
    pub txs: Stats,
    pub ops: Stats,
}

#[derive(Debug, Default, Clone)]
pub struct Journal(Vec<(Instant, Summary)>);

impl Journal {
    pub fn record_stats(&mut self, at: Instant, entry: Summary) {
        self.0.push((at, entry));
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }
}
