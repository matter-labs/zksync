use chrono::Utc;
use zksync_storage::{chain::operations_ext::records::TxReceiptResponse, ConnectionPool};
use zksync_types::{
    forced_exit_requests::{ForcedExitRequest, ForcedExitRequestId},
    tx::TxHash,
    AccountId, Nonce,
};

use zksync_api::core_api_client::CoreApiClient;
use zksync_types::SignedZkSyncTx;

// We could use `db reset` and test the db the same way as in rust_api
// but it seemed to be an overkill here, so it was decided to use
// traits for unit-testing. Also it gives a much broader level of control
// over what's going on
#[async_trait::async_trait]
pub trait CoreInteractionWrapper {
    async fn get_nonce(&self, account_id: AccountId) -> anyhow::Result<Option<Nonce>>;
    async fn get_unconfirmed_requests(&self) -> anyhow::Result<Vec<ForcedExitRequest>>;
    async fn set_fulfilled_at(&self, id: i64) -> anyhow::Result<()>;
    async fn set_fulfilled_by(
        &self,
        id: ForcedExitRequestId,
        value: Option<Vec<TxHash>>,
    ) -> anyhow::Result<()>;
    async fn get_request_by_id(&self, id: i64) -> anyhow::Result<Option<ForcedExitRequest>>;
    async fn get_receipt(&self, tx_hash: TxHash) -> anyhow::Result<Option<TxReceiptResponse>>;
    async fn send_and_save_txs_batch(
        &self,
        request: &ForcedExitRequest,
        txs: Vec<SignedZkSyncTx>,
    ) -> anyhow::Result<Vec<TxHash>>;
    async fn get_oldest_unfulfilled_request(&self) -> anyhow::Result<Option<ForcedExitRequest>>;
    async fn delete_old_unfulfilled_requests(
        &self,
        deleting_threshold: chrono::Duration,
    ) -> anyhow::Result<()>;
}
#[derive(Clone)]
pub struct MempoolCoreInteractionWrapper {
    core_api_client: CoreApiClient,
    connection_pool: ConnectionPool,
}

impl MempoolCoreInteractionWrapper {
    pub fn new(core_api_client: CoreApiClient, connection_pool: ConnectionPool) -> Self {
        Self {
            core_api_client,
            connection_pool,
        }
    }
}

#[async_trait::async_trait]
impl CoreInteractionWrapper for MempoolCoreInteractionWrapper {
    async fn get_nonce(&self, account_id: AccountId) -> anyhow::Result<Option<Nonce>> {
        let mut storage = self.connection_pool.access_storage().await?;
        let mut account_schema = storage.chain().account_schema();

        let sender_state = account_schema
            .last_committed_state_for_account(account_id)
            .await?;

        Ok(sender_state.map(|state| state.nonce))
    }

    async fn get_unconfirmed_requests(&self) -> anyhow::Result<Vec<ForcedExitRequest>> {
        let mut storage = self.connection_pool.access_storage().await?;
        let mut forced_exit_requests_schema = storage.forced_exit_requests_schema();
        forced_exit_requests_schema.get_unconfirmed_requests().await
    }

    async fn set_fulfilled_at(&self, id: i64) -> anyhow::Result<()> {
        let mut storage = self.connection_pool.access_storage().await?;
        let mut fe_schema = storage.forced_exit_requests_schema();

        fe_schema.set_fulfilled_at(id, Utc::now()).await?;

        vlog::info!("FE request with id {} was fulfilled", id);

        Ok(())
    }

    async fn set_fulfilled_by(
        &self,
        id: ForcedExitRequestId,
        value: Option<Vec<TxHash>>,
    ) -> anyhow::Result<()> {
        let mut storage = self.connection_pool.access_storage().await?;
        let mut forced_exit_requests_schema = storage.forced_exit_requests_schema();
        forced_exit_requests_schema
            .set_fulfilled_by(id, value)
            .await
    }

    async fn get_receipt(&self, tx_hash: TxHash) -> anyhow::Result<Option<TxReceiptResponse>> {
        let mut storage = self.connection_pool.access_storage().await?;
        storage
            .chain()
            .operations_ext_schema()
            .tx_receipt(tx_hash.as_ref())
            .await
    }

    async fn get_request_by_id(&self, id: i64) -> anyhow::Result<Option<ForcedExitRequest>> {
        let mut storage = self.connection_pool.access_storage().await?;
        let mut fe_schema = storage.forced_exit_requests_schema();

        let request = fe_schema.get_request_by_id(id).await?;
        Ok(request)
    }

    async fn send_and_save_txs_batch(
        &self,
        request: &ForcedExitRequest,
        txs: Vec<SignedZkSyncTx>,
    ) -> anyhow::Result<Vec<TxHash>> {
        let mut storage = self.connection_pool.access_storage().await?;
        let mut schema = storage.forced_exit_requests_schema();

        let hashes: Vec<TxHash> = txs.iter().map(|tx| tx.hash()).collect();
        self.core_api_client.send_txs_batch(txs, vec![]).await??;

        schema
            .set_fulfilled_by(request.id, Some(hashes.clone()))
            .await?;

        Ok(hashes)
    }

    async fn get_oldest_unfulfilled_request(&self) -> anyhow::Result<Option<ForcedExitRequest>> {
        let mut storage = self.connection_pool.access_storage().await?;
        storage
            .forced_exit_requests_schema()
            .get_oldest_unfulfilled_request()
            .await
    }

    async fn delete_old_unfulfilled_requests(
        &self,
        deleting_threshold: chrono::Duration,
    ) -> anyhow::Result<()> {
        let mut storage = self.connection_pool.access_storage().await?;
        storage
            .forced_exit_requests_schema()
            .delete_old_unfulfilled_requests(deleting_threshold)
            .await
    }
}
