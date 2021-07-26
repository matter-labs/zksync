// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::{Address, H256};
// Local uses
use crate::envy_load;

/// Data about deployed contracts.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ContractsConfig {
    pub upgrade_gatekeeper_addr: Address,
    pub governance_target_addr: Address,
    pub verifier_target_addr: Address,
    pub contract_target_addr: Address,
    pub contract_addr: Address,
    pub governance_addr: Address,
    pub verifier_addr: Address,
    pub deploy_factory_addr: Address,
    pub forced_exit_addr: Address,
    pub genesis_tx_hash: H256,
    pub init_contract_version: u32,
    pub upgrade_eth_blocks: Vec<u64>,
}

impl ContractsConfig {
    pub fn from_env() -> Self {
        envy_load!("contracts", "CONTRACTS_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::{addr, hash, set_env};

    fn expected_config() -> ContractsConfig {
        ContractsConfig {
            upgrade_gatekeeper_addr: addr("5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"),
            governance_target_addr: addr("5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"),
            verifier_target_addr: addr("5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"),
            contract_target_addr: addr("5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"),
            contract_addr: addr("70a0F165d6f8054d0d0CF8dFd4DD2005f0AF6B55"),
            governance_addr: addr("5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"),
            verifier_addr: addr("DAbb67b676F5b01FcC8997Cc8439846D0d8078ca"),
            deploy_factory_addr: addr("FC073319977e314F251EAE6ae6bE76B0B3BAeeCF"),
            forced_exit_addr: addr("9c7AeE886D6FcFc14e37784f143a6dAccEf50Db7"),
            genesis_tx_hash: hash(
                "b99ebfea46cbe05a21cd80fe5597d97b204befc52a16303f579c607dc1ac2e2e",
            ),
            init_contract_version: 4,
            upgrade_eth_blocks: vec![1, 4294967296, 1152921504606846976],
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
CONTRACTS_UPGRADE_GATEKEEPER_ADDR="0x5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"
CONTRACTS_GOVERNANCE_TARGET_ADDR="0x5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"
CONTRACTS_VERIFIER_TARGET_ADDR="0x5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"
CONTRACTS_CONTRACT_TARGET_ADDR="0x5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"
CONTRACTS_CONTRACT_ADDR="0x70a0F165d6f8054d0d0CF8dFd4DD2005f0AF6B55"
CONTRACTS_GOVERNANCE_ADDR="0x5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9"
CONTRACTS_VERIFIER_ADDR="0xDAbb67b676F5b01FcC8997Cc8439846D0d8078ca"
CONTRACTS_DEPLOY_FACTORY_ADDR="0xFC073319977e314F251EAE6ae6bE76B0B3BAeeCF"
CONTRACTS_FORCED_EXIT_ADDR="0x9c7AeE886D6FcFc14e37784f143a6dAccEf50Db7"
CONTRACTS_GENESIS_TX_HASH="0xb99ebfea46cbe05a21cd80fe5597d97b204befc52a16303f579c607dc1ac2e2e"
CONTRACTS_INIT_CONTRACT_VERSION=4
CONTRACTS_UPGRADE_ETH_BLOCKS="1,4294967296,1152921504606846976"
        "#;
        set_env(config);

        let actual = ContractsConfig::from_env();
        assert_eq!(actual, expected_config());
    }
}
