//! Search part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_crypto::{convert::FeConvert, Fr};
use zksync_storage::{ConnectionPool, QueryResult};
use zksync_types::{tx::TxHash, BlockNumber};

// Local uses
use super::{
    blocks::BlockInfo,
    client::{self, Client},
    Error as ApiError, JsonResult,
};

/// Shared data between `api/v1/search` endpoints.
#[derive(Clone)]
struct ApiSearchData {
    pool: ConnectionPool,
}

impl ApiSearchData {
    fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }

    async fn search_block(&self, query: String) -> QueryResult<Option<BlockInfo>> {
        let mut storage = self.pool.access_storage().await?;

        let block = storage
            .chain()
            .block_schema()
            .find_block_by_height_or_hash(query)
            .await;

        Ok(block.map(BlockInfo::from))
    }
}

// Data transfer objects.

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct BlockSearchQuery {
    query: String,
}

// Client implementation

impl From<BlockNumber> for BlockSearchQuery {
    /// Convert the block number into the search query.
    fn from(inner: BlockNumber) -> Self {
        Self {
            query: inner.to_string(),
        }
    }
}

impl From<Fr> for BlockSearchQuery {
    /// Converts the state root hash of the block into the search query.
    fn from(inner: Fr) -> Self {
        Self {
            query: inner.to_hex(),
        }
    }
}

impl From<TxHash> for BlockSearchQuery {
    /// Converts the commit/verify Ethereum transaction hash into the search query.
    fn from(inner: TxHash) -> Self {
        Self {
            // Serialize without prefix.
            query: hex::encode(inner),
        }
    }
}

/// Search API part.
impl Client {
    /// Performs a block search with an uncertain query, which can be either of:
    ///
    /// - Hash of commit/verify Ethereum transaction for the block.
    /// - The state root hash of the block.
    /// - The number of the block.
    pub async fn search_block(
        &self,
        query: impl Into<BlockSearchQuery>,
    ) -> client::Result<Option<BlockInfo>> {
        self.get("search").query(&query.into()).send().await
    }
}

// Server implementation

async fn block_search(
    data: web::Data<ApiSearchData>,
    web::Query(query): web::Query<BlockSearchQuery>,
) -> JsonResult<Option<BlockInfo>> {
    let block_info = data
        .search_block(query.query)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(block_info))
}

pub fn api_scope(pool: ConnectionPool) -> Scope {
    let data = ApiSearchData::new(pool);

    web::scope("search")
        .data(data)
        .route("", web::get().to(block_search))
}

#[cfg(test)]
mod tests {
    use super::{super::test_utils::TestServerConfig, *};

    #[actix_rt::test]
    async fn search_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let (client, server) = cfg.start_server(move |cfg| api_scope(cfg.pool.clone()));

        // Search for the existing block by number.
        let block_info = client
            .search_block(1)
            .await?
            .expect("block should be exist");
        // Search for the existing block by root hash.
        assert_eq!(
            client
                .search_block(block_info.new_state_root)
                .await?
                .unwrap(),
            block_info
        );
        // Search for the existing block by committed tx hash.
        assert_eq!(
            client
                .search_block(block_info.commit_tx_hash.unwrap())
                .await?
                .unwrap(),
            block_info
        );
        // Search for the existing block by verified tx hash.
        assert_eq!(
            client
                .search_block(block_info.verify_tx_hash.unwrap())
                .await?
                .unwrap(),
            block_info
        );

        server.stop().await;
        Ok(())
    }
}
