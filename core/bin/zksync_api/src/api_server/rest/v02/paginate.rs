use super::error::Error;
use serde::Serialize;
use zksync_storage::StorageProcessor;
use zksync_types::{
    pagination::{Paginated, PaginationQuery},
    Token, TokenId,
};

#[async_trait::async_trait]
pub trait Paginate<T: Serialize> {
    type F: Serialize;

    async fn paginate(
        &mut self,
        query: PaginationQuery<Self::F>,
    ) -> Result<Paginated<T, Self::F>, Error>;
}

#[async_trait::async_trait]
impl Paginate<Token> for StorageProcessor<'_> {
    type F = TokenId;

    async fn paginate(
        &mut self,
        query: PaginationQuery<TokenId>,
    ) -> Result<Paginated<Token, TokenId>, Error> {
        let tokens = self
            .tokens_schema()
            .load_token_page(&query)
            .await
            .map_err(Error::internal)?;
        let count = self
            .tokens_schema()
            .get_count()
            .await
            .map_err(Error::internal)? as u32;
        Ok(Paginated::new(
            tokens,
            query.from,
            count,
            query.limit,
            query.direction,
        ))
    }
}
