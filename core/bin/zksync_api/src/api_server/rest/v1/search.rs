//! Search part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_api_client::rest::v1::BlockSearchQuery;
use zksync_storage::{ConnectionPool, QueryResult};

// Local uses
use super::{
    blocks::{convert::block_info_from_details, BlockInfo},
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

        Ok(block.map(block_info_from_details))
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
    use zksync_types::BlockNumber;

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn search_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let (client, server) = cfg.start_server(move |cfg| api_scope(cfg.pool.clone()));

        // Search for the existing block by number.
        let block_info = client
            .search_block(BlockNumber(1))
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
