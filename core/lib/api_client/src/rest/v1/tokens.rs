//! Tokens part of API implementation.

// Built-in uses

// External uses
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{Token, TokenLike};

// Local uses
use super::client::{self, Client};

// Data transfer objects.

#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TokenPriceKind {
    Currency,
    Token,
}

#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TokenPriceQuery {
    #[serde(rename = "in")]
    pub kind: TokenPriceKind,
}

/// Tokens API part.
impl Client {
    pub async fn tokens(&self) -> client::Result<Vec<Token>> {
        self.get("tokens").send().await
    }

    pub async fn token_by_id(&self, token: &TokenLike) -> client::Result<Option<Token>> {
        self.get(&format!("tokens/{}", token)).send().await
    }

    pub async fn token_price(
        &self,
        token: &TokenLike,
        kind: TokenPriceKind,
    ) -> client::Result<Option<BigDecimal>> {
        self.get(&format!("tokens/{}/price", token))
            .query(&TokenPriceQuery { kind })
            .send()
            .await
    }
}
