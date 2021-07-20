use serde::{Deserialize, Serialize};
use std::convert::TryInto;

/// Defines time range `[valid_from, valid_until]` for which transaction is valid,
/// time format is the same as Ethereum (UNIX timestamp in seconds)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeRange {
    pub valid_from: u64,
    pub valid_until: u64,
}

impl TimeRange {
    pub fn new(valid_from: u64, valid_until: u64) -> Self {
        Self {
            valid_from,
            valid_until,
        }
    }

    pub fn to_be_bytes(&self) -> [u8; 16] {
        [
            self.valid_from.to_be_bytes(),
            self.valid_until.to_be_bytes(),
        ]
        .concat()
        .try_into()
        .expect("valid_from and valid_until should be u64")
    }

    pub fn check_correctness(&self) -> bool {
        self.valid_from <= self.valid_until
    }

    pub fn is_valid(&self, block_timestamp: u64) -> bool {
        self.valid_from <= block_timestamp && block_timestamp <= self.valid_until
    }

    pub fn intersects(&self, other: Self) -> bool {
        self.valid_from <= other.valid_until && other.valid_from <= self.valid_until
    }
}

impl Default for TimeRange {
    fn default() -> Self {
        Self {
            valid_from: 0,
            valid_until: u64::max_value(),
        }
    }
}
