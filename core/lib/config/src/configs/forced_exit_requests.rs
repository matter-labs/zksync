use crate::envy_load;
/// External uses
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ForcedExitRequestsConfig {
    pub enabled: bool,
}

impl ForcedExitRequestsConfig {
    pub fn from_env() -> Self {
        envy_load!("forced_exit_requests", "FORCED_EXIT_REQUESTS")
    }
}
