//! Account part of API implementation.

// Built-in uses
use std::str::FromStr;

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_api_types::v02::{
    pagination::{AccountTxsRequest, Paginated, PaginationQuery, PendingOpsRequest},
    transaction::Transaction,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;
use zksync_types::{tx::TxHash, AccountId, Address, BlockNumber, SerialId};

// Local uses
use super::{
    error::{Error, InvalidDataError},
    paginate_trait::Paginate,
    response::ApiResult,
};
use crate::{api_try, core_api_client::CoreApiClient, utils::token_db_cache::TokenDBCache};

/// Shared data between `api/v1/accounts` endpoints.
#[derive(Clone)]
struct ApiAccountData {
    pool: ConnectionPool,
    tokens: TokenDBCache,
    core_api_client: CoreApiClient,
    confirmations_for_eth_event: BlockNumber,
}

impl ApiAccountData {
    fn new(
        pool: ConnectionPool,
        tokens: TokenDBCache,
        core_api_client: CoreApiClient,
        confirmations_for_eth_event: BlockNumber,
    ) -> Self {
        Self {
            pool,
            tokens,
            core_api_client,
            confirmations_for_eth_event,
        }
    }

    async fn parse_account_id_or_address(
        &self,
        account_id_or_address: &str,
    ) -> Result<(Address, AccountId), Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        if let Ok(account_id) = u32::from_str(account_id_or_address) {
            let account_id = AccountId(account_id);
            let address = storage
                .chain()
                .account_schema()
                .account_address_by_id(account_id)
                .await
                .map_err(Error::storage)?;
            if let Some(address) = address {
                Ok((address, account_id))
            } else {
                Err(Error::from(InvalidDataError::AccountNotFound))
            }
        } else {
            let address_str = if let Some(address_str) = account_id_or_address.strip_prefix("0x") {
                address_str
            } else {
                account_id_or_address
            };

            if let Ok(address) = Address::from_str(address_str) {
                let account_id = storage
                    .chain()
                    .account_schema()
                    .account_id_by_address(address)
                    .await
                    .map_err(Error::storage)?;
                if let Some(account_id) = account_id {
                    Ok((address, account_id))
                } else {
                    Err(Error::from(InvalidDataError::AccountNotFound))
                }
            } else {
                Err(Error::from(InvalidDataError::InvalidAccountIdOrAddress))
            }
        }
    }

    async fn account_txs(
        &self,
        query: PaginationQuery<TxHash>,
        address: Address,
        account_id: AccountId,
    ) -> Result<Paginated<Transaction, AccountTxsRequest>, Error> {
        let new_query = PaginationQuery {
            from: AccountTxsRequest {
                address,
                account_id,
                tx_hash: query.from,
            },
            limit: query.limit,
            direction: query.direction,
        };
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        storage.paginate(&new_query).await
    }

    async fn account_pending_txs(
        &self,
        query: PaginationQuery<SerialId>,
        address: Address,
        account_id: AccountId,
    ) -> Result<Paginated<Transaction, PendingOpsRequest>, Error> {
        let new_query = PaginationQuery {
            from: PendingOpsRequest {
                address,
                account_id,
                serial_id: query.from,
            },
            limit: query.limit,
            direction: query.direction,
        };
        let mut client = self.core_api_client.clone();
        client.paginate(&new_query).await
    }
}

async fn account_txs(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
    web::Query(query): web::Query<PaginationQuery<TxHash>>,
) -> ApiResult<Paginated<Transaction, AccountTxsRequest>> {
    let (address, account_id) = api_try!(
        data.parse_account_id_or_address(&account_id_or_address)
            .await
    );
    data.account_txs(query, address, account_id).await.into()
}

async fn account_pending_txs(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
    web::Query(query): web::Query<PaginationQuery<SerialId>>,
) -> ApiResult<Paginated<Transaction, PendingOpsRequest>> {
    let (address, account_id) = api_try!(
        data.parse_account_id_or_address(&account_id_or_address)
            .await
    );
    data.account_pending_txs(query, address, account_id)
        .await
        .into()
}

pub fn api_scope(
    pool: ConnectionPool,
    config: &ZkSyncConfig,
    tokens: TokenDBCache,
    core_api_client: CoreApiClient,
) -> Scope {
    let data = ApiAccountData::new(
        pool,
        tokens,
        core_api_client,
        BlockNumber(config.eth_watch.confirmations_for_eth_event as u32),
    );

    web::scope("account")
        .data(data)
        // .route(
        //     "{account_id_or_address}/{block}",
        //     web::get().to(account_info),
        // )
        .route(
            "{account_id_or_address}/transactions",
            web::get().to(account_txs),
        )
        .route(
            "{account_id_or_address}/transactions/pending",
            web::get().to(account_pending_txs),
        )
}
