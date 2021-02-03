use serde::Deserialize;

pub use crate::configs::{
    ApiConfig, ChainConfig, ContractsConfig, DBConfig, DevLiquidityTokenWatcherConfig,
    ETHClientConfig, ETHSenderConfig, ETHWatchConfig, MiscConfig, ProverConfig, TickerConfig,
};

pub mod configs;
pub mod test_config;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ZkSyncConfig {
    pub api: ApiConfig,
    pub chain: ChainConfig,
    pub contracts: ContractsConfig,
    pub db: DBConfig,
    pub eth_client: ETHClientConfig,
    pub eth_sender: ETHSenderConfig,
    pub eth_watch: ETHWatchConfig,
    pub prover: ProverConfig,
    pub ticker: TickerConfig,
}

impl ZkSyncConfig {
    pub fn from_env() -> Self {
        Self {
            api: ApiConfig::from_env(),
            chain: ChainConfig::from_env(),
            contracts: ContractsConfig::from_env(),
            db: DBConfig::from_env(),
            eth_client: ETHClientConfig::from_env(),
            eth_sender: ETHSenderConfig::from_env(),
            eth_watch: ETHWatchConfig::from_env(),
            prover: ProverConfig::from_env(),
            ticker: TickerConfig::from_env(),
        }
    }
}
