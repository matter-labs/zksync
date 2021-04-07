use crate::rest::client::{Client, Result};

use zksync_api_types::v02::{pagination::PaginationQuery, Response};
use zksync_types::{tx::TxHash, BlockNumber};

/// Block API part.
impl Client {
    /// Returns information about block with the specified number or null if block doesn't exist.
    pub async fn block_by_number_v02(&self, block_position: &str) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, &format!("block/{}", block_position))
            .send()
            .await
    }

    /// Returns information about transactions of the block with the specified number.
    pub async fn block_transactions_v02(
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

    /// Returns information about several blocks in a range.
    pub async fn block_pagination_v02(
        &self,
        pagination_query: &PaginationQuery<BlockNumber>,
    ) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, "block")
            .query(pagination_query)
            .send()
            .await
    }
}
