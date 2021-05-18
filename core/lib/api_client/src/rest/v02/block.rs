use crate::rest::client::{Client, Result};

use zksync_api_types::v02::{pagination::PaginationQuery, Response};
use zksync_types::{tx::TxHash, BlockNumber};

impl Client {
    pub async fn block_by_number(&self, block_position: &str) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, &format!("block/{}", block_position))
            .send()
            .await
    }

    pub async fn block_transactions(
        &self,
        pagination_query: &PaginationQuery<TxHash>,
        block_position: &str,
    ) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("block/{}/transaction", block_position),
        )
        .query(&pagination_query)
        .send()
        .await
    }

    pub async fn block_pagination(
        &self,
        pagination_query: &PaginationQuery<BlockNumber>,
    ) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, "block")
            .query(pagination_query)
            .send()
            .await
    }
}
