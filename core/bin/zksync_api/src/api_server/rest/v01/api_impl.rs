//! Implementation of REST API v0.1 endpoints.
//!
//! Since all the methods declared in this file are valid `actix-web` handlers,
//! they take no `self` argument, but instead expect it to be set as `data` in the
//! scope configuration. This is done by the `ApiV01::into_scope` method.

use crate::api_server::{
    helpers::try_parse_hash,
    rest::{
        helpers::{deposit_op_to_tx_by_hash, parse_tx_id, priority_op_to_tx_history},
        v01::{api_decl::ApiV01, types::*},
    },
};
use actix_web::{web, HttpResponse, Result as ActixResult};
use num::{rational::Ratio, BigUint, FromPrimitive};
use std::time::Instant;
use zksync_storage::chain::operations_ext::SearchDirection;
use zksync_types::{Address, BlockNumber, Token, TokenId};

/// Helper macro which wraps the serializable object into `Ok(HttpResponse::Ok().json(...))`.
macro_rules! ok_json {
    ($resp:expr) => {
        Ok(HttpResponse::Ok().json($resp))
    };
}

impl ApiV01 {
    pub async fn testnet_config(self_: web::Data<Self>) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let contract_address = self_.contract_address.clone();
        metrics::histogram!("api.v01.testnet_config", start.elapsed());
        ok_json!(TestnetConfigResponse { contract_address })
    }

    pub async fn status(self_: web::Data<Self>) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let result = ok_json!(self_.network_status.read().await);
        metrics::histogram!("api.v01.status", start.elapsed());
        result
    }

    pub async fn tokens(self_: web::Data<Self>) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let mut storage = self_.access_storage().await?;
        let tokens = storage
            .tokens_schema()
            .load_tokens()
            .await
            .map_err(Self::db_error)?;

        let mut vec_tokens = tokens.values().cloned().collect::<Vec<_>>();
        vec_tokens.sort_by_key(|t| t.id);

        metrics::histogram!("api.v01.tokens", start.elapsed());
        ok_json!(vec_tokens)
    }

    pub async fn tokens_acceptable_for_fees(self_: web::Data<Self>) -> ActixResult<HttpResponse> {
        let start = Instant::now();

        let liquidity_volume = Ratio::from(
            BigUint::from_f64(self_.config.ticker.liquidity_volume)
                .expect("TickerConfig::liquidity_volume must be positive"),
        );

        let mut storage = self_.access_storage().await?;
        let mut tokens = storage
            .tokens_schema()
            .load_tokens_by_market_volume(liquidity_volume)
            .await
            .map_err(Self::db_error)?;

        // Add ETH for tokens allowed for fee
        // Different APIs have different views on how to represent ETH in their system.
        // But ETH is always allowed to pay fee, and in all cases it should be on the list.

        if tokens.get(&TokenId(0)).is_none() {
            let eth = Token::new(TokenId(0), Default::default(), "ETH", 18);
            tokens.insert(eth.id, eth);
        }

        let mut tokens = tokens.values().cloned().collect::<Vec<_>>();

        tokens.sort_by_key(|t| t.id);

        metrics::histogram!("api.v01.tokens_acceptable_for_fees", start.elapsed());
        ok_json!(tokens)
    }

    pub async fn tx_history(
        self_: web::Data<Self>,
        web::Path((address, mut offset, mut limit)): web::Path<(Address, u64, u64)>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        const MAX_LIMIT: u64 = 100;
        if limit > MAX_LIMIT {
            return Err(HttpResponse::BadRequest().finish().into());
        }

        let tokens = self_
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

        // Fetch ongoing deposits, since they must be reported within the transactions history.
        let mut ongoing_ops = self_
            .api_client
            .get_unconfirmed_deposits(address)
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
        ongoing_ops.sort_by(|lhs, rhs| rhs.eth_block.cmp(&lhs.eth_block));

        // Collect the unconfirmed priority operations with respect to the
        // `offset` and `limit` parameters.
        let mut ongoing_transactions_history: Vec<_> = ongoing_ops
            .iter()
            .map(|op| priority_op_to_tx_history(&tokens, op.eth_block, op))
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

        let mut transactions_history = self_
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

        metrics::histogram!("api.v01.tx_history", start.elapsed());
        ok_json!(transactions_history)
    }

    pub async fn tx_history_older_than(
        self_: web::Data<Self>,
        web::Path(address): web::Path<Address>,
        web::Query(query): web::Query<TxHistoryQuery>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let tx_id = query.tx_id.as_ref().map(|s| s.as_ref()).unwrap_or("-");
        let limit = query.limit.unwrap_or(MAX_LIMIT);

        const MAX_LIMIT: u64 = 100;
        if limit > MAX_LIMIT {
            return Err(HttpResponse::BadRequest().finish().into());
        }
        let mut storage = self_.access_storage().await?;
        let mut transaction = storage.start_transaction().await.map_err(Self::db_error)?;

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

        transaction.commit().await.map_err(Self::db_error)?;

        metrics::histogram!("api.v01.tx_history_older_than", start.elapsed());
        ok_json!(transactions_history)
    }

    pub async fn tx_history_newer_than(
        self_: web::Data<Self>,
        web::Path(address): web::Path<Address>,
        web::Query(query): web::Query<TxHistoryQuery>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let tx_id = query.tx_id.as_ref().map(|s| s.as_ref()).unwrap_or("-");
        let mut limit = query.limit.unwrap_or(MAX_LIMIT);

        const MAX_LIMIT: u64 = 100;
        if limit > MAX_LIMIT {
            return Err(HttpResponse::BadRequest().finish().into());
        }

        let direction = SearchDirection::Newer;
        let mut transactions_history = {
            let mut storage = self_.access_storage().await?;
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

            // Fetch ongoing deposits, since they must be reported within the transactions history.
            let mut ongoing_ops = self_
                .api_client
                .get_unconfirmed_deposits(address)
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
            ongoing_ops.sort_by(|lhs, rhs| rhs.eth_block.cmp(&lhs.eth_block));

            let tokens = self_
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
                .map(|op| priority_op_to_tx_history(&tokens, op.eth_block, op))
                .take(limit as usize)
                .collect();

            // Merge `txs` and `transactions_history` and reassign the `transactions_history` to the
            // merged list.
            // Unprocessed operations must be in the end (as the newest ones).
            transactions_history.append(&mut txs);
        }

        metrics::histogram!("api.v01.tx_history_newer_than", start.elapsed());
        ok_json!(transactions_history)
    }

    pub async fn executed_tx_by_hash(
        self_: web::Data<Self>,
        web::Path(tx_hash_hex): web::Path<String>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        if tx_hash_hex.len() < 2 {
            return Err(HttpResponse::BadRequest().finish().into());
        }
        let transaction_hash =
            hex::decode(&tx_hash_hex[2..]).map_err(|_| HttpResponse::BadRequest().finish())?;

        let tx_receipt = self_.get_tx_receipt(transaction_hash).await?;

        metrics::histogram!("api.v01.executed_tx_by_hash", start.elapsed());
        ok_json!(tx_receipt)
    }

    pub async fn tx_by_hash(
        self_: web::Data<Self>,
        web::Path(hash_hex_with_prefix): web::Path<String>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let hash = try_parse_hash(&hash_hex_with_prefix)
            .map_err(|_| HttpResponse::BadRequest().finish())?;

        let mut res = self_
            .access_storage()
            .await?
            .chain()
            .operations_ext_schema()
            .get_tx_by_hash(hash.as_ref())
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
            return ok_json!(res);
        }

        // Or try to find this priority op in eth_watcher
        let unconfirmed_op = self_
            .get_unconfirmed_op_by_hash(hash)
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
            let tokens = self_
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

        metrics::histogram!("api.v01.tx_by_hash", start.elapsed());
        ok_json!(res)
    }

    pub async fn priority_op(
        self_: web::Data<Self>,
        web::Path(pq_id): web::Path<u32>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let receipt = self_.get_priority_op_receipt(pq_id).await?;
        metrics::histogram!("api.v01.priority_op", start.elapsed());
        ok_json!(receipt)
    }

    pub async fn block_tx(
        self_: web::Data<Self>,
        web::Path((block_id, tx_id)): web::Path<(BlockNumber, u32)>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let exec_ops = self_.get_block_executed_ops(block_id).await?;

        let result = if let Some(exec_op) = exec_ops.get(tx_id as usize) {
            ok_json!(exec_op.clone())
        } else {
            Err(HttpResponse::NotFound().finish().into())
        };

        metrics::histogram!("api.v01.block_tx", start.elapsed());
        result
    }

    // pub async fn block_transactions(self_: web::Data<Self>, block_id: BlockNumber) -> !;
    pub async fn blocks(
        self_: web::Data<Self>,
        web::Query(block_query): web::Query<HandleBlocksQuery>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let max_block = block_query.max_block.unwrap_or(999_999_999);
        let limit = block_query.limit.unwrap_or(20);
        if limit > 100 {
            return Err(HttpResponse::BadRequest().finish().into());
        }
        let mut storage = self_.access_storage().await?;

        let resp = storage
            .chain()
            .block_schema()
            .load_block_range(BlockNumber(max_block), limit)
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

        metrics::histogram!("api.v01.blocks", start.elapsed());
        ok_json!(resp)
    }

    pub async fn block_by_id(
        self_: web::Data<Self>,
        web::Path(block_id): web::Path<BlockNumber>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let block = self_.get_block_info(block_id).await?;
        let result = if let Some(block) = block {
            ok_json!(block)
        } else {
            Err(HttpResponse::NotFound().finish().into())
        };
        metrics::histogram!("api.v01.block_by_id", start.elapsed());
        result
    }

    pub async fn block_transactions(
        self_: web::Data<Self>,
        web::Path(block_id): web::Path<BlockNumber>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let mut storage = self_.access_storage().await?;

        let txs = storage
            .chain()
            .block_schema()
            .get_block_transactions(block_id)
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: {}", err, *block_id);
                HttpResponse::InternalServerError().finish()
            })?;

        metrics::histogram!("api.v01.block_transactions", start.elapsed());
        ok_json!(txs)
    }

    pub async fn explorer_search(
        self_: web::Data<Self>,
        web::Query(block_query): web::Query<BlockExplorerSearchQuery>,
    ) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let block = self_.get_block_by_height_or_hash(block_query.query).await?;

        let result = if let Some(block) = block {
            ok_json!(block)
        } else {
            Err(HttpResponse::NotFound().finish().into())
        };

        metrics::histogram!("api.v01.explorer_search", start.elapsed());
        result
    }

    pub async fn withdrawal_processing_time(self_: web::Data<Self>) -> ActixResult<HttpResponse> {
        let start = Instant::now();
        let state_keeper_config = &self_.config.chain.state_keeper;
        let processing_time = WithdrawalProcessingTimeResponse {
            normal: (state_keeper_config.miniblock_iteration_interval()
                * state_keeper_config.miniblock_iterations as u32)
                .as_secs(),
            fast: (state_keeper_config.miniblock_iteration_interval()
                * state_keeper_config.fast_block_miniblock_iterations as u32)
                .as_secs(),
        };

        metrics::histogram!("api.v01.withdrawal_processing_time", start.elapsed());
        ok_json!(processing_time)
    }
}
