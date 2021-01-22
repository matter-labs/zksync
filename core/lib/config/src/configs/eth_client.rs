// External uses
use serde::Deserialize;
// Local uses
use crate::envy_load;

/// Configuration for the Ethereum gateways.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ETHClientConfig {
    /// Numeric identifier of the L1 network (e.g. `9` for localhost).
    pub chain_id: u8,
    /// How much do we want to increase gas price provided by the network?
    /// Normally it's 1, we use the network-provided price (and limit it with the gas adjuster in eth sender).
    /// However, it can be increased to speed up the transaction mining time.
    pub gas_price_factor: f64,
    /// Address of the Ethereum node API.
    pub web3_url: Vec<String>,
}

impl ETHClientConfig {
    pub fn from_env() -> Self {
        envy_load!("eth_client", "ETH_CLIENT_")
    }

    /// Get first web3 url, useful in direct web3 clients, which don't need any multiplexers
    pub fn web3_url(&self) -> String {
        self.web3_url
            .first()
            .cloned()
            .expect("Should be at least one")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::set_env;

    fn expected_config() -> ETHClientConfig {
        ETHClientConfig {
            chain_id: 9,
            gas_price_factor: 1.0f64,
            web3_url: vec![
                "http://127.0.0.1:8545".into(),
                "http://127.0.0.1:8546".into(),
            ],
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
ETH_CLIENT_CHAIN_ID="9"
ETH_CLIENT_GAS_PRICE_FACTOR="1"
ETH_CLIENT_WEB3_URL="http://127.0.0.1:8545,http://127.0.0.1:8546"
        "#;
        set_env(config);

        let actual = ETHClientConfig::from_env();
        assert_eq!(actual, expected_config());
        assert_eq!(actual.web3_url(), "http://127.0.0.1:8545");
    }
}
