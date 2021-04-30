use crate::rest::client::{Client, Result};

use zksync_api_types::v02::{pagination::PaginationQuery, Response};
use zksync_types::tx::TxHash;

impl Client {
    pub async fn account_info_v02(
        &self,
        account_id_or_address: &str,
        state_type: &str,
    ) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("account/{}/{}", account_id_or_address, state_type),
        )
        .send()
        .await
    }

    pub async fn account_txs(
        &self,
        pagination_query: &PaginationQuery<TxHash>,
        account_id_or_address: &str,
    ) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("block/{}/transactions", account_id_or_address),
        )
        .query(&pagination_query)
        .send()
        .await
    }

    pub async fn account_pending_txs(
        &self,
        pagination_query: &PaginationQuery<TxHash>,
        account_id_or_address: &str,
    ) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("block/{}/transactions/pending", account_id_or_address),
        )
        .query(pagination_query)
        .send()
        .await
    }
}
