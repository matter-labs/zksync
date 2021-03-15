// Built-in uses
use std::fs;
use std::time::Duration;
// External uses
use reqwest::Url;
// Workspace uses
use zksync_types::TokenInfo;
use zksync_utils::{get_env, parse_env, parse_env_if_exists};

/// Configuration for the Ethereum sender crate.
#[derive(Debug, Clone)]
pub struct TokenHandlerConfig {
    /// List of trusted tokens.
    pub token_list: Vec<TokenInfo>,
    /// The number of seconds that set the request period to EthWatcher.
    pub poll_interval: u64,
    /// Link to MatterMost channel for token list notification.
    pub webhook_url: Option<Url>,
    // TODO: Having reqwest as a dependency seems an overkill to just have the Url type here.
    // This can be done easily on the caller side. (ZKS-563)
}

impl TokenHandlerConfig {
    pub fn from_env() -> Self {
        // TODO: I think that in the config, we should only load variables, not doing any FS IO. In this case, we can do the following:
        // Only load TOKEN_HANDLER_TOKEN_LIST_NAME. This will enable us to reuse envy_load.
        // Provide a token_list_file() method which will provide a path to the file.
        // Provide a token_list() method which will load the list from the FS.
        // This way, it'd be possible to load config even if there is no such file in the FS.
        // It may be essential if, for example, this config is a part of some structure, and this structure will be constructed in the unit tests. (ZKS-563)

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
            webhook_url: parse_env_if_exists("NOTIFICATION_WEBHOOK_URL"),
            // TODO: Not having this variable prefixed feels odd. Exceptions from the conventions are hard to manage long term,
            // that was the reason why we renamed the old-established DB_POOL_SIZE to DATABASE_POOL_SIZE. (ZKS-563)
        }
    }

    /// Converts `self.poll_interval` into `Duration`.
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval)
    }
}
// TODO: Tests are missing for this config. (ZKS-563)
