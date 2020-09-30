use crate::mempool::MempoolRequest;
use crate::utils::shared_lru_cache::SharedLruCache;
use actix_cors::Cors;
use actix_web::{
    middleware,
    web::{self},
    App, HttpResponse, HttpServer, Result as ActixResult,
};
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::{runtime::Runtime, time};
use zksync_basic_types::H160;
use zksync_config::ConfigurationOptions;
use zksync_storage::chain::block::records::BlockDetails;
use zksync_storage::chain::operations_ext::{
    records::{PriorityOpReceiptResponse, TxReceiptResponse},
    SearchDirection,
};
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::NetworkStatus;
use zksync_types::{
    Account, AccountId, Address, ExecutedOperations, FranklinPriorityOp, PriorityOp, Token, TokenId,
};

use super::rpc_server::get_ongoing_priority_ops;
use crate::eth_watch::{EthBlockId, EthWatchRequest};
use crate::panic_notify::ThreadPanicNotify;
use zksync_storage::chain::operations_ext::records::{TransactionsHistoryItem, TxByHashResponse};

#[derive(Default, Clone)]
struct SharedNetworkStatus(Arc<RwLock<NetworkStatus>>);

impl SharedNetworkStatus {
    fn read(&self) -> NetworkStatus {
        (*self.0.as_ref().read().unwrap()).clone()
    }
}

fn remove_prefix(query: &str) -> &str {
    if query.starts_with("0x") {
        &query[2..]
    } else if query.starts_with("sync-bl:") || query.starts_with("sync-tx:") {
        &query[8..]
    } else {
        &query
    }
}

fn try_parse_hash(query: &str) -> Option<Vec<u8>> {
    const HASH_SIZE: usize = 32; // 32 bytes

    let query = remove_prefix(query);
    let b = hex::decode(query).ok()?;

    if b.len() == HASH_SIZE {
        Some(b)
    } else {
        None
    }
}

/// Checks if block is finalized, meaning that
/// both Verify operation is performed for it, and this
/// operation is anchored on the Ethereum blockchain.
fn block_verified(block: &BlockDetails) -> bool {
    // We assume that it's not possible to have block that is
    // verified and not committed.
    block.verified_at.is_some() && block.verify_tx_hash.is_some()
}

/// Caches used by REST API server.
#[derive(Debug, Clone)]
struct Caches {
    pub transaction_receipts: SharedLruCache<Vec<u8>, TxReceiptResponse>,
    pub priority_op_receipts: SharedLruCache<u32, PriorityOpReceiptResponse>,
    pub block_executed_ops: SharedLruCache<u32, Vec<ExecutedOperations>>,
    pub blocks_info: SharedLruCache<u32, BlockDetails>,
    pub blocks_by_height_or_hash: SharedLruCache<String, BlockDetails>,
}

impl Caches {
    pub fn new(caches_size: usize) -> Self {
        Self {
            transaction_receipts: SharedLruCache::new(caches_size),
            priority_op_receipts: SharedLruCache::new(caches_size),
            block_executed_ops: SharedLruCache::new(caches_size),
            blocks_info: SharedLruCache::new(caches_size),
            blocks_by_height_or_hash: SharedLruCache::new(caches_size),
        }
    }
}

/// AppState is a collection of records cloned by each thread to shara data between them
#[derive(Clone)]
struct AppState {
    caches: Caches,
    connection_pool: ConnectionPool,
    network_status: SharedNetworkStatus,
    contract_address: String,
    mempool_request_sender: mpsc::Sender<MempoolRequest>,
    eth_watcher_request_sender: mpsc::Sender<EthWatchRequest>,
    config_options: ConfigurationOptions,
}

