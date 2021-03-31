// Built-in uses
use std::fmt;

// External uses
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

// Workspace uses

use zksync_types::{
    pagination::{Paginated, PaginationQuery},
    Address, TokenId, TokenLike,
};

// Local uses
use crate::rest::client::{self, Client};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ApiToken {
    pub id: TokenId,
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub enabled_for_fees: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Usd {
    Usd,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum TokenIdOrUsd {
    Id(TokenId),
    Usd(Usd),
}

impl fmt::Display for TokenIdOrUsd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenIdOrUsd::Id(id) => id.fmt(f),
            TokenIdOrUsd::Usd(_) => write!(f, "usd"),
        }
    }
}

/// Tokens API part.
impl Client {
    pub async fn token_pagination_v02(
        &self,
        pagination_query: PaginationQuery<TokenId>,
    ) -> client::Result<Paginated<ApiToken, TokenId>> {
        self.get("token").query(&pagination_query).send().await
    }

    pub async fn token_by_id_v02(&self, token: &TokenLike) -> client::Result<ApiToken> {
        self.get(&format!("token/{}", token)).send().await
    }

    pub async fn token_price_v02(
        &self,
        token: &TokenLike,
        token_id_or_usd: TokenIdOrUsd,
    ) -> client::Result<BigDecimal> {
        self.get(&format!("token/{}/price_in/{}", token, token_id_or_usd))
            .send()
            .await
    }
}
