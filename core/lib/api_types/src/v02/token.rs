use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use zksync_types::{Address, Token, TokenId};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ApiToken {
    pub id: TokenId,
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub enabled_for_fees: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenPrice {
    pub token_id: TokenId,
    pub token_symbol: String,
    pub price_in: String,
    pub decimals: u8,
    pub price: BigDecimal,
}

impl ApiToken {
    pub fn from_token_and_eligibility(token: Token, eligibility: bool) -> Self {
        ApiToken {
            id: token.id,
            address: token.address,
            symbol: token.symbol,
            decimals: token.decimals,
            enabled_for_fees: eligibility,
        }
    }
}