impl AppState {
    async fn access_storage(&self) -> ActixResult<StorageProcessor<'_>> {
        self.connection_pool
            .access_storage_fragile()
            .await
            .map_err(|err| {
                vlog::warn!("DB await timeout: '{}';", err);
                HttpResponse::RequestTimeout().finish().into()
            })
    }

    fn db_error(error: failure::Error) -> HttpResponse {
        vlog::warn!("DB error: '{}';", error);
        HttpResponse::InternalServerError().finish()
    }

    // Spawns future updating SharedNetworkStatus in the current `actix::System`
    fn spawn_network_status_updater(&self, panic_notify: mpsc::Sender<bool>) {
        let state = self.clone();

        std::thread::Builder::new()
            .name("rest-state-updater".to_string())
            .spawn(move || {
                let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

                let mut runtime = Runtime::new().expect("tokio runtime creation");

                let state_update_task = async move {
                    let mut timer = time::interval(Duration::from_millis(1000));
                    loop {
                        timer.tick().await;

                        let mut storage = match state.connection_pool.access_storage().await {
                            Ok(storage) => storage,
                            Err(err) => {
                                log::warn!("Unable to update the network status. Storage access failed: {}", err);
                                continue;
                            }
                        };

                        let mut transaction =  match storage.start_transaction().await {
                            Ok(transaction) => transaction,
                            Err(err) => {
                                log::warn!("Unable to update the network status. Storage access failed: {}", err);
                                continue;
                            }
                        };

                        let last_verified = transaction
                            .chain()
                            .block_schema()
                            .get_last_verified_block()
                            .await
                            .unwrap_or(0);

                        let last_committed = transaction
                            .chain()
                            .block_schema()
                            .get_last_committed_block()
                            .await
                            .unwrap_or(0);

                        let total_transactions = transaction
                            .chain()
                            .stats_schema()
                            .count_total_transactions()
                            .await
                            .unwrap_or(0);

                        let outstanding_txs = transaction
                            .chain()
                            .stats_schema()
                            .count_outstanding_proofs(last_verified)
                            .await
                            .unwrap_or(0);

                        let status = NetworkStatus {
                            next_block_at_max: None,
                            last_committed,
                            last_verified,
                            total_transactions,
                            outstanding_txs,
                        };

                        transaction.commit().await.unwrap_or_default();

                        // save status to state
                        *state.network_status.0.as_ref().write().unwrap() = status;
                    }
                };
                runtime.block_on(state_update_task);
            })
            .expect("State update thread");
    }

    // cache access functions
    async fn get_tx_receipt(
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

    async fn get_priority_op_receipt(
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

    async fn get_block_executed_ops(
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

    async fn get_block_info(
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

    async fn get_block_by_height_or_hash(
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TestnetConfigResponse {
    contract_address: String,
}

async fn handle_get_testnet_config(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let contract_address = data.contract_address.clone();
    Ok(HttpResponse::Ok().json(TestnetConfigResponse { contract_address }))
}

async fn handle_get_network_status(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let network_status = data.network_status.read();
    Ok(HttpResponse::Ok().json(network_status))
}

#[derive(Debug, Serialize)]
struct WithdrawalProcessingTimeResponse {
    normal: u64,
    fast: u64,
}

async fn handle_get_withdrawal_processing_time(
    data: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    let miniblock_timings = &data.config_options.miniblock_timings;
    let processing_time = WithdrawalProcessingTimeResponse {
        normal: (miniblock_timings.miniblock_iteration_interval
            * miniblock_timings.max_miniblock_iterations as u32)
            .as_secs(),
        fast: (miniblock_timings.miniblock_iteration_interval
            * miniblock_timings.fast_miniblock_iterations as u32)
            .as_secs(),
    };

    Ok(HttpResponse::Ok().json(processing_time))
}

#[derive(Debug, Serialize)]
struct AccountStateResponse {
    // None if account is not created yet.
    id: Option<AccountId>,
    commited: Account,
    verified: Account,
}

async fn handle_get_tokens(data: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let mut storage = data.access_storage().await?;
    let tokens = storage
        .tokens_schema()
        .load_tokens()
        .await
        .map_err(AppState::db_error)?;

    let mut vec_tokens = tokens.values().cloned().collect::<Vec<_>>();
    vec_tokens.sort_by_key(|t| t.id);

    Ok(HttpResponse::Ok().json(vec_tokens))
}

/// Sends an EthWatchRequest asking for an unconfirmed priority op
/// with given hash. If no such priority op exists, returns Ok(None).
pub(crate) async fn get_unconfirmed_op_by_hash(
    eth_watcher_request_sender: &mpsc::Sender<EthWatchRequest>,
    eth_hash: &[u8],
) -> Result<Option<(EthBlockId, PriorityOp)>, failure::Error> {
    let mut eth_watcher_request_sender = eth_watcher_request_sender.clone();

    let eth_watcher_response = oneshot::channel();

    // Find unconfirmed op with given hash
    eth_watcher_request_sender
        .send(EthWatchRequest::GetUnconfirmedOpByHash {
            eth_hash: eth_hash.to_vec(),
            resp: eth_watcher_response.0,
        })
        .await
        .map_err(|err| {
            vlog::warn!(
                "Internal Server Error: '{}'; input: ({})",
                err,
                hex::encode(&eth_hash)
            );

            failure::format_err!("Internal Server Error: '{}'", err)
        })?;

    eth_watcher_response
        .1
        .await
        .map_err(|err| failure::format_err!("Failed to send response: {}", err))
}

/// Converts a non-executed priority operation into a
/// `TxByHashResponse` so the user can track its status in explorer.
/// It also adds new field `tx.eth_block_number`, which is normally not there,
/// which is the block number of Ethereum tx of the priority operation,
/// it enables tracking the number of blocks (confirmations) user needs to wait
/// before the priority op is included into zkSync block.
/// Currently returns Some(TxByHashResponse) if PriorityOp is Deposit, and None in other cases.
fn deposit_op_to_tx_by_hash(
    tokens: &HashMap<TokenId, Token>,
    op: &PriorityOp,
    eth_block: EthBlockId,
) -> Option<TxByHashResponse> {
    match &op.data {
        FranklinPriorityOp::Deposit(deposit) => {
            // As the time of creation is indefinite, we always will provide the current time.
            let current_time = chrono::Utc::now();
            let naive_current_time =
                chrono::NaiveDateTime::from_timestamp(current_time.timestamp(), 0);

            // Account ID may not exist for depositing ops, so it'll be `null`.
            let account_id: Option<u32> = None;

            let token_symbol = tokens.get(&deposit.token).map(|t| t.symbol.clone());

            // Copy the JSON representation of the executed tx so the appearance
            // will be the same as for txs from storage.
            let tx_json = serde_json::json!({
                "account_id": account_id,
                "priority_op": {
                    "amount": deposit.amount,
                    "from": deposit.from,
                    "to": deposit.to,
                    "token": token_symbol
                },
                "type": "Deposit",
                "eth_block_number": eth_block,
            });

            Some(TxByHashResponse {
                tx_type: "Deposit".into(),
                from: format!("{:?}", deposit.from),
                to: format!("{:?}", deposit.to),
                token: deposit.token as i32,
                amount: deposit.amount.to_string(),
                fee: None,
                block_number: -1,
                nonce: -1,
                created_at: naive_current_time
                    .format("%Y-%m-%dT%H:%M:%S%.6f")
                    .to_string(),
                fail_reason: None,
                tx: tx_json,
            })
        }
        _ => None,
    }
}

/// Converts a non-executed priority operation into a
/// `TransactionsHistoryItem` to include it into the list of transactions
/// in the client.
fn priority_op_to_tx_history(
    tokens: &HashMap<TokenId, Token>,
    eth_block: u64,
    op: &PriorityOp,
) -> TransactionsHistoryItem {
    let deposit = op
        .data
        .try_get_deposit()
        .expect("Not a deposit sent by eth_watch");
    let token_symbol = tokens
        .get(&deposit.token)
        .map(|t| t.symbol.clone())
        .unwrap_or_else(|| "unknown".into());

    let hash_str = format!("0x{}", hex::encode(&op.eth_hash));
    let pq_id = Some(op.serial_id as i64);

    // Account ID may not exist for depositing ops, so it'll be `null`.
    let account_id: Option<u32> = None;

    // Copy the JSON representation of the executed tx so the appearance
    // will be the same as for txs from storage.
    let tx_json = serde_json::json!({
        "account_id": account_id,
        "priority_op": {
            "amount": deposit.amount.to_string(),
            "from": deposit.from,
            "to": deposit.to,
            "token": token_symbol
        },
        "type": "Deposit"
    });

    // As the time of creation is indefinite, we always will provide the current time.
    let current_time = chrono::Utc::now();

    TransactionsHistoryItem {
        tx_id: "-".into(),
        hash: Some(hash_str),
        eth_block: Some(eth_block as i64),
        pq_id,
        tx: tx_json,
        success: None,
        fail_reason: None,
        commited: false,
        verified: false,
        created_at: current_time,
    }
}

async fn handle_get_account_transactions_history(
    data: web::Data<AppState>,
    request_path: web::Path<(Address, u64, u64)>,
) -> ActixResult<HttpResponse> {
    let (address, mut offset, mut limit) = request_path.into_inner();

    const MAX_LIMIT: u64 = 100;
    if limit > MAX_LIMIT {
        return Err(HttpResponse::BadRequest().finish().into());
    }

    let tokens = data
        .access_storage()
        .await?
        .tokens_schema()
        .load_tokens()
        .await
        .map_err(|err| {
            vlog::warn!(
                "Internal Server Error: '{}'; input: ({}, {}, {})",
                err,
                address,
                offset,
                limit,
            );
            HttpResponse::InternalServerError().finish()
        })?;

    let eth_watcher_request_sender = data.eth_watcher_request_sender.clone();
    // Fetch ongoing deposits, since they must be reported within the transactions history.
    let mut ongoing_ops = get_ongoing_priority_ops(&eth_watcher_request_sender, address)
        .await
        .map_err(|err| {
            vlog::warn!(
                "Internal Server Error: '{}'; input: ({}, {}, {})",
                err,
                address,
                offset,
                limit,
            );
            HttpResponse::InternalServerError().finish()
        })?;

    // Sort operations by block number from smaller (older) to greater (newer).
    ongoing_ops.sort_by(|lhs, rhs| rhs.0.cmp(&lhs.0));

    // Collect the unconfirmed priority operations with respect to the
    // `offset` and `limit` parameters.
    let mut ongoing_transactions_history: Vec<_> = ongoing_ops
        .iter()
        .map(|(block, op)| priority_op_to_tx_history(&tokens, *block, op))
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    // Now we must include committed transactions, thus we have to modify `offset` and
    // `limit` values.
    if !ongoing_transactions_history.is_empty() {
        // We've taken at least one transaction, this means
        // offset is consumed completely, and limit is reduced.
        offset = 0;
        limit -= ongoing_transactions_history.len() as u64;
    } else {
        // Decrement the offset by the number of pending deposits
        // that are soon to be added to the db. `ongoing_ops` consists
        // of the deposits related to a target account only.
        let num_account_ongoing_deposits = ongoing_ops.len() as u64;
        offset = offset.saturating_sub(num_account_ongoing_deposits);
    }

    let mut transactions_history = data
        .access_storage()
        .await?
        .chain()
        .operations_ext_schema()
        .get_account_transactions_history(&address, offset, limit)
        .await
        .map_err(|err| {
            vlog::warn!(
                "Internal Server Error: '{}'; input: ({}, {}, {})",
                err,
                address,
                offset,
                limit,
            );
            HttpResponse::InternalServerError().finish()
        })?;

    // Append ongoing operations to the end of the end of the list, as the history
    // goes from oldest tx to the newest tx.
    transactions_history.append(&mut ongoing_transactions_history);

    Ok(HttpResponse::Ok().json(transactions_history))
}

#[derive(Debug, Deserialize)]
struct TxHistoryQuery {
    tx_id: Option<String>,
    limit: Option<u64>,
}

async fn parse_tx_id(data: &str, storage: &mut StorageProcessor<'_>) -> ActixResult<(u64, u64)> {
    if data.is_empty() || data == "-" {
        let last_block_id = storage
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: ({})", err, data,);
                HttpResponse::InternalServerError().finish()
            })?;

        let next_block_id = last_block_id + 1;

        return Ok((next_block_id as u64, 0));
    }

    let parts: Vec<u64> = data
        .split(',')
        .map(|val| val.parse().map_err(|_| HttpResponse::BadRequest().finish()))
        .collect::<Result<Vec<u64>, HttpResponse>>()?;

    if parts.len() != 2 {
        return Err(HttpResponse::BadRequest().finish().into());
    }

    Ok((parts[0], parts[1]))
}

async fn handle_get_account_transactions_history_older_than(
    data: web::Data<AppState>,
    request_path: web::Path<Address>,
    request_query: web::Query<TxHistoryQuery>,
) -> ActixResult<HttpResponse> {
    let address = request_path.into_inner();
    let tx_id = request_query
        .tx_id
        .as_ref()
        .map(|s| s.as_ref())
        .unwrap_or("-");
    let limit = request_query.limit.unwrap_or(MAX_LIMIT);

    const MAX_LIMIT: u64 = 100;
    if limit > MAX_LIMIT {
        return Err(HttpResponse::BadRequest().finish().into());
    }
    let mut storage = data.access_storage().await?;
    let mut transaction = storage
        .start_transaction()
        .await
        .map_err(AppState::db_error)?;

    let tx_id = parse_tx_id(&tx_id, &mut transaction).await?;

    let direction = SearchDirection::Older;
    let transactions_history = transaction
        .chain()
        .operations_ext_schema()
        .get_account_transactions_history_from(&address, tx_id, direction, limit)
        .await
        .map_err(|err| {
            vlog::warn!(
                "Internal Server Error: '{}'; input: ({}, {:?}, {})",
                err,
                address,
                tx_id,
                limit,
            );
            HttpResponse::InternalServerError().finish()
        })?;

    transaction.commit().await.map_err(AppState::db_error)?;

    Ok(HttpResponse::Ok().json(transactions_history))
}

async fn handle_get_account_transactions_history_newer_than(
    data: web::Data<AppState>,
    request_path: web::Path<Address>,
    request_query: web::Query<TxHistoryQuery>,
) -> ActixResult<HttpResponse> {
    let address = request_path.into_inner();
    let tx_id = request_query
        .tx_id
        .as_ref()
        .map(|s| s.as_ref())
        .unwrap_or("-");
    let mut limit = request_query.limit.unwrap_or(MAX_LIMIT);

    const MAX_LIMIT: u64 = 100;
    if limit > MAX_LIMIT {
        return Err(HttpResponse::BadRequest().finish().into());
    }

    let direction = SearchDirection::Newer;
    let mut transactions_history = {
        let mut storage = data.access_storage().await?;
        let tx_id = parse_tx_id(&tx_id, &mut storage).await?;
        storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history_from(&address, tx_id, direction, limit)
            .await
            .map_err(|err| {
                vlog::warn!(
                    "Internal Server Error: '{}'; input: ({}, {:?}, {})",
                    err,
                    address,
                    tx_id,
                    limit,
                );
                HttpResponse::InternalServerError().finish()
            })?
    };

    limit -= transactions_history.len() as u64;

    if limit > 0 {
        // We've got some free space, so load unconfirmed operations to
        // fill the rest of the limit.

        let eth_watcher_request_sender = data.eth_watcher_request_sender.clone();
        // Fetch ongoing deposits, since they must be reported within the transactions history.
        let mut ongoing_ops = get_ongoing_priority_ops(&eth_watcher_request_sender, address)
            .await
            .map_err(|err| {
                vlog::warn!(
                    "Internal Server Error: '{}'; input: ({}, {:?}, {})",
                    err,
                    address,
                    tx_id,
                    limit,
                );
                HttpResponse::InternalServerError().finish()
            })?;

        // Sort operations by block number from smaller (older) to greater (newer).
        ongoing_ops.sort_by(|lhs, rhs| rhs.0.cmp(&lhs.0));

        let tokens = data
            .access_storage()
            .await?
            .tokens_schema()
            .load_tokens()
            .await
            .map_err(|err| {
                vlog::warn!(
                    "Internal Server Error: '{}'; input: ({}, {:?}, {})",
                    err,
                    address,
                    tx_id,
                    limit,
                );
                HttpResponse::InternalServerError().finish()
            })?;
        // Collect the unconfirmed priority operations with respect to the
        // `limit` parameters.
        let mut txs: Vec<_> = ongoing_ops
            .iter()
            .map(|(block, op)| priority_op_to_tx_history(&tokens, *block, op))
            .take(limit as usize)
            .collect();

        // Merge `txs` and `transactions_history` and reassign the `transactions_history` to the
        // merged list.
        // Unprocessed operations must be in the end (as the newest ones).
        transactions_history.append(&mut txs);
    }

    Ok(HttpResponse::Ok().json(transactions_history))
}

async fn handle_get_executed_transaction_by_hash(
    data: web::Data<AppState>,
    tx_hash_hex: web::Path<String>,
) -> ActixResult<HttpResponse> {
    if tx_hash_hex.len() < 2 {
        return Err(HttpResponse::BadRequest().finish().into());
    }
    let transaction_hash = hex::decode(&tx_hash_hex.into_inner()[2..])
        .map_err(|_| HttpResponse::BadRequest().finish())?;

    let tx_receipt = data.get_tx_receipt(transaction_hash).await?;

    if let Some(tx) = tx_receipt {
        Ok(HttpResponse::Ok().json(tx))
    } else {
        Ok(HttpResponse::Ok().json(()))
    }
}

async fn handle_get_tx_by_hash(
    data: web::Data<AppState>,
    hash_hex_with_prefix: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let hash =
        try_parse_hash(&hash_hex_with_prefix).ok_or_else(|| HttpResponse::BadRequest().finish())?;

    let mut res;

    res = data
        .access_storage()
        .await?
        .chain()
        .operations_ext_schema()
        .get_tx_by_hash(hash.as_slice())
        .await
        .map_err(|err| {
            vlog::warn!(
                "Internal Server Error: '{}'; input: {}",
                err,
                hex::encode(&hash)
            );
            HttpResponse::InternalServerError().finish()
        })?;

    // If storage returns Some, return the result.
    if res.is_some() {
        return Ok(HttpResponse::Ok().json(res));
    }

    // Or try to find this priority op in eth_watcher
    let eth_watcher_request_sender = data.eth_watcher_request_sender.clone();
    let unconfirmed_op = get_unconfirmed_op_by_hash(&eth_watcher_request_sender, &hash)
        .await
        .map_err(|err| {
            vlog::warn!(
                "Internal Server Error: '{}'; input({})",
                err,
                hex::encode(&hash)
            );
            HttpResponse::InternalServerError().finish()
        })?;

    // If eth watcher has a priority op with given hash, transform it
    // to TxByHashResponse and assign it to res.
    if let Some((eth_block, priority_op)) = unconfirmed_op {
        let tokens = data
            .access_storage()
            .await?
            .tokens_schema()
            .load_tokens()
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}';", err);
                HttpResponse::InternalServerError().finish()
            })?;

        res = deposit_op_to_tx_by_hash(&tokens, &priority_op, eth_block);
    }

    // Return res
    Ok(HttpResponse::Ok().json(res))
}

