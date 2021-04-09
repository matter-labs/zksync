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
