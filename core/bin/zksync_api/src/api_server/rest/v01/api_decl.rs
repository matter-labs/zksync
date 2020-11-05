//! Declaration of the API structure.

use crate::{
    api_server::rest::{
        helpers::*,
        v01::{caches::Caches, network_status::SharedNetworkStatus},
    },
    core_api_client::{CoreApiClient, EthBlockId},
};
use actix_web::{web, HttpResponse, Result as ActixResult};
use futures::channel::mpsc;
use zksync_config::ConfigurationOptions;
use zksync_storage::{
    chain::{
        block::records::BlockDetails,
        operations_ext::records::{PriorityOpReceiptResponse, TxReceiptResponse},
    },
    ConnectionPool, StorageProcessor,
};
use zksync_types::{block::ExecutedOperations, PriorityOp, H160, H256};

/// `ApiV01` structure contains the implementation of `/api/v0.1` endpoints set.
/// It is considered (somewhat) stable and will be supported for a while.
///
/// Once a new API is designed, it will be created as `ApiV1` structure, so that
/// each API version is encapsulated inside one type.
#[derive(Debug, Clone)]
pub struct ApiV01 {
    pub(crate) caches: Caches,
    pub(crate) connection_pool: ConnectionPool,
    pub(crate) api_client: CoreApiClient,
    pub(crate) network_status: SharedNetworkStatus,
    pub(crate) contract_address: String,
    pub(crate) config_options: ConfigurationOptions,
}

impl ApiV01 {
    pub fn new(
        connection_pool: ConnectionPool,
        contract_address: H160,
        config_options: ConfigurationOptions,
    ) -> Self {
        let api_client = CoreApiClient::new(config_options.core_server_url.clone());
        Self {
            caches: Caches::new(config_options.api_requests_caches_size),
            connection_pool,
            api_client,
            network_status: SharedNetworkStatus::default(),
            contract_address: format!("{:?}", contract_address),
            config_options,
        }
    }

    /// Creates an actix-web `Scope`, which can be mounted to the Http server.
    pub fn into_scope(self) -> actix_web::Scope {
        web::scope("/api/v0.1")
            .data(self)
            .route("/testnet_config", web::get().to(Self::testnet_config))
            .route("/status", web::get().to(Self::status))
            .route("/tokens", web::get().to(Self::tokens))
            .route(
                "/account/{address}/history/{offset}/{limit}",
                web::get().to(Self::tx_history),
            )
            .route(
                "/account/{address}/history/older_than",
                web::get().to(Self::tx_history_older_than),
            )
            .route(
                "/account/{address}/history/newer_than",
                web::get().to(Self::tx_history_newer_than),
            )
            .route(
                "/transactions/{tx_hash}",
                web::get().to(Self::executed_tx_by_hash),
            )
            .route(
                "/transactions_all/{tx_hash}",
                web::get().to(Self::tx_by_hash),
            )
            .route(
                "/priority_operations/{pq_id}/",
                web::get().to(Self::priority_op),
            )
            .route(
                "/blocks/{block_id}/transactions/{tx_id}",
                web::get().to(Self::block_tx),
            )
            .route(
                "/blocks/{block_id}/transactions",
                web::get().to(Self::block_transactions),
            )
            .route("/blocks/{block_id}", web::get().to(Self::block_by_id))
            .route("/blocks", web::get().to(Self::blocks))
            .route("/search", web::get().to(Self::explorer_search))
            .route(
                "/withdrawal_processing_time",
                web::get().to(Self::withdrawal_processing_time),
            )
    }

