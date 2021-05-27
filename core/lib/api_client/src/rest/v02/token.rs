use crate::rest::client::{Client, Result};
use zksync_api_types::v02::{
    pagination::{IdOrLatest, PaginationQuery},
    Response,
};
use zksync_types::{TokenId, TokenLike};

impl Client {
    pub async fn token_pagination(
        &self,
        pagination_query: &PaginationQuery<IdOrLatest<TokenId>>,
    ) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, "tokens")
            .query(&pagination_query)
            .send()
            .await
    }

    pub async fn token_by_id(&self, token: &TokenLike) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, &format!("tokens/{}", token))
            .send()
            .await
    }

    pub async fn token_price(&self, token: &TokenLike, token_id_or_usd: &str) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("tokens/{}/priceIn/{}", token, token_id_or_usd),
        )
        .send()
        .await
    }
}
