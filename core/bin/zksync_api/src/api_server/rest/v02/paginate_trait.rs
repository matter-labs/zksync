// Built-in uses

// External uses
use serde::Serialize;

// Workspace uses
use zksync_api_types::v02::pagination::{Paginated, PaginationQuery, MAX_LIMIT};

// Local uses
use super::error::{Error, InvalidDataError};

#[async_trait::async_trait]
pub trait Paginate<T: Serialize + 'static, I: Serialize + Send + Sync> {
    async fn paginate(&mut self, query: &PaginationQuery<I>) -> Result<Paginated<T, I>, Error>;

    async fn paginate_checked(
        &mut self,
        query: &PaginationQuery<I>,
    ) -> Result<Paginated<T, I>, Error> {
        if query.limit > MAX_LIMIT {
            Err(Error::from(InvalidDataError::PaginationLimitTooBig))
        } else {
            self.paginate(query).await
        }
    }
}
