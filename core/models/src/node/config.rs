use std::time::Duration;

// HACK: hardcode some configuration options for now.
pub const PADDING_SUB_INTERVAL: Duration = Duration::from_secs(10);
pub const PROVER_GONE_TIMEOUT: Duration = Duration::from_secs(60);
pub const PROVER_PREPARE_DATA_INTERVAL: Duration = Duration::from_secs(3);
pub const PROVER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
pub const PROVER_CYCLE_WAIT: Duration = Duration::from_secs(5);
pub const TX_MINIBATCH_CREATE_TIME: Duration = Duration::from_millis(100);
pub const MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL: u64 = 20;
/// After server replica places its into leader_election table,
/// it checks db to see who is current leader with this interval.
pub const LEADER_LOOKUP_INTERVAL: Duration = Duration::from_secs(1);
/// Interval between state updates in server replica's observer mode.
pub const OBSERVER_MODE_PULL_INTERVAL: Duration = Duration::from_secs(1);
