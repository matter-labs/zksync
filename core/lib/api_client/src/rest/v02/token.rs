// Built-in uses

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{pagination::PaginationQuery, Address, Token, TokenId, TokenLike};

// Local uses
use super::Response;
use crate::rest::client::{Client, Result};

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

/// Tokens API part.
impl Client {
    pub async fn token_pagination_v02(
        &self,
        pagination_query: &PaginationQuery<TokenId>,
    ) -> Result<Response> {
        self.get("token").query(&pagination_query).send().await
    }

    pub async fn token_by_id_v02(&self, token: &TokenLike) -> Result<Response> {
        self.get(&format!("token/{}", token)).send().await
    }

    pub async fn token_price_v02(
        &self,
        token: &TokenLike,
        token_id_or_usd: &str,
    ) -> Result<Response> {
        self.get(&format!("token/{}/price_in/{}", token, token_id_or_usd))
            .send()
            .await
    }
}
