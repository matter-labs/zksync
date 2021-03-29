use std::time::Duration;

use crate::envy_load;
/// External uses
use serde::Deserialize;
use zksync_types::{Address, H256};

// There are two types of configs:
// The original one (with tx_interval_scaling_factor)
// And the public one (with max_tx_interval)

// It's easier for humans to think in factors
// But the rest of the codebase does not
// really care about the factor, it only needs the max_tx_interval

#[derive(Debug, Deserialize, Clone, PartialEq)]
struct ForcedExitRequestsInternalConfig {
    pub enabled: bool,
    pub max_tokens_per_request: u8,
    pub recomended_tx_interval: i64,
    pub tx_interval_scaling_factor: f64,
    pub price_per_token: i64,
    pub digits_in_id: u8,
    pub wait_confirmations: u64,
    pub sender_private_key: String,
    pub sender_eth_private_key: H256,
    pub sender_account_address: Address,
    pub expiration_period: u64,
    pub blocks_check_amount: u64,
    pub eth_node_poll_interval: u64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ForcedExitRequestsConfig {
    pub enabled: bool,
    pub max_tokens_per_request: u8,
    pub recomended_tx_interval: i64,
    pub max_tx_interval: i64,
    pub price_per_token: i64,
    pub digits_in_id: u8,
    pub wait_confirmations: u64,
    pub sender_private_key: String,
    pub sender_eth_private_key: H256,
    pub sender_account_address: Address,
    pub expiration_period: u64,
    pub blocks_check_amount: u64,
    pub eth_node_poll_interval: u64,
}

// Checks that in no way the price will overlap with the requests id space
//
// The amount that the users have to send to pay for the ForcedExit request
// = (number of tokens) * (price_per_token) + id
//
// Thus we need to check that at least digits_in_id first digits
// are equal to zeroes in price_per_token
fn validate_price_with_id_space(price: i64, digits_in_id: u8) {
    let id_space = (10_i64).saturating_pow(digits_in_id.into());

    assert!(
        price % id_space == 0,
        "The price per token may overlap with request id"
    )
}

impl ForcedExitRequestsConfig {
    pub fn from_env() -> Self {
        let config: ForcedExitRequestsInternalConfig =
            envy_load!("forced_exit_requests", "FORCED_EXIT_REQUESTS_");

        let max_tx_interval: f64 =
            (config.recomended_tx_interval as f64) * config.tx_interval_scaling_factor;

        validate_price_with_id_space(config.price_per_token, config.digits_in_id);

        ForcedExitRequestsConfig {
            enabled: config.enabled,
            max_tokens_per_request: config.max_tokens_per_request,
            recomended_tx_interval: config.recomended_tx_interval,
            max_tx_interval: max_tx_interval.round() as i64,
            digits_in_id: config.digits_in_id,
            price_per_token: config.price_per_token,
            wait_confirmations: config.wait_confirmations,
            sender_private_key: config.sender_private_key,
            sender_eth_private_key: config.sender_eth_private_key,
            sender_account_address: config.sender_account_address,
            expiration_period: config.expiration_period,
            blocks_check_amount: config.blocks_check_amount,
            eth_node_poll_interval: config.eth_node_poll_interval,
        }
    }

    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.eth_node_poll_interval)
    }
}
