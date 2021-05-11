// Built-in uses
use std::time::Duration;
// External uses
use serde::Deserialize;
// Local uses
use crate::envy_load;

/// Configuration of the Gateway Watch.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GatewayWatcherConfig {
    /// How often all gateways will be checked.
    /// Value in milliseconds.
    pub gateway_check_interval: u64,
    /// Time to wait before request again in case of unsuccessful request.
    /// Value in milliseconds.
    pub gateway_check_retry_delay: u64,
    /// Max request timeout. In milliseconds.
    pub gateway_check_request_timeout: u64,
    /// How many requests are allowed to be done within a single task.
    pub gateway_check_request_per_task_limit: usize,
    /// How many tasks are allowed to simultaneously make requests.
    pub gateway_check_task_limit: usize,
}

impl GatewayWatcherConfig {
    pub fn from_env() -> Self {
        envy_load!("gateway_watcher", "GATEWAY_WATCHER_")
    }

    /// Converts `self.gateway_check_interval` into `Duration`
    pub fn check_interval(&self) -> Duration {
        Duration::from_millis(self.gateway_check_interval)
    }

    /// Converts `self.gateway_check_retry_delay` into `Duration`
    pub fn retry_delay(&self) -> Duration {
        Duration::from_millis(self.gateway_check_retry_delay)
    }

    /// Converts `self.gateway_check_retry_delay` into `Duration`
    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.gateway_check_request_timeout)
    }

    pub fn task_limit(&self) -> usize {
        self.gateway_check_task_limit
    }

    pub fn request_per_task_limit(&self) -> usize {
        self.gateway_check_request_per_task_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::set_env;

    fn expected_config() -> GatewayWatcherConfig {
        GatewayWatcherConfig {
            gateway_check_interval: 1000,
            gateway_check_retry_delay: 500,
            gateway_check_request_per_task_limit: 10,
            gateway_check_task_limit: 1,
            gateway_check_request_timeout: 1000,
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
GATEWAY_WATCHER_GATEWAY_CHECK_INTERVAL="1000"
GATEWAY_WATCHER_GATEWAY_CHECK_RETRY_DELAY="500"
GATEWAY_WATCHER_GATEWAY_CHECK_REQUEST_PER_TASK_LIMIT="10"
GATEWAY_WATCHER_GATEWAY_CHECK_TASK_LIMIT="1"
GATEWAY_WATCHER_GATEWAY_CHECK_REQUEST_TIMEOUT="1000"
        "#;
        set_env(config);

        let actual = GatewayWatcherConfig::from_env();
        assert_eq!(actual, expected_config());
    }
}
