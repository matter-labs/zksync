use crate::rest::client::{Client, Result};

use zksync_api_types::v02::{
    pagination::{ApiEither, PaginationQuery},
    Response,
};
use zksync_types::{tx::TxHash, SerialId};

impl Client {
    pub async fn account_info(
        &self,
        account_id_or_address: &str,
        state_type: &str,
    ) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("accounts/{}/{}", account_id_or_address, state_type),
        )
        .send()
        .await
    }

    pub async fn account_full_info(&self, account_id_or_address: &str) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("accounts/{}", account_id_or_address),
        )
        .send()
        .await
    }

    pub async fn account_txs(
        &self,
        pagination_query: &PaginationQuery<ApiEither<TxHash>>,
        account_id_or_address: &str,
    ) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("accounts/{}/transactions", account_id_or_address),
        )
        .query(&pagination_query)
        .send()
        .await
    }

    pub async fn account_pending_txs(
        &self,
        pagination_query: &PaginationQuery<ApiEither<SerialId>>,
        account_id_or_address: &str,
    ) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("accounts/{}/transactions/pending", account_id_or_address),
        )
        .query(pagination_query)
        .send()
        .await
    }
}
