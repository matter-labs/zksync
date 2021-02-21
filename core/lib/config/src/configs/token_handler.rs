// Built-in uses
use std::fs;
use std::time::Duration;
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::TokenInfo;
use zksync_utils::{get_env, parse_env};

/// Configuration for the Ethereum sender crate.
#[derive(Debug, Deserialize, Clone)]
pub struct TokenHandlerConfig {
    pub poll_interval: u64,
    pub token_list: Vec<TokenInfo>,
}

impl TokenHandlerConfig {
    pub fn from_env() -> Self {
        let token_list = {
            let home = get_env("ZKSYNC_HOME");
            let token_list_name = get_env("TOKEN_HANDLER_TOKEN_LIST_NAME");
            let path = format!("{}/etc/token-lists/{}.json", home, token_list_name);

            serde_json::from_str(&fs::read_to_string(path).expect("Invalid config path"))
                .expect("Invalid config format")
        };

        Self {
            token_list,
            poll_interval: parse_env("TOKEN_HANDLER_POLL_INTERVAL"),
        }
    }

    /// Converts `self.poll_interval` into `Duration`.
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval)
    }
}
