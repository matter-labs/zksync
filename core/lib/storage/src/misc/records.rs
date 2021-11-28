// Built-in imports
use std::str::FromStr;
// External imports
use serde::{Deserialize, Serialize};
use sqlx::{types::BigDecimal, FromRow};
// Workspace imports
// Local imports
use crate::utils::{address_to_stored_string, stored_str_address_to_address};
use chrono::{DateTime, Utc};
use zksync_api_types::v02::token::ApiNFT;
use zksync_types::{
    tokens::{TokenMarketVolume, TokenPrice},
    AccountId, Address, Token, TokenId, H256, NFT,
};
use zksync_utils::big_decimal_to_ratio;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, FromRow)]
pub struct DbSubsidy {
    pub id: i32,
    pub tx_hash: Vec<u8>,
    pub usd_amount: u64,
    pub full_cost_usd: u64,
    pub token_id: i32,
    pub token_amount: BigDecimal,
    pub full_cost_token: BigDecimal,
    pub subsidy_type: String,
}

pub struct Subsidy {
    pub tx_hash: Vec<u8>,
    pub usd_amount: u64,
    pub full_cost_usd: u64,
    pub token_id: i32,
    pub token_amount: BigDecimal,
    pub full_cost_token: BigDecimal,
    pub subsidy_type: String,
}