async fn handle_get_priority_op_receipt(
    data: web::Data<AppState>,
    id: web::Path<u32>,
) -> ActixResult<HttpResponse> {
    let id = id.into_inner();
    let receipt = data.get_priority_op_receipt(id).await?;

    Ok(HttpResponse::Ok().json(receipt))
}

async fn handle_get_transaction_by_id(
    data: web::Data<AppState>,
    path: web::Path<(u32, u32)>,
) -> ActixResult<HttpResponse> {
    let (block_id, tx_id) = path.into_inner();

    let exec_ops = data.get_block_executed_ops(block_id).await?;

    if let Some(exec_op) = exec_ops.get(tx_id as usize) {
        Ok(HttpResponse::Ok().json(exec_op))
    } else {
        Err(HttpResponse::NotFound().finish().into())
    }
}

#[derive(Deserialize)]
struct HandleBlocksQuery {
    max_block: Option<u32>,
    limit: Option<u32>,
}

async fn handle_get_blocks(
    data: web::Data<AppState>,
    query: web::Query<HandleBlocksQuery>,
) -> ActixResult<HttpResponse> {
    let max_block = query.max_block.unwrap_or(999_999_999);
    let limit = query.limit.unwrap_or(20);
    if limit > 100 {
        return Err(HttpResponse::BadRequest().finish().into());
    }
    let mut storage = data.access_storage().await?;

    let resp = storage
        .chain()
        .block_schema()
        .load_block_range(max_block, limit)
        .await
        .map_err(|err| {
            vlog::warn!(
                "Internal Server Error: '{}'; input: ({}, {})",
                err,
                max_block,
                limit
            );
            HttpResponse::InternalServerError().finish()
        })?;
    Ok(HttpResponse::Ok().json(resp))
}

