//! Operations part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_storage::{ConnectionPool, QueryResult};
use zksync_types::BlockNumber;

// Local uses
use super::{blocks::BlockInfo, transactions::TxReceipt, Error as ApiError, JsonResult};

/// Shared data between `api/v1/operations` endpoints.
#[derive(Debug, Clone)]
struct ApiOperationsData {
    pool: ConnectionPool,
}

impl ApiOperationsData {
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }

    pub async fn priority_op(&self, serial_id: u64) -> QueryResult<Option<PriorityOpReceipt>> {
        let mut storage = self.pool.access_storage().await?;

        let executed_op = storage
            .chain()
            .operations_schema()
            .get_executed_priority_operation(serial_id as u32)
            .await?;

        let executed_op = if let Some(executed_op) = executed_op {
            executed_op
        } else {
            return Ok(None);
        };

        let blocks = storage
            .chain()
            .block_schema()
            .load_block_range(executed_op.block_number as BlockNumber, 1)
            .await?;

        let block_info = blocks.into_iter().next().map(BlockInfo::from);

        let status = match block_info {
            None => todo!(),
            Some(info) if info.commit_tx_hash.is_some() => TxReceipt::Committed {
                block: info.block_number,
            },
            Some(info) if info.verify_tx_hash.is_some() => TxReceipt::Verified {
                block: info.block_number,
            },
            Some(_) => TxReceipt::Executed,
        };

        Ok(Some(PriorityOpReceipt {
            status,
            index: executed_op.block_index as u64,
        }))
    }
}

// Data transfer objects.

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PriorityOpReceipt {
    status: TxReceipt,
    index: u64,
}

// Client implementation

// Server implementation

async fn priority_op(
    data: web::Data<ApiOperationsData>,
    web::Path(serial_id): web::Path<u64>,
) -> JsonResult<Option<PriorityOpReceipt>> {
    let receipt = data
        .priority_op(serial_id)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(receipt))
}

pub fn api_scope(pool: ConnectionPool) -> Scope {
    let data = ApiOperationsData::new(pool);

    web::scope("operations")
        .data(data)
        .route("priority_op/{id}", web::get().to(priority_op))
}

#[cfg(test)]
mod tests {}
