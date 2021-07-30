// Built-in uses
use std::time::Duration;
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::{Address, H256};
// Local uses
use crate::envy_load;

/// Configuration for the Ethereum sender crate.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ETHSenderConfig {
    /// Options related to the Ethereum sender directly.
    pub sender: Sender,
    /// Options related to the `gas_adjuster` submodule.
    pub gas_price_limit: GasLimit,
}

impl ETHSenderConfig {
    pub fn from_env() -> Self {
        Self {
            sender: envy_load!("eth_sender", "ETH_SENDER_SENDER_"),
            gas_price_limit: envy_load!(
                "eth_sender.gas_price_limit",
                "ETH_SENDER_GAS_PRICE_LIMIT_"
            ),
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Sender {
    /// Private key of the operator account.
    pub operator_private_key: H256,
    /// Address of the operator account.
    pub operator_commit_eth_addr: Address,
    /// mount of confirmations required to consider L1 transaction committed.
    pub wait_confirmations: u64,
    /// Amount of blocks we will wait before considering L1 transaction stuck.
    pub expected_wait_time_block: u64,
    /// Node polling period in seconds.
    pub tx_poll_period: u64,
    /// The maximum amount of simultaneously sent Ethereum transactions.
    pub max_txs_in_flight: u64,
    /// Whether sender should interact with L1 or not.
    pub is_enabled: bool,
}

impl Sender {
    /// Converts `self.tx_poll_period` into `Duration`.
    pub fn tx_poll_period(&self) -> Duration {
        Duration::from_secs(self.tx_poll_period)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GasLimit {
    /// Gas price limit to be used by GasAdjuster until the statistics data is gathered.
    pub default: u64,
    /// Interval between updates of the gas price limit (used by GasAdjuster) in seconds.
    pub update_interval: u64,
    /// Interval between adding the Ethereum node gas price to the GasAdjuster in seconds.
    pub sample_interval: u64,
    /// Scale factor for gas price limit (used by GasAdjuster).
    pub scale_factor: f64,
}

impl GasLimit {
    /// Converts `self.update_interval` into `Duration`.
    pub fn update_interval(&self) -> Duration {
        Duration::from_secs(self.update_interval)
    }

    /// Converts `self.sample_interval` into `Duration`.
    pub fn sample_interval(&self) -> Duration {
        Duration::from_secs(self.sample_interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::{addr, hash, set_env};

    fn expected_config() -> ETHSenderConfig {
        ETHSenderConfig {
            sender: Sender {
                wait_confirmations: 1,
                expected_wait_time_block: 30,
                tx_poll_period: 3,
                max_txs_in_flight: 3,
                is_enabled: true,
                operator_private_key: hash(
                    "c1783a9a8222e47778911c58bb5aac1343eb425159ff140799e0a283bfb8fa16",
                ),
                operator_commit_eth_addr: addr("debe71e1de41fc77c44df4b6db940026e31b0e71"),
            },
            gas_price_limit: GasLimit {
                default: 400000000000,
                update_interval: 150,
                sample_interval: 15,
                scale_factor: 1.0f64,
            },
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
ETH_SENDER_SENDER_WAIT_CONFIRMATIONS="1"
ETH_SENDER_SENDER_EXPECTED_WAIT_TIME_BLOCK="30"
ETH_SENDER_SENDER_TX_POLL_PERIOD="3"
ETH_SENDER_SENDER_MAX_TXS_IN_FLIGHT="3"
ETH_SENDER_SENDER_IS_ENABLED="true"
ETH_SENDER_SENDER_OPERATOR_PRIVATE_KEY="0xc1783a9a8222e47778911c58bb5aac1343eb425159ff140799e0a283bfb8fa16"
ETH_SENDER_SENDER_OPERATOR_COMMIT_ETH_ADDR="0xdebe71e1de41fc77c44df4b6db940026e31b0e71"
ETH_SENDER_GAS_PRICE_LIMIT_DEFAULT="400000000000"
ETH_SENDER_GAS_PRICE_LIMIT_UPDATE_INTERVAL="150"
ETH_SENDER_GAS_PRICE_LIMIT_SAMPLE_INTERVAL="15"
ETH_SENDER_GAS_PRICE_LIMIT_SCALE_FACTOR="1"
        "#;
        set_env(config);

        let actual = ETHSenderConfig::from_env();
        assert_eq!(actual, expected_config());
    }

    /// Checks the correctness of the config helper methods.
    #[test]
    fn methods() {
        let config = expected_config();

        assert_eq!(
            config.sender.tx_poll_period(),
            Duration::from_secs(config.sender.tx_poll_period)
        );

        assert_eq!(
            config.gas_price_limit.update_interval(),
            Duration::from_secs(config.gas_price_limit.update_interval)
        );
        assert_eq!(
            config.gas_price_limit.sample_interval(),
            Duration::from_secs(config.gas_price_limit.sample_interval)
        );
    }
}
