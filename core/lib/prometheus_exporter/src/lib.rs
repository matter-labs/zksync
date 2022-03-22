//! This module handles metric export to the Prometheus server

use metrics_exporter_prometheus::PrometheusBuilder;
use num::rational::Ratio;
use num::{BigUint, ToPrimitive};
use std::collections::HashMap;
use std::ops::Add;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use zksync_storage::{ConnectionPool, QueryResult, StorageProcessor};
use zksync_token_db_cache::TokenDBCache;
use zksync_types::aggregated_operations::AggregatedActionType::*;
use zksync_types::block::IncompleteBlock;
use zksync_types::TokenId;

const QUERY_INTERVAL: Duration = Duration::from_secs(30);

pub fn run_operation_counter(connection_pool: ConnectionPool) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(e) = prometheus_exporter_iteration(connection_pool.clone()).await {
                vlog::error!("Prometheus error: {}", e);
            }
            sleep(QUERY_INTERVAL).await;
        }
    })
}

async fn prometheus_exporter_iteration(connection_pool: ConnectionPool) -> QueryResult<()> {
    let mut storage = connection_pool.access_storage().await?;
    let mut transaction = storage.start_transaction().await?;

    let mut block_schema = transaction.chain().block_schema();

    for &action in &[CommitBlocks, ExecuteBlocks] {
        for &is_confirmed in &[false, true] {
            let result = block_schema
                .count_aggregated_operations(action, is_confirmed)
                .await?;
            metrics::gauge!(
                "count_operations",
                result as f64,
                "action" => action.to_string(),
                "confirmed" => is_confirmed.to_string()
            );
        }
    }

    let rejected_txs = block_schema.count_rejected_txs().await?;

    metrics::gauge!("stored_rejected_txs", rejected_txs as f64);

    let mempool_size = transaction
        .chain()
        .mempool_schema()
        .get_mempool_size()
        .await?;
    metrics::gauge!("mempool_size", mempool_size as f64);

    transaction.commit().await?;
    Ok(())
}

/// Extract volumes from block
fn get_volumes(block: &IncompleteBlock) -> HashMap<TokenId, BigUint> {
    let mut volumes: HashMap<TokenId, BigUint> = HashMap::new();

    // Iterator over tx amounts in the block.
    let amounts_iter = block
        .block_transactions
        .iter()
        .filter(|executed_op| executed_op.is_successful()) // Only process successful operations.
        .filter_map(|executed_op| executed_op.get_executed_op()) // Obtain transaction.
        .filter_map(|tx| tx.get_amount_info()) // Process transactions with amounts.
        .flatten(); // Each transaction can have multiple amounts, process one by one.

    for (token, amount) in amounts_iter {
        volumes
            .entry(token)
            .and_modify(|volume| *volume = volume.clone().add(amount.clone()))
            .or_insert(amount);
    }
    volumes
}

/// Send volume of all transactions in block in usd to prometheus
pub async fn calculate_volume_for_block(
    storage: &mut StorageProcessor<'_>,
    block: &IncompleteBlock,
    token_db_cache: &mut TokenDBCache,
) -> Result<(), anyhow::Error> {
    let start = Instant::now();
    let volumes = get_volumes(block);
    for (token_id, amount) in volumes.into_iter() {
        if let Some(price) = storage
            .tokens_schema()
            .get_historical_ticker_price(token_id)
            .await?
        {
            let token = token_db_cache.get_token(storage, token_id).await?.unwrap();
            let labels = vec![("token", token.symbol)];
            let usd_amount = Ratio::from(amount)
                / BigUint::from(10u32).pow(u32::from(token.decimals))
                * price.usd_price;
            metrics::increment_gauge!("txs_volume", usd_amount.to_f64().unwrap(), &labels);
        }
    }
    metrics::histogram!("calculate_metric",  start.elapsed(), "type" => "volume_for_block");
    Ok(())
}

pub fn run_prometheus_exporter(port: u16) -> JoinHandle<()> {
    let addr = ([0, 0, 0, 0], port);
    let (recorder, exporter) = PrometheusBuilder::new()
        .listen_address(addr)
        .build_with_exporter()
        .expect("failed to install Prometheus recorder");
    metrics::set_boxed_recorder(Box::new(recorder)).expect("failed to set metrics recorder");

    tokio::spawn(async move {
        tokio::pin!(exporter);
        loop {
            tokio::select! {
                _ = &mut exporter => {}
            }
        }
    })
}
