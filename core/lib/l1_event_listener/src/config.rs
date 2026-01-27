use serde::Deserialize;
use web3::types::Address;
use zksync_config::configs::{ChainConfig, ContractsConfig as EnvContractsConfig};
use zksync_types::H256;
use zksync_types::network::Network;

#[derive(Debug, Deserialize)]
pub struct ContractsConfig {
    pub eth_network: Network,
    pub governance_addr: Address,
    pub genesis_tx_hash: H256,
    pub contract_addr: Address,
    pub init_contract_version: u32,
    pub upgrade_eth_blocks: Vec<u64>,
}

impl ContractsConfig {
    pub fn from_file(path: &str) -> Self {
        let content =
            std::fs::read_to_string(path).expect("Unable to find the specified config file");
        serde_json::from_str(&content).expect("Invalid configuration file provided")
    }

    pub fn from_env() -> Self {
        let contracts_opts = EnvContractsConfig::from_env();
        let chain_opts = ChainConfig::from_env();

        Self {
            eth_network: chain_opts.eth.network,
            governance_addr: contracts_opts.governance_addr,
            genesis_tx_hash: contracts_opts.genesis_tx_hash,
            contract_addr: contracts_opts.contract_addr,
            init_contract_version: contracts_opts.init_contract_version,
            upgrade_eth_blocks: contracts_opts.upgrade_eth_blocks,
        }
    }
}