async fn handle_get_block_by_id(
    data: web::Data<AppState>,
    block_id: web::Path<u32>,
) -> ActixResult<HttpResponse> {
    let block_id = block_id.into_inner();
    let block = data.get_block_info(block_id).await?;
    if let Some(block) = block {
        Ok(HttpResponse::Ok().json(block))
    } else {
        Err(HttpResponse::NotFound().finish().into())
    }
}

async fn handle_get_block_transactions(
    data: web::Data<AppState>,
    path: web::Path<u32>,
) -> ActixResult<HttpResponse> {
    let block_number = path.into_inner();

    let mut storage = data.access_storage().await?;

    let txs = storage
        .chain()
        .block_schema()
        .get_block_transactions(block_number)
        .await
        .map_err(|err| {
            vlog::warn!("Internal Server Error: '{}'; input: {}", err, block_number);
            HttpResponse::InternalServerError().finish()
        })?;

    Ok(HttpResponse::Ok().json(txs))
}

#[derive(Deserialize)]
struct BlockExplorerSearchQuery {
    query: String,
}

async fn handle_block_explorer_search(
    data: web::Data<AppState>,
    query: web::Query<BlockExplorerSearchQuery>,
) -> ActixResult<HttpResponse> {
    let query = query.into_inner().query;
    let block = data.get_block_by_height_or_hash(query).await?;

    if let Some(block) = block {
        Ok(HttpResponse::Ok().json(block))
    } else {
        Err(HttpResponse::NotFound().finish().into())
    }
}

