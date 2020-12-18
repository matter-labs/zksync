// External uses
use serde::Deserialize;
// Local uses
use crate::envy_load;

/// Configuration for the Ethereum gateways.
#[derive(Debug, Deserialize)]
pub struct ETHClientConfig {
    /// Numeric identifier of the L1 network (e.g. `9` for localhost).
    pub chain_id: u64,
    /// How much do we want to increase gas price provided by the network?
    /// Normally it's 1, we use the network-provided price (and limit it with the gas adjuster in eth sender).
    /// However, it can be increased to speed up the transaction mining time.
    pub gas_price_factor: f64,
    /// Address of the Ethereum node API.
    pub web3_url: String,
}

impl ETHClientConfig {
    pub fn from_env() -> Self {
        envy_load!("eth_client", "ETH_CLIENT_")
    }
}
