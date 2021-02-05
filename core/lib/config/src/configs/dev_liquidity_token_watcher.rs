use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::envy_load;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DevLiquidityTokenWatcherConfig {
    pub blacklisted_tokens: HashSet<String>,
    pub default_volume: u32,
    pub regime: Regime,
}

impl DevLiquidityTokenWatcherConfig {
    pub fn from_env() -> Self {
        envy_load!(
            "dev-liquidity-token-watcher",
            "DEV_LIQUIDITY_TOKEN_WATCHER_"
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Regime {
    Blacklist,
    Whitelist,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::set_env;

    fn expected_config() -> DevLiquidityTokenWatcherConfig {
        let mut blacklisted_tokens = HashSet::new();
        blacklisted_tokens.insert("0x0000000000000000000000000000000000000001".to_string());
        DevLiquidityTokenWatcherConfig {
            blacklisted_tokens,
            default_volume: 500,
            regime: Regime::Whitelist,
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
DEV_LIQUIDITY_TOKEN_WATCHER_BLACKLISTED_TOKENS="0x6b175474e89094c44da98b954eedeac495271d0f"
DEV_LIQUIDITY_TOKEN_WATCHER_BLACKLISTED_TOKENS="0x0000000000000000000000000000000000000001"
DEV_LIQUIDITY_TOKEN_WATCHER_DEFAULT_VOLUME="500"
DEV_LIQUIDITY_TOKEN_WATCHER_REGIME="whitelist"
        "#;
        set_env(config);

        let actual = DevLiquidityTokenWatcherConfig::from_env();
        assert_eq!(actual, expected_config());
    }
}