async fn start_server(state: AppState, bind_to: SocketAddr) {
    let logger_format = crate::api_server::loggers::rest::get_logger_format();
    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(middleware::Logger::new(&logger_format))
            .wrap(Cors::new().send_wildcard().max_age(3600).finish())
            .service(
                web::scope("/api/v0.1")
                    .route("/testnet_config", web::get().to(handle_get_testnet_config))
                    .route("/status", web::get().to(handle_get_network_status))
                    .route("/tokens", web::get().to(handle_get_tokens))
                    .route(
                        "/account/{address}/history/{offset}/{limit}",
                        web::get().to(handle_get_account_transactions_history),
                    )
                    .route(
                        "/account/{address}/history/older_than",
                        web::get().to(handle_get_account_transactions_history_older_than),
                    )
                    .route(
                        "/account/{address}/history/newer_than",
                        web::get().to(handle_get_account_transactions_history_newer_than),
                    )
                    .route(
                        "/transactions/{tx_hash}",
                        web::get().to(handle_get_executed_transaction_by_hash),
                    )
                    .route(
                        "/transactions_all/{tx_hash}",
                        web::get().to(handle_get_tx_by_hash),
                    )
                    .route(
                        "/priority_operations/{pq_id}/",
                        web::get().to(handle_get_priority_op_receipt),
                    )
                    .route(
                        "/blocks/{block_id}/transactions/{tx_id}",
                        web::get().to(handle_get_transaction_by_id),
                    )
                    .route(
                        "/blocks/{block_id}/transactions",
                        web::get().to(handle_get_block_transactions),
                    )
                    .route("/blocks/{block_id}", web::get().to(handle_get_block_by_id))
                    .route("/blocks", web::get().to(handle_get_blocks))
                    .route("/search", web::get().to(handle_block_explorer_search))
                    .route(
                        "/withdrawal_processing_time",
                        web::get().to(handle_get_withdrawal_processing_time),
                    ),
            )
            // Endpoint needed for js isReachable
            .route(
                "/favicon.ico",
                web::get().to(|| HttpResponse::Ok().finish()),
            )
    })
    .bind(bind_to)
    .unwrap()
    .shutdown_timeout(1)
    .run()
    .await
    .expect("REST API server has crashed");
}

/// Start HTTP REST API
pub(super) fn start_server_thread_detached(
    connection_pool: ConnectionPool,
    listen_addr: SocketAddr,
    contract_address: H160,
    mempool_request_sender: mpsc::Sender<MempoolRequest>,
    eth_watcher_request_sender: mpsc::Sender<EthWatchRequest>,
    panic_notify: mpsc::Sender<bool>,
    config_options: ConfigurationOptions,
) {
    std::thread::Builder::new()
        .name("actix-rest-api".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

            actix_rt::System::new("api-server").block_on(async move {
                let state = AppState {
                    caches: Caches::new(config_options.api_requests_caches_size),
                    connection_pool,
                    network_status: SharedNetworkStatus::default(),
                    contract_address: format!("{:?}", contract_address),
                    mempool_request_sender,
                    eth_watcher_request_sender,
                    config_options,
                };
                state.spawn_network_status_updater(panic_notify);

                start_server(state, listen_addr).await;
            });
        })
        .expect("Api server thread");
}
