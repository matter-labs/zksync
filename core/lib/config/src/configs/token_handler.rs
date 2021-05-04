// Built-in uses
use std::fs;
use std::time::Duration;
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::TokenInfo;
// Local uses
use crate::envy_load;

/// Configuration for the Ethereum sender crate.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct TokenHandlerConfig {
    /// The name of the trusted list of tokens.
    pub token_list_name: String,
    /// The number of seconds that set the request period to EthWatcher.
    pub poll_interval: u64,
    /// Link to MatterMost channel for token list notification.
    pub webhook_url: String,
}

impl TokenHandlerConfig {
    pub fn from_env() -> Self {
        envy_load!("token_handler", "TOKEN_HANDLER_")
    }

    pub fn token_list_file(&self) -> String {
        self.token_list_name.clone()
    }

    /// Converts self.poll_interval into Duration.
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval)
    }

    pub fn token_list(&self) -> Vec<TokenInfo> {
        let token_list_name = self.token_list_file();
        let path = format!("./etc/token-lists/{}.json", token_list_name);

        serde_json::from_str(&fs::read_to_string(path).expect("File for token list not found"))
            .expect("Invalid config format")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::set_env;

    fn expected_config() -> TokenHandlerConfig {
        TokenHandlerConfig {
            token_list_name: "localhost".to_string(),
            poll_interval: 1,
            webhook_url: "http://127.0.0.1".to_string(),
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
TOKEN_HANDLER_POLL_INTERVAL=1
TOKEN_HANDLER_WEBHOOK_URL="http://127.0.0.1"
TOKEN_HANDLER_TOKEN_LIST_NAME="localhost"
        "#;
        set_env(config);

        let actual_config = TokenHandlerConfig::from_env();
        let expected_config = expected_config();
        assert_eq!(actual_config, expected_config);
    }
}
