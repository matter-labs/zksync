//! Implementation of REST API v0.1 endpoints.
//!
//! Since all the methods declared in this file are valid `actix-web` handlers,
//! they take no `self` argument, but instead expect it to be set as `data` in the
//! scope configuration. This is done by the `ApiV01::into_scope` method.

use crate::api_server::{
    rest::{
        helpers::*,
        v01::{api_decl::ApiV01, types::*},
    },
    rpc_server::get_ongoing_priority_ops,
};
use actix_web::{web, HttpResponse, Result as ActixResult};
use zksync_storage::chain::operations_ext::SearchDirection;
use zksync_types::{Address, BlockNumber};

/// Helper macro which wraps the serializable object into `Ok(HttpResponse::Ok().json(...))`.
macro_rules! ok_json {
    ($resp:expr) => {
        Ok(HttpResponse::Ok().json($resp))
    };
}

impl ApiV01 {
    pub async fn testnet_config(self_: web::Data<Self>) -> ActixResult<HttpResponse> {
        let contract_address = self_.contract_address.clone();
        ok_json!(TestnetConfigResponse { contract_address })
    }

    pub async fn status(self_: web::Data<Self>) -> ActixResult<HttpResponse> {
        ok_json!(self_.network_status.read().await)
    }

    pub async fn tokens(self_: web::Data<Self>) -> ActixResult<HttpResponse> {
        let mut storage = self_.access_storage().await?;
        let tokens = storage
            .tokens_schema()
            .load_tokens()
            .await
            .map_err(Self::db_error)?;

        let mut vec_tokens = tokens.values().cloned().collect::<Vec<_>>();
        vec_tokens.sort_by_key(|t| t.id);

        ok_json!(vec_tokens)
    }

    pub async fn tx_history(
        self_: web::Data<Self>,
        web::Path((address, mut offset, mut limit)): web::Path<(Address, u64, u64)>,
    ) -> ActixResult<HttpResponse> {
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
        let mut ongoing_ops = get_ongoing_priority_ops(&self_.api_client, address)
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

        ok_json!(transactions_history)
    }

    pub async fn tx_history_older_than(
        self_: web::Data<Self>,
        web::Path(address): web::Path<Address>,
        web::Query(query): web::Query<TxHistoryQuery>,
    ) -> ActixResult<HttpResponse> {
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

        ok_json!(transactions_history)
    }

    pub async fn tx_history_newer_than(
        self_: web::Data<Self>,
        web::Path(address): web::Path<Address>,
        web::Query(query): web::Query<TxHistoryQuery>,
    ) -> ActixResult<HttpResponse> {
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
            let mut ongoing_ops = get_ongoing_priority_ops(&self_.api_client, address)
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
                .map(|(block, op)| priority_op_to_tx_history(&tokens, *block, op))
                .take(limit as usize)
                .collect();

            // Merge `txs` and `transactions_history` and reassign the `transactions_history` to the
            // merged list.
            // Unprocessed operations must be in the end (as the newest ones).
            transactions_history.append(&mut txs);
        }

        ok_json!(transactions_history)
    }

    pub async fn executed_tx_by_hash(
        self_: web::Data<Self>,
        web::Path(tx_hash_hex): web::Path<String>,
    ) -> ActixResult<HttpResponse> {
        if tx_hash_hex.len() < 2 {
            return Err(HttpResponse::BadRequest().finish().into());
        }
        let transaction_hash =
            hex::decode(&tx_hash_hex[2..]).map_err(|_| HttpResponse::BadRequest().finish())?;

        let tx_receipt = self_.get_tx_receipt(transaction_hash).await?;

        ok_json!(tx_receipt)
    }

    pub async fn tx_by_hash(
        self_: web::Data<Self>,
        web::Path(hash_hex_with_prefix): web::Path<String>,
    ) -> ActixResult<HttpResponse> {
        let hash = try_parse_hash(&hash_hex_with_prefix)
            .ok_or_else(|| HttpResponse::BadRequest().finish())?;

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

        ok_json!(res)
    }

    pub async fn priority_op(
        self_: web::Data<Self>,
        web::Path(pq_id): web::Path<u32>,
    ) -> ActixResult<HttpResponse> {
        let receipt = self_.get_priority_op_receipt(pq_id).await?;
        ok_json!(receipt)
    }

    pub async fn block_tx(
        self_: web::Data<Self>,
        web::Path((block_id, tx_id)): web::Path<(BlockNumber, u32)>,
    ) -> ActixResult<HttpResponse> {
        let exec_ops = self_.get_block_executed_ops(block_id).await?;

        if let Some(exec_op) = exec_ops.get(tx_id as usize) {
            ok_json!(exec_op.clone())
        } else {
            Err(HttpResponse::NotFound().finish().into())
        }
    }

    // pub async fn block_transactions(self_: web::Data<Self>, block_id: BlockNumber) -> !;
    pub async fn blocks(
        self_: web::Data<Self>,
        web::Query(block_query): web::Query<HandleBlocksQuery>,
    ) -> ActixResult<HttpResponse> {
        let max_block = block_query.max_block.unwrap_or(999_999_999);
        let limit = block_query.limit.unwrap_or(20);
        if limit > 100 {
            return Err(HttpResponse::BadRequest().finish().into());
        }
        let mut storage = self_.access_storage().await?;

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
        ok_json!(resp)
    }

    pub async fn block_by_id(
        self_: web::Data<Self>,
        web::Path(block_id): web::Path<BlockNumber>,
    ) -> ActixResult<HttpResponse> {
        let block = self_.get_block_info(block_id).await?;
        if let Some(block) = block {
            ok_json!(block)
        } else {
            Err(HttpResponse::NotFound().finish().into())
        }
    }

    pub async fn block_transactions(
        self_: web::Data<Self>,
        web::Path(block_id): web::Path<BlockNumber>,
    ) -> ActixResult<HttpResponse> {
        let mut storage = self_.access_storage().await?;

        let txs = storage
            .chain()
            .block_schema()
            .get_block_transactions(block_id)
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: {}", err, block_id);
                HttpResponse::InternalServerError().finish()
            })?;

        ok_json!(txs)
    }

    pub async fn explorer_search(
        self_: web::Data<Self>,
        web::Query(block_query): web::Query<BlockExplorerSearchQuery>,
    ) -> ActixResult<HttpResponse> {
        let block = self_.get_block_by_height_or_hash(block_query.query).await?;

        if let Some(block) = block {
            ok_json!(block)
        } else {
            Err(HttpResponse::NotFound().finish().into())
        }
    }

    pub async fn withdrawal_processing_time(self_: web::Data<Self>) -> ActixResult<HttpResponse> {
        let miniblock_timings = &self_.config_options.miniblock_timings;
        let processing_time = WithdrawalProcessingTimeResponse {
            normal: (miniblock_timings.miniblock_iteration_interval
                * miniblock_timings.max_miniblock_iterations as u32)
                .as_secs(),
            fast: (miniblock_timings.miniblock_iteration_interval
                * miniblock_timings.fast_miniblock_iterations as u32)
                .as_secs(),
        };

        ok_json!(processing_time)
    }
}
