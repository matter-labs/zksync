//! Block part of API implementation.

// Built-in uses
use std::str::FromStr;

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_storage::{chain::block::records::BlockDetails, ConnectionPool, QueryResult};
use zksync_types::{
    pagination::{BlockAndTxHash, Paginated, PaginationQuery},
    tx::TxHash,
    BlockNumber,
};
// Local uses
use super::{
    error::Error,
    paginate::Paginate,
    response::ApiResult,
    types::{BlockInfo, Transaction},
};
use crate::utils::block_details_cache::BlockDetailsCache;

/// Shared data between `api/v0.2/block` endpoints.
#[derive(Debug, Clone)]
struct ApiBlockData {
    pool: ConnectionPool,
    /// Verified blocks cache.
    cache: BlockDetailsCache,
}

impl ApiBlockData {
    fn new(pool: ConnectionPool, cache: BlockDetailsCache) -> Self {
        Self { pool, cache }
    }

    /// Returns information about block with the specified number.
    ///
    /// This method caches some of the verified blocks.
    async fn block_info(&self, block_number: BlockNumber) -> QueryResult<Option<BlockDetails>> {
        self.cache.get(&self.pool, block_number).await
    }

    async fn block_page(
        &self,
        query: PaginationQuery<BlockNumber>,
    ) -> Result<Paginated<BlockInfo, BlockNumber>, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::internal)?;
        storage.paginate(query).await
    }

    async fn transaction_page(
        &self,
        block_number: BlockNumber,
        query: PaginationQuery<TxHash>,
    ) -> Result<Paginated<Transaction, BlockAndTxHash>, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::internal)?;

        let new_query = PaginationQuery {
            from: BlockAndTxHash {
                block_number,
                tx_hash: query.from,
            },
            limit: query.limit,
            direction: query.direction,
        };

        storage.paginate(new_query).await
    }

    async fn get_last_committed_block_number(&self) -> QueryResult<BlockNumber> {
        let mut storage = self.pool.access_storage().await?;
        storage
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await
    }

    async fn get_last_finalized_block_number(&self) -> QueryResult<BlockNumber> {
        let mut storage = self.pool.access_storage().await?;
        storage
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await
    }
}

// Server implementation

async fn block_pagination(
    data: web::Data<ApiBlockData>,
    web::Query(query): web::Query<PaginationQuery<BlockNumber>>,
) -> ApiResult<Paginated<BlockInfo, BlockNumber>> {
    data.block_page(query).await.map_err(Error::from).into()
}

async fn block_by_number(
    data: web::Data<ApiBlockData>,
    web::Path(block_position): web::Path<String>,
) -> ApiResult<Option<BlockInfo>> {
    // TODO: take block_position as enum
    let block_number: BlockNumber;
    if let Ok(number) = u32::from_str(&block_position) {
        block_number = BlockNumber(number);
    } else {
        match block_position.as_str() {
            "last_committed" => match data.get_last_committed_block_number().await {
                Ok(number) => {
                    block_number = number;
                }
                Err(err) => {
                    return Error::internal(err).into();
                }
            },
            "last_finalized" => match data.get_last_finalized_block_number().await {
                Ok(number) => {
                    block_number = number;
                }
                Err(err) => {
                    return Error::internal(err).into();
                }
            },
            _ => {
                return Error::invalid_data(
                    "There are only {block_number}, last_committed, last_finalized options",
                )
                .into();
            }
        }
    };
    data.block_info(block_number)
        .await
        .map_err(Error::internal)
        .map(|details| details.map(BlockInfo::from))
        .into()
}

async fn block_transactions(
    data: web::Data<ApiBlockData>,
    web::Path(block_position): web::Path<String>,
    web::Query(query): web::Query<PaginationQuery<TxHash>>,
) -> ApiResult<Paginated<Transaction, BlockAndTxHash>> {
    // TODO: take block_position as enum
    let block_number: BlockNumber;
    if let Ok(number) = u32::from_str(&block_position) {
        block_number = BlockNumber(number);
    } else {
        match block_position.as_str() {
            "last_committed" => match data.get_last_committed_block_number().await {
                Ok(number) => {
                    block_number = number;
                }
                Err(err) => {
                    return Error::internal(err).into();
                }
            },
            "last_finalized" => match data.get_last_finalized_block_number().await {
                Ok(number) => {
                    block_number = number;
                }
                Err(err) => {
                    return Error::internal(err).into();
                }
            },
            _ => {
                return Error::invalid_data(
                    "There are only {block_number}, last_committed, last_finalized options",
                )
                .into();
            }
        }
    };

    data.transaction_page(block_number, query)
        .await
        .map_err(Error::from)
        .into()
}

pub fn api_scope(pool: ConnectionPool, cache: BlockDetailsCache) -> Scope {
    let data = ApiBlockData::new(pool, cache);

    web::scope("block")
        .data(data)
        .route("", web::get().to(block_pagination))
        .route("{block_number}", web::get().to(block_by_number))
        .route(
            "{block_number}/transaction",
            web::get().to(block_transactions),
        )
}
