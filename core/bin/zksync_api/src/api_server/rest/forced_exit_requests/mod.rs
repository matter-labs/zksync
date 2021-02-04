//! First stable API implementation.

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
pub use zksync_api_client::rest::v1::{
    Client, ClientError, Pagination, PaginationQuery, MAX_LIMIT,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;

// Local uses
use crate::api_server::tx_sender::TxSender;

use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{Duration, Utc};
use futures::channel::mpsc;
use num::{bigint::ToBigInt, BigUint};
use std::ops::Add;
use std::str::FromStr;
use std::time::Instant;

// Workspace uses
pub use zksync_api_client::rest::v1::{
    FastProcessingQuery, IncomingTx, IncomingTxBatch, Receipt, TxData,
};
use zksync_types::{
    forced_exit_requests::{ForcedExitRequest, SaveForcedExitRequestQuery},
    TokenLike, TxFeeTypes,
};

// Local uses
use crate::api_server::rest::v1::{Error as ApiError, JsonResult};

use crate::{
    api_server::{
        forced_exit_checker::ForcedExitChecker,
        tx_sender::{ticker_request, SubmitError},
    },
    fee_ticker::TickerRequest,
};

mod v01;

pub(crate) fn api_scope(
    connection_pool: ConnectionPool,
    config: &ZkSyncConfig,
    ticker_request_sender: mpsc::Sender<TickerRequest>,
) -> Scope {
    web::scope("/api/forced_exit_requests").service(v01::api_scope(
        connection_pool,
        config,
        ticker_request_sender,
    ))
}