    pub(crate) async fn access_storage(&self) -> ActixResult<StorageProcessor<'_>> {
        self.connection_pool.access_storage().await.map_err(|err| {
            vlog::warn!("DB await timeout: '{}';", err);
            HttpResponse::RequestTimeout().finish().into()
        })
    }

    pub(crate) fn db_error(error: anyhow::Error) -> HttpResponse {
        vlog::warn!("DB error: '{}';", error);
        HttpResponse::InternalServerError().finish()
    }

    // Spawns future updating SharedNetworkStatus in the current `actix::System`
    pub fn spawn_network_status_updater(&self, panic_notify: mpsc::Sender<bool>) {
        self.network_status
            .clone()
            .start_updater_detached(panic_notify, self.connection_pool.clone());
    }

    // cache access functions
    pub async fn get_tx_receipt(
        &self,
        transaction_hash: Vec<u8>,
    ) -> Result<Option<TxReceiptResponse>, actix_web::error::Error> {
        if let Some(tx_receipt) = self.caches.transaction_receipts.get(&transaction_hash) {
            return Ok(Some(tx_receipt));
        }

        let mut storage = self.access_storage().await?;
        let tx_receipt = storage
            .chain()
            .operations_ext_schema()
            .tx_receipt(transaction_hash.as_slice())
            .await
            .unwrap_or(None);

        if let Some(tx_receipt) = tx_receipt.clone() {
            // Unverified blocks can still change, so we can't cache them.
            if tx_receipt.verified {
                self.caches
                    .transaction_receipts
                    .insert(transaction_hash, tx_receipt);
            }
        }

        Ok(tx_receipt)
    }

    pub async fn get_priority_op_receipt(
        &self,
        id: u32,
    ) -> Result<PriorityOpReceiptResponse, actix_web::error::Error> {
        if let Some(receipt) = self.caches.priority_op_receipts.get(&id) {
            return Ok(receipt);
        }

        let mut storage = self.access_storage().await?;
        let receipt = storage
            .chain()
            .operations_ext_schema()
            .get_priority_op_receipt(id)
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: {}", err, id);
                HttpResponse::InternalServerError().finish()
            })?;

        // Unverified blocks can still change, so we can't cache them.
        if receipt.verified {
            self.caches.priority_op_receipts.insert(id, receipt.clone());
        }

        Ok(receipt)
    }

    pub async fn get_block_executed_ops(
        &self,
        block_id: u32,
    ) -> Result<Vec<ExecutedOperations>, actix_web::error::Error> {
        if let Some(executed_ops) = self.caches.block_executed_ops.get(&block_id) {
            return Ok(executed_ops);
        }

        let mut storage = self.access_storage().await?;
        let mut transaction = storage.start_transaction().await.map_err(Self::db_error)?;
        let executed_ops = transaction
            .chain()
            .block_schema()
            .get_block_executed_ops(block_id)
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: {}", err, block_id);
                HttpResponse::InternalServerError().finish()
            })?;

        if let Ok(block_details) = transaction
            .chain()
            .block_schema()
            .load_block_range(block_id, 1)
            .await
        {
            // Unverified blocks can still change, so we can't cache them.
            if !block_details.is_empty() && block_verified(&block_details[0]) {
                self.caches
                    .block_executed_ops
                    .insert(block_id, executed_ops.clone());
            }
        }
        transaction.commit().await.unwrap_or_default();

        Ok(executed_ops)
    }

    pub async fn get_block_info(
        &self,
        block_id: u32,
    ) -> Result<Option<BlockDetails>, actix_web::error::Error> {
        if let Some(block) = self.caches.blocks_info.get(&block_id) {
            return Ok(Some(block));
        }

        let mut storage = self.access_storage().await?;
        let mut blocks = storage
            .chain()
            .block_schema()
            .load_block_range(block_id, 1)
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: {}", err, block_id);
                HttpResponse::InternalServerError().finish()
            })?;

        if !blocks.is_empty()
            && block_verified(&blocks[0])
            && blocks[0].block_number == block_id as i64
        {
            self.caches
                .blocks_info
                .insert(block_id as u32, blocks[0].clone());
        }

        Ok(blocks.pop())
    }

    pub async fn get_block_by_height_or_hash(
        &self,
        query: String,
    ) -> Result<Option<BlockDetails>, actix_web::error::Error> {
        if let Some(block) = self.caches.blocks_by_height_or_hash.get(&query) {
            return Ok(Some(block));
        }

        let mut storage = self.access_storage().await?;
        let block = storage
            .chain()
            .block_schema()
            .find_block_by_height_or_hash(query.clone())
            .await;

        if let Some(block) = block.clone() {
            if block_verified(&block) {
                self.caches.blocks_by_height_or_hash.insert(query, block);
            }
        }

        Ok(block)
    }

    /// Sends an EthWatchRequest asking for an unconfirmed priority op
    /// with given hash. If no such priority op exists, returns Ok(None).
    pub(crate) async fn get_unconfirmed_op_by_hash(
        &self,
        eth_tx_hash: H256,
    ) -> Result<Option<(EthBlockId, PriorityOp)>, anyhow::Error> {
        self.api_client.get_unconfirmed_op(eth_tx_hash).await
    }
}
