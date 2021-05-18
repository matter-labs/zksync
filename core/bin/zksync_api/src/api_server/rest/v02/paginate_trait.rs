// Built-in uses

// External uses
use serde::Serialize;

// Workspace uses
use zksync_api_types::v02::pagination::{Paginated, PaginationQuery};

// Local uses
use super::error::Error;

#[async_trait::async_trait]
pub trait Paginate<T: Serialize, I: Serialize> {
    async fn paginate(&mut self, query: &PaginationQuery<I>) -> Result<Paginated<T, I>, Error>;
}
