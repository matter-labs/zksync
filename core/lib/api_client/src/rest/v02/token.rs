use crate::rest::client::{Client, Result};
use zksync_api_types::v02::{
    pagination::{ApiEither, PaginationQuery},
    Response,
};
use zksync_types::{TokenId, TokenLike};

impl Client {
    pub async fn token_pagination(
        &self,
        pagination_query: &PaginationQuery<ApiEither<TokenId>>,
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

    pub async fn nft_by_id(&self, id: TokenId) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, &format!("tokens/nft/{}", id))
            .send()
            .await
    }
}
