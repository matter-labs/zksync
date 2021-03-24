use super::error::{ApiError, InternalError};
use serde::Serialize;
use std::convert::TryFrom;
use zksync_storage::StorageProcessor;
use zksync_types::{
    pagination::{Paginated, PaginationQuery},
    Token, TokenId,
};

#[async_trait::async_trait]
pub trait Paginate<T: Serialize> {
    type F: Serialize;
    type E: ApiError;

    async fn paginate(
        &mut self,
        query: PaginationQuery<Self::F>,
    ) -> Result<Paginated<T, Self::F>, Self::E>;
}

#[async_trait::async_trait]
impl Paginate<Token> for StorageProcessor<'_> {
    type F = TokenId;
    type E = InternalError;

    async fn paginate(
        &mut self,
        query: PaginationQuery<TokenId>,
    ) -> Result<Paginated<Token, TokenId>, InternalError> {
        let tokens = self
            .tokens_schema()
            .load_token_page(&query)
            .await
            .map_err(InternalError::new)?;
        let count = self
            .tokens_schema()
            .get_count()
            .await
            .map_err(InternalError::new)?;
        let count = u32::try_from(count).map_err(InternalError::new)?;
        Ok(Paginated::new(
            tokens,
            query.from,
            count,
            query.limit,
            query.direction,
        ))
    }
}
