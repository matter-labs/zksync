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
use zksync_types::{ExecutedOperations, TokenId};

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
fn get_volumes(txs: &[ExecutedOperations]) -> HashMap<TokenId, BigUint> {
    let mut volumes: HashMap<TokenId, BigUint> = HashMap::new();

    // Iterator over tx amounts in the block.
    let amounts_iter = txs
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
    let volumes = get_volumes(&block.block_transactions);
    for (token_id, amount) in volumes.into_iter() {
        if let Some(price) = storage
            .tokens_schema()
            .get_historical_ticker_price(token_id)
            .await?
        {
            let token = token_db_cache.get_token(storage, token_id).await?.unwrap();
            let usd_amount = token_amount_to_usd(amount, token.decimals, price.usd_price);
            let labels = vec![("token", token.symbol)];
            metrics::increment_gauge!("txs_volumes", usd_amount.to_f64().unwrap(), &labels);
        }
    }
    metrics::histogram!("calculate_metric",  start.elapsed(), "type" => "volume_for_block");
    Ok(())
}

fn token_amount_to_usd(amount: BigUint, decimals: u8, usd_price: Ratio<BigUint>) -> Ratio<BigUint> {
    Ratio::from(amount) / BigUint::from(10u32).pow(u32::from(decimals)) * usd_price
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

#[cfg(test)]
mod tests {
    use crate::{get_volumes, token_amount_to_usd, BigUint, ToPrimitive, TokenId};
    use chrono::Utc;
    use num::FromPrimitive;
    use zksync_crypto::{
        priv_key_from_fs,
        rand::{thread_rng, Rng},
    };
    use zksync_storage::BigDecimal;
    use zksync_types::{
        AccountId, Address, ExecutedOperations, ExecutedTx, Nonce, Order, SignedZkSyncTx, Swap,
        SwapOp, Transfer, TransferOp, ZkSyncOp, ZkSyncTx,
    };
    use zksync_utils::big_decimal_to_ratio;

    #[test]
    fn calculate_volume() {
        let usdc_price = big_decimal_to_ratio(&BigDecimal::from_f64(1.032).unwrap()).unwrap();
        let usdc_amount = BigUint::from(65169500u32);
        let usdc_decimals = 6;
        let volume = token_amount_to_usd(usdc_amount, usdc_decimals, usdc_price);
        assert!((67.254924 - volume.to_f64().unwrap()).abs() <= f64::EPSILON);
        let eth_price = big_decimal_to_ratio(&BigDecimal::from_f64(3424.05).unwrap()).unwrap();
        let eth_amount = BigUint::from(87829590000000000u64);
        let eth_decimals = 18;
        let volume = token_amount_to_usd(eth_amount, eth_decimals, eth_price);
        assert!((300.7329076395 - volume.to_f64().unwrap()).abs() <= f64::EPSILON);
    }

    fn create_transfer(amount: u64, token: TokenId, success: bool) -> ExecutedOperations {
        let correct_transfer = Transfer::new(
            AccountId(0),
            Default::default(),
            Default::default(),
            token,
            BigUint::from(amount),
            BigUint::from(10u64),
            Nonce(0),
            Default::default(),
            None,
        );
        let transfer_op = TransferOp {
            tx: correct_transfer.clone(),
            from: Default::default(),
            to: Default::default(),
        };

        ExecutedOperations::Tx(Box::new(ExecutedTx {
            signed_tx: SignedZkSyncTx::from(ZkSyncTx::Transfer(Box::new(correct_transfer))),
            success,
            op: Some(ZkSyncOp::Transfer(Box::new(transfer_op))),
            fail_reason: None,
            block_index: None,
            created_at: Utc::now(),
            batch_id: None,
        }))
    }

    fn create_swap(
        amount: u64,
        token_0: TokenId,
        token_1: TokenId,
        success: bool,
    ) -> ExecutedOperations {
        let rng = &mut thread_rng();
        let sk = priv_key_from_fs(rng.gen());
        let swap = Swap::new(
            AccountId(0),
            Default::default(),
            Nonce(0),
            (
                Order::new_signed(
                    AccountId(1),
                    Address::random(),
                    Nonce(0),
                    token_1,
                    token_0,
                    (BigUint::from(1u64), BigUint::from(1u64)),
                    BigUint::from(amount),
                    Default::default(),
                    &sk,
                )
                .unwrap(),
                Order::new_signed(
                    AccountId(1),
                    Address::random(),
                    Nonce(0),
                    token_0,
                    token_1,
                    (BigUint::from(1u64), BigUint::from(1u64)),
                    BigUint::from(amount),
                    Default::default(),
                    &sk,
                )
                .unwrap(),
            ),
            (BigUint::from(amount), BigUint::from(amount)),
            BigUint::from(10u64),
            TokenId(0),
            None,
        );
        let swap_op = SwapOp {
            tx: swap.clone(),
            submitter: Default::default(),
            accounts: (Default::default(), Default::default()),
            recipients: (Default::default(), Default::default()),
        };

        ExecutedOperations::Tx(Box::new(ExecutedTx {
            signed_tx: SignedZkSyncTx::from(ZkSyncTx::Swap(Box::new(swap))),
            success,
            op: Some(ZkSyncOp::Swap(Box::new(swap_op))),
            fail_reason: None,
            block_index: None,
            created_at: Utc::now(),
            batch_id: None,
        }))
    }

    #[test]
    fn test_get_volumes() {
        let txs = vec![
            create_transfer(100, TokenId(0), true),
            create_transfer(200, TokenId(0), false),
            create_transfer(10, TokenId(0), true),
            create_transfer(10, TokenId(1), true),
            create_transfer(10, TokenId(33), true),
            create_swap(10, TokenId(0), TokenId(33), true),
        ];
        let res = get_volumes(&txs);
        let volume = res.get(&TokenId(0)).unwrap().clone();
        assert_eq!(volume, BigUint::from(120u64));
        let volume = res.get(&TokenId(1)).unwrap().clone();
        assert_eq!(volume, BigUint::from(10u64));
        let volume = res.get(&TokenId(33)).unwrap().clone();
        assert_eq!(volume, BigUint::from(20u64));
    }
}
