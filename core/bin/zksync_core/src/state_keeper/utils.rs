use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn system_time_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("failed to get system time")
        .as_secs()
}
