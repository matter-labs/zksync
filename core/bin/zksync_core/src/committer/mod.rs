// Built-in uses
use std::time::{Duration, Instant};
// External uses
use futures::{channel::mpsc::Receiver, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{task::JoinHandle, time};

// Workspace uses
use zksync_config::ChainConfig;
use zksync_crypto::Fr;
use zksync_storage::ConnectionPool;
use zksync_token_db_cache::TokenDBCache;
use zksync_types::{
    block::{Block, BlockMetadata, ExecutedOperations, IncompleteBlock, PendingBlock},
    AccountUpdates, BlockNumber,
};

mod aggregated_committer;

// In this component, the most interesting part of the database is decimals,
// Usually we don't change them, so we can invalidate the cache once an hour.
const TOKEN_INVALIDATE_CACHE: Duration = Duration::from_secs(60 * 60);

#[derive(Debug)]
pub enum CommitRequest {
    PendingBlock((PendingBlock, AppliedUpdatesRequest)),
    RemoveRevertedBlock(BlockNumber),
    SealIncompleteBlock((BlockCommitRequest, AppliedUpdatesRequest)),
    FinishBlock(BlockFinishRequest),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockCommitRequest {
    pub block: IncompleteBlock,
    pub block_metadata: BlockMetadata,
}

#[derive(Clone, Debug)]
pub struct BlockFinishRequest {
    pub block_number: BlockNumber,
    pub root_hash: Fr,
}

#[derive(Clone, Debug)]
pub struct AppliedUpdatesRequest {
    pub account_updates: AccountUpdates,
    pub first_update_order_id: usize,
}

pub struct ExecutedOpsNotify {
    pub operations: Vec<ExecutedOperations>,
    pub block_number: BlockNumber,
}

const PROOF_POLL_INTERVAL: Duration = Duration::from_secs(1);

async fn handle_new_commit_task(mut rx_for_ops: Receiver<CommitRequest>, pool: ConnectionPool) {
    vlog::info!("Run committer");
    let mut token_db_cache = TokenDBCache::new(TOKEN_INVALIDATE_CACHE);
    token_db_cache
        .fill_token_cache(&mut pool.access_storage().await.unwrap())
        .await;
    while let Some(request) = rx_for_ops.next().await {
        match request {
            CommitRequest::SealIncompleteBlock((block_commit_request, applied_updates_req)) => {
                seal_incomplete_block(
                    block_commit_request,
                    applied_updates_req,
                    &pool,
                    &mut token_db_cache,
                )
                .await;
            }
            CommitRequest::PendingBlock((pending_block, applied_updates_req)) => {
                save_pending_block(pending_block, applied_updates_req, &pool).await;
            }
            CommitRequest::FinishBlock(request) => {
                finish_block(request, &pool).await;
            }
            CommitRequest::RemoveRevertedBlock(block_number) => {
                remove_reverted_block(block_number, &pool).await;
            }
        }
    }
}

async fn remove_reverted_block(block_number: BlockNumber, pool: &ConnectionPool) {
    let start = Instant::now();
    let mut storage = pool
        .access_storage()
        .await
        .expect("db connection fail for committer");
    storage
        .chain()
        .mempool_schema()
        .remove_reverted_block(block_number)
        .await
        .expect("Failed to remove reverted blocks");
    metrics::histogram!("committer.remove_reverted_block", start.elapsed());
}

async fn save_pending_block(
    pending_block: PendingBlock,
    applied_updates_request: AppliedUpdatesRequest,
    pool: &ConnectionPool,
) {
    let start = Instant::now();
    let mut storage = pool
        .access_storage()
        .await
        .expect("db connection fail for committer");

    let mut transaction = storage
        .start_transaction()
        .await
        .expect("Failed initializing a DB transaction");

    let block_number = pending_block.number;

    vlog::trace!("persist pending block #{}", block_number);

    transaction
        .chain()
        .block_schema()
        .save_pending_block(pending_block)
        .await
        .expect("committer must commit the pending block into db");

    transaction
        .chain()
        .state_schema()
        .commit_state_update(
            block_number,
            &applied_updates_request.account_updates,
            applied_updates_request.first_update_order_id,
        )
        .await
        .expect("committer must commit the pending block into db");

    transaction
        .commit()
        .await
        .expect("Unable to commit DB transaction");

    metrics::histogram!("committer.save_pending_block", start.elapsed());
}

async fn seal_incomplete_block(
    block_commit_request: BlockCommitRequest,
    applied_updates_request: AppliedUpdatesRequest,
    pool: &ConnectionPool,
    token_db_cache: &mut TokenDBCache,
) {
    let start = Instant::now();
    let BlockCommitRequest {
        block,
        block_metadata,
    } = block_commit_request;

    let mut storage = pool
        .access_storage()
        .await
        .expect("db connection fail for committer");

    let mut transaction = storage
        .start_transaction()
        .await
        .expect("Failed initializing a DB transaction");

    // This is needed to keep track of how many priority ops are in each block
    // and trigger grafana alerts if there are suspiciously few
    let total_priority_ops = block
        .block_transactions
        .iter()
        .filter(|tx| matches!(tx, ExecutedOperations::PriorityOp(_)))
        .count();
    metrics::histogram!(
        "committer.priority_ops_per_block",
        total_priority_ops as f64
    );

    transaction
        .chain()
        .state_schema()
        .commit_state_update(
            block.block_number,
            &applied_updates_request.account_updates,
            applied_updates_request.first_update_order_id,
        )
        .await
        .expect("committer must commit the pending block into db");

    vlog::info!("seal incomplete block #{}", block.block_number);

    let block_number = block.block_number;

    transaction
        .chain()
        .block_schema()
        .save_incomplete_block(&block)
        .await
        .expect("committer must commit the op into db");

    transaction
        .chain()
        .block_schema()
        .save_block_metadata(block_number, block_metadata)
        .await
        .expect("committer must commit block block metadata into db");

    transaction
        .commit()
        .await
        .expect("Unable to commit DB transaction");

    // We do this outside of a transaction,
    // because we want the incomplete block data to be available as soon as possible.
    // If something happened to the metric count, it won't affect the block data
    if let Err(err) =
        zksync_prometheus_exporter::calculate_volume_for_block(&mut storage, &block, token_db_cache)
            .await
    {
        vlog::warn!("Can't calculate volume metric: {:?}", err)
    }
    metrics::histogram!("committer.seal_incomplete_block", start.elapsed());
}

async fn finish_block(request: BlockFinishRequest, pool: &ConnectionPool) {
    let start = Instant::now();
    let BlockFinishRequest {
        block_number,
        root_hash,
    } = request;

    let mut storage = pool
        .access_storage()
        .await
        .expect("db connection fail for committer");

    let mut transaction = storage
        .start_transaction()
        .await
        .expect("Failed initializing a DB transaction");

    vlog::info!("finish block #{}", block_number);

    let (incomplete_block, prev_root_hash) = transaction
        .chain()
        .block_schema()
        .get_data_to_complete_block(block_number)
        .await
        .expect("committer: unable to get incomplete block data from db");

    let incomplete_block = incomplete_block.unwrap_or_else(|| {
        panic!(
            "Received a request to finish block #{} which is not saved to the database",
            block_number
        );
    });
    let prev_root_hash = prev_root_hash.unwrap_or_else(|| {
        // Invariant: we calculate root hashes for blocks sequentially.
        // It means that if we want to finish block `X`, then root hash for block `X-1` is already calculated and saved to the database.
        // If there is no root hash data in the database, it means that there is a bug in application logic.
        panic!("Received a request to finish block #{}, but there is no root hash for previous block in the database", block_number);
    });

    let block = Block::from_incomplete(incomplete_block, prev_root_hash, root_hash);

    transaction
        .chain()
        .block_schema()
        .finish_incomplete_block(block)
        .await
        .expect("committer must commit the op into db");

    transaction
        .commit()
        .await
        .expect("Unable to commit DB transaction");

    metrics::histogram!("committer.finish_block", start.elapsed());
}

async fn poll_for_new_proofs_task(pool: ConnectionPool, config: ChainConfig) {
    let mut timer = time::interval(PROOF_POLL_INTERVAL);
    loop {
        timer.tick().await;

        let mut storage = pool
            .access_storage()
            .await
            .expect("db connection failed for committer");

        aggregated_committer::create_aggregated_operations_storage(&mut storage, &config)
            .await
            .map_err(|e| vlog::error!("Failed to create aggregated operation: {}", e))
            .unwrap_or_default();
    }
}

#[must_use]
pub fn run_committer(
    rx_for_ops: Receiver<CommitRequest>,
    pool: ConnectionPool,
    config: ChainConfig,
) -> JoinHandle<()> {
    tokio::spawn(handle_new_commit_task(rx_for_ops, pool.clone()));
    tokio::spawn(poll_for_new_proofs_task(pool, config))
}
