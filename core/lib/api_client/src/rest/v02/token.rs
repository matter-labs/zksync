use crate::rest::client::{Client, Result};
use zksync_api_types::v02::{pagination::PaginationQuery, Response};
use zksync_types::{TokenId, TokenLike};

impl Client {
    pub async fn token_pagination_v02(
        &self,
        pagination_query: &PaginationQuery<TokenId>,
    ) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, "token")
            .query(&pagination_query)
            .send()
            .await
    }

    pub async fn token_by_id_v02(&self, token: &TokenLike) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, &format!("token/{}", token))
            .send()
            .await
    }

    pub async fn token_price_v02(
        &self,
        token: &TokenLike,
        token_id_or_usd: &str,
    ) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("token/{}/price_in/{}", token, token_id_or_usd),
        )
        .send()
        .await
    }
}
