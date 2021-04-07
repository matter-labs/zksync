use crate::{Address, Token, TokenId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ApiToken {
    pub id: TokenId,
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub enabled_for_fees: bool,
}

impl From<(Token, bool)> for ApiToken {
    fn from(token: (Token, bool)) -> Self {
        ApiToken {
            id: token.0.id,
            address: token.0.address,
            symbol: token.0.symbol,
            decimals: token.0.decimals,
            enabled_for_fees: token.1,
        }
    }
}
