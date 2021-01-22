//! Operations part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_api_client::rest::v1::{
    PriorityOpData, PriorityOpQuery, PriorityOpQueryError, PriorityOpReceipt,
};
use zksync_storage::{
    chain::operations::records::StoredExecutedPriorityOperation, ConnectionPool, QueryResult,
    StorageProcessor,
};
use zksync_types::{BlockNumber, H256};

// Local uses
use super::{transactions::Receipt, Error as ApiError, JsonResult};

/// Shared data between `api/v1/operations` endpoints.
#[derive(Debug, Clone)]
struct ApiOperationsData {
    pool: ConnectionPool,
}

impl ApiOperationsData {
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }

    pub async fn priority_op_data(
        &self,
        query: PriorityOpQuery,
    ) -> QueryResult<Option<PriorityOpData>> {
        let mut storage = self.pool.access_storage().await?;

        let executed_op = executed_priority_op_for_query(query, &mut storage).await?;
        Ok(executed_op.map(convert::priority_op_data_from_stored))
    }

    pub async fn priority_op(
        &self,
        query: PriorityOpQuery,
    ) -> QueryResult<Option<PriorityOpReceipt>> {
        let mut storage = self.pool.access_storage().await?;

        let executed_op = executed_priority_op_for_query(query, &mut storage).await?;
        let executed_op = if let Some(executed_op) = executed_op {
            executed_op
        } else {
            return Ok(None);
        };

        let blocks = storage
            .chain()
            .block_schema()
            .load_block_range(BlockNumber(executed_op.block_number as u32), 1)
            .await?;

        let block_info = blocks
            .into_iter()
            .next()
            .expect("Database provided an incorrect priority op receipt");

        let block = BlockNumber(block_info.block_number as u32);
        let index = executed_op.block_index as u32;

        let receipt = if block_info.verify_tx_hash.is_some() {
            PriorityOpReceipt {
                status: Receipt::Verified { block },
                index: Some(index),
            }
        } else if block_info.commit_tx_hash.is_some() {
            PriorityOpReceipt {
                status: Receipt::Committed { block },
                index: Some(index),
            }
        } else {
            PriorityOpReceipt {
                status: Receipt::Executed,
                index: None,
            }
        };

        Ok(Some(receipt))
    }
}

async fn executed_priority_op_for_query(
    query: PriorityOpQuery,
    storage: &mut StorageProcessor<'_>,
) -> QueryResult<Option<StoredExecutedPriorityOperation>> {
    let mut schema = storage.chain().operations_schema();

    match query {
        PriorityOpQuery::Id(serial_id) => {
            schema
                .get_executed_priority_operation(serial_id as u32)
                .await
        }
        PriorityOpQuery::Hash(eth_hash) => {
            schema
                .get_executed_priority_operation_by_hash(eth_hash.as_bytes())
                .await
        }
    }
}

mod convert {
    use super::*;

    pub fn priority_op_data_from_stored(v: StoredExecutedPriorityOperation) -> PriorityOpData {
        PriorityOpData {
                data: serde_json::from_value(v.operation.clone()).unwrap_or_else(|err|
                    panic!(
                        "Database provided an incorrect priority operation data: {:?}, an error occurred: {}",
                        v.operation, err
                    )
                ),
                eth_hash: H256::from_slice(&v.eth_hash),
                serial_id: v.priority_op_serialid as u64,
            }
    }

    impl From<PriorityOpQueryError> for ApiError {
        fn from(err: PriorityOpQueryError) -> Self {
            ApiError::bad_request("Cannot parse PrioorityOpQuery").detail(err.detail)
        }
    }
}

// Server implementation

async fn priority_op(
    data: web::Data<ApiOperationsData>,
    web::Path(path): web::Path<String>,
) -> JsonResult<Option<PriorityOpReceipt>> {
    let query = PriorityOpQuery::from_path(path)?;

    let receipt = data.priority_op(query).await.map_err(ApiError::internal)?;
    Ok(Json(receipt))
}

async fn priority_op_data(
    data: web::Data<ApiOperationsData>,
    web::Path(path): web::Path<String>,
) -> JsonResult<Option<PriorityOpData>> {
    let query = PriorityOpQuery::from_path(path)?;

    let data = data
        .priority_op_data(query)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(data))
}

pub fn api_scope(pool: ConnectionPool) -> Scope {
    let data = ApiOperationsData::new(pool);

    web::scope("operations")
        .data(data)
        .route("{id}", web::get().to(priority_op))
        .route("{id}/data", web::get().to(priority_op_data))
}

#[cfg(test)]
mod tests {
    use zksync_storage::test_data::dummy_ethereum_tx_hash;
    use zksync_types::{AccountId, Address};

    use crate::api_server::v1::test_utils::{dummy_deposit_op, dummy_full_exit_op};

    use super::{
        super::test_utils::{TestServerConfig, COMMITTED_OP_SERIAL_ID, VERIFIED_OP_SERIAL_ID},
        *,
    };

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn operations_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let (client, server) = cfg.start_server(|cfg| api_scope(cfg.pool.clone()));

        // Check verified priority operation.

        let verified_op_hash = dummy_ethereum_tx_hash(VERIFIED_OP_SERIAL_ID as i64);

        let expected_receipt = PriorityOpReceipt {
            index: Some(2),
            status: Receipt::Verified {
                block: BlockNumber(2),
            },
        };
        assert_eq!(
            client.priority_op(VERIFIED_OP_SERIAL_ID).await?.as_ref(),
            Some(&expected_receipt)
        );
        assert_eq!(
            client.priority_op(verified_op_hash).await?.as_ref(),
            Some(&expected_receipt)
        );

        let expected_data = PriorityOpData {
            data: dummy_deposit_op(Address::default(), AccountId(1), 15, 2).op,
            serial_id: VERIFIED_OP_SERIAL_ID,
            eth_hash: verified_op_hash,
        };

        assert_eq!(
            client
                .priority_op_data(VERIFIED_OP_SERIAL_ID)
                .await?
                .as_ref()
                .unwrap()
                .serial_id,
            expected_data.serial_id
        );
        assert_eq!(
            client
                .priority_op_data(verified_op_hash)
                .await?
                .unwrap()
                .eth_hash,
            expected_data.eth_hash
        );

        // Check committed priority operation.
        let committed_eth_hash = dummy_ethereum_tx_hash(COMMITTED_OP_SERIAL_ID as i64);

        let expected_receipt = PriorityOpReceipt {
            index: Some(1),
            status: Receipt::Committed {
                block: BlockNumber(4),
            },
        };
        assert_eq!(
            client.priority_op(COMMITTED_OP_SERIAL_ID).await?.as_ref(),
            Some(&expected_receipt)
        );
        assert_eq!(
            client.priority_op(committed_eth_hash).await?.as_ref(),
            Some(&expected_receipt)
        );

        let expected_data = PriorityOpData {
            data: dummy_full_exit_op(AccountId(1), Address::default(), 16, 3).op,
            serial_id: COMMITTED_OP_SERIAL_ID,
            eth_hash: committed_eth_hash,
        };
        assert_eq!(
            client
                .priority_op_data(COMMITTED_OP_SERIAL_ID)
                .await?
                .unwrap()
                .eth_hash,
            expected_data.eth_hash
        );
        assert_eq!(
            client
                .priority_op_data(committed_eth_hash)
                .await?
                .unwrap()
                .serial_id,
            expected_data.serial_id
        );

        // Try to get non-existing priority operation.
        assert!(client.priority_op(1000).await?.is_none());
        assert!(client.priority_op(H256::default()).await?.is_none());

        server.stop().await;
        Ok(())
    }
}
