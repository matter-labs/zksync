//! Transactions part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

use crate::{
    api_server::{
        forced_exit_checker::ForcedExitChecker,
        helpers::try_parse_hash,
        rest::{
            helpers::{deposit_op_to_tx_by_hash, parse_tx_id, priority_op_to_tx_history},
            v01::{api_decl::ApiV01, types::*},
        },
        tx_sender::ticker_request,
    },
    fee_ticker::{Fee, TickerRequest},
};
use actix_web::{HttpResponse, Result as ActixResult};
use std::ops::{Add, Mul};

use bigdecimal::{BigDecimal, FromPrimitive};
use futures::{channel::mpsc, SinkExt, TryFutureExt};
use num::{
    bigint::{ToBigInt, ToBigUint},
    BigUint,
};

use zksync_config::{test_config::unit_vectors::ForcedExit, ZkSyncConfig};

use chrono::{DateTime, Duration, Utc};
use std::str::FromStr;
use std::time::Instant;
use zksync_storage::{chain::operations_ext::SearchDirection, ConnectionPool};
use zksync_types::{
    misc::{ForcedExitRequest, SaveForcedExitRequestQuery},
    Address, BlockNumber, TokenId, TokenLike, TxFeeTypes,
};

// Workspace uses
pub use zksync_api_client::rest::v1::{
    FastProcessingQuery, IncomingTx, IncomingTxBatch, Receipt, TxData,
};
use zksync_storage::{
    chain::operations_ext::records::TxReceiptResponse, QueryResult, StorageProcessor,
};
use zksync_types::{tx::TxHash, SignedZkSyncTx};

// Local uses
use super::{Client, ClientError, Error as ApiError, JsonResult, Pagination, PaginationQuery};
use crate::api_server::rpc_server::types::TxWithSignature;
use crate::api_server::tx_sender::{SubmitError, TxSender};

use serde::{Deserialize, Serialize};
use zksync_utils::BigUintSerdeAsRadix10Str;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForcedExitRequestFee {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub request_fee: BigUint,
}

#[derive(Serialize, Deserialize)]
pub struct IsForcedExitEnabledResponse {
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct ForcedExitRegisterRequest {
    pub target: Address,
    pub tokens: Vec<TokenId>,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub price_in_wei: BigUint,
}

/// Shared data between `api/v1/transactions` endpoints.
#[derive(Clone)]
pub struct ApiForcedExitRequestsData {
    pub(crate) connection_pool: ConnectionPool,
    pub(crate) forced_exit_checker: ForcedExitChecker,
    pub(crate) ticker_request_sender: mpsc::Sender<TickerRequest>,

    pub(crate) is_enabled: bool,
    pub(crate) price_scaling_factor: BigDecimal,
    pub(crate) max_tokens: u8,
}

impl ApiForcedExitRequestsData {
    fn new(
        connection_pool: ConnectionPool,
        config: &ZkSyncConfig,
        ticker_request_sender: mpsc::Sender<TickerRequest>,
    ) -> Self {
        let forced_exit_checker = ForcedExitChecker::new(&config);
        Self {
            connection_pool,
            forced_exit_checker,
            ticker_request_sender,

            is_enabled: config.forced_exit_requests.enabled,
            price_scaling_factor: BigDecimal::from_f64(
                config.forced_exit_requests.price_scaling_factor,
            )
            .unwrap(),
            max_tokens: config.forced_exit_requests.max_tokens,
        }
    }

    async fn tx_receipt(
        storage: &mut StorageProcessor<'_>,
        tx_hash: TxHash,
    ) -> QueryResult<Option<TxReceiptResponse>> {
        storage
            .chain()
            .operations_ext_schema()
            .tx_receipt(tx_hash.as_ref())
            .await
    }
}

// Server implementation

async fn is_enabled(
    data: web::Data<ApiForcedExitRequestsData>,
) -> JsonResult<IsForcedExitEnabledResponse> {
    let start = Instant::now();

    let response = IsForcedExitEnabledResponse {
        enabled: data.is_enabled,
    };

    metrics::histogram!("api.v01.is_forced_exit_enabled", start.elapsed());
    Ok(Json(response))
}

async fn get_forced_exit_request_fee(
    ticker_request_sender: mpsc::Sender<TickerRequest>,
    price_scaling_factor: BigDecimal,
) -> Result<BigUint, SubmitError> {
    let price = ticker_request(
        ticker_request_sender.clone(),
        TxFeeTypes::Withdraw,
        TokenLike::Id(0),
    )
    .await?;
    let price = BigDecimal::from(price.total_fee.to_bigint().unwrap());

    let scaled_price = price * price_scaling_factor;
    let scaled_price = scaled_price.round(0).to_bigint().unwrap();

    Ok(scaled_price.to_biguint().unwrap())
}

async fn get_fee(data: web::Data<ApiForcedExitRequestsData>) -> JsonResult<ForcedExitRequestFee> {
    let request_fee = get_forced_exit_request_fee(
        data.ticker_request_sender.clone(),
        data.price_scaling_factor.clone(),
    )
    .await
    .map_err(ApiError::from)?;

    Ok(Json(ForcedExitRequestFee { request_fee }))
}

pub async fn submit_request(
    data: web::Data<ApiForcedExitRequestsData>,
    params: web::Json<ForcedExitRegisterRequest>,
) -> JsonResult<ForcedExitRequest> {
    let start = Instant::now();

    if !data.is_enabled {
        return Err(ApiError::bad_request(
            "ForcedExit requests feature is disabled!",
        ));
    }

    let mut storage = data.connection_pool.access_storage().await.map_err(|err| {
        vlog::warn!("Internal Server Error: '{}';", err);
        return ApiError::internal("");
    })?;

    data.forced_exit_checker
        .check_forced_exit(&mut storage, params.target)
        .await
        .map_err(ApiError::from)?;

    let price = get_forced_exit_request_fee(
        data.ticker_request_sender.clone(),
        data.price_scaling_factor.clone(),
    )
    .await
    .map_err(ApiError::from)?;
    let price = BigDecimal::from(price.to_bigint().unwrap());

    let user_fee = params.price_in_wei.to_bigint().unwrap();
    let user_fee = BigDecimal::from(user_fee);
    let user_scaling_coefficient = BigDecimal::from_str("1.05").unwrap();
    let user_scaled_fee = user_scaling_coefficient * user_fee;

    if user_scaled_fee < price {
        return Err(ApiError::bad_request("Not enough fee"));
    }

    if params.tokens.len() > 10 {
        return Err(ApiError::bad_request(
            "Maximum number of tokens per FE request exceeded",
        ));
    }

    let mut tokens_schema = storage.tokens_schema();

    for token_id in params.tokens.iter() {
        // The result is going nowhere.
        // This is simply to make sure that the tokens
        // that were supplied do indeed exist
        tokens_schema
            .get_token(TokenLike::Id(*token_id))
            .await
            .map_err(|e| {
                return ApiError::bad_request("One of the tokens does no exist");
            })?;
    }

    let mut fe_schema = storage.forced_exit_requests_schema();

    let valid_until = Utc::now().add(Duration::weeks(1));

    let saved_fe_request = fe_schema
        .store_request(SaveForcedExitRequestQuery {
            target: params.target,
            tokens: params.tokens.clone(),
            price_in_wei: params.price_in_wei.clone(),
            valid_until,
        })
        .await
        .map_err(|e| {
            return ApiError::internal("");
        })?;

    metrics::histogram!("api.v01.register_forced_exit_request", start.elapsed());
    Ok(Json(saved_fe_request))
}

pub fn api_scope(
    connection_pool: ConnectionPool,
    config: &ZkSyncConfig,
    ticker_request_sender: mpsc::Sender<TickerRequest>,
) -> Scope {
    let data = ApiForcedExitRequestsData::new(connection_pool, config, ticker_request_sender);

    web::scope("forced_exit")
        .data(data)
        .route("enabled", web::get().to(is_enabled))
        .route("submit", web::post().to(submit_request))
        .route("fee", web::get().to(get_fee))
}

//#[cfg(test)]
// mod tests {
//     use actix_web::App;
//     use bigdecimal::BigDecimal;
//     use futures::{channel::mpsc, StreamExt};
//     use num::BigUint;

//     use zksync_api_client::rest::v1::Client;
//     use zksync_storage::ConnectionPool;
//     use zksync_test_account::ZkSyncAccount;
//     use zksync_types::{
//         tokens::TokenLike,
//         tx::{PackedEthSignature, TxEthSignature},
//         ZkSyncTx,
//     };

//     use crate::{
//         // api_server::helpers::try_parse_tx_hash,
//         core_api_client::CoreApiClient,
//         fee_ticker::{Fee, OutputFeeType::Withdraw, TickerRequest},
//         signature_checker::{VerifiedTx, VerifyTxSignatureRequest},
//     };

//     use super::super::test_utils::{TestServerConfig, TestTransactions};
//     use super::*;

//     fn submit_txs_loopback() -> (CoreApiClient, actix_web::test::TestServer) {
//         async fn send_tx(_tx: Json<SignedZkSyncTx>) -> Json<Result<(), ()>> {
//             Json(Ok(()))
//         }

//         async fn send_txs_batch(
//             _txs: Json<(Vec<SignedZkSyncTx>, Vec<TxEthSignature>)>,
//         ) -> Json<Result<(), ()>> {
//             Json(Ok(()))
//         }

//         let server = actix_web::test::start(move || {
//             App::new()
//                 .route("new_tx", web::post().to(send_tx))
//                 .route("new_txs_batch", web::post().to(send_txs_batch))
//         });

//         let url = server.url("").trim_end_matches('/').to_owned();

//         (CoreApiClient::new(url), server)
//     }

//     fn dummy_fee_ticker() -> mpsc::Sender<TickerRequest> {
//         let (sender, mut receiver) = mpsc::channel(10);

//         actix_rt::spawn(async move {
//             while let Some(item) = receiver.next().await {
//                 match item {
//                     TickerRequest::GetTxFee { response, .. } => {
//                         let fee = Ok(Fee::new(
//                             Withdraw,
//                             BigUint::from(1_u64).into(),
//                             BigUint::from(1_u64).into(),
//                             1_u64.into(),
//                             1_u64.into(),
//                         ));

//                         response.send(fee).expect("Unable to send response");
//                     }
//                     TickerRequest::GetTokenPrice { response, .. } => {
//                         let price = Ok(BigDecimal::from(1_u64));

//                         response.send(price).expect("Unable to send response");
//                     }
//                     TickerRequest::IsTokenAllowed { token, response } => {
//                         // For test purposes, PHNX token is not allowed.
//                         let is_phnx = match token {
//                             TokenLike::Id(id) => id == 1,
//                             TokenLike::Symbol(sym) => sym == "PHNX",
//                             TokenLike::Address(_) => unreachable!(),
//                         };
//                         response.send(Ok(!is_phnx)).unwrap_or_default();
//                     }
//                 }
//             }
//         });

//         sender
//     }

//     fn dummy_sign_verifier() -> mpsc::Sender<VerifyTxSignatureRequest> {
//         let (sender, mut receiver) = mpsc::channel::<VerifyTxSignatureRequest>(10);

//         actix_rt::spawn(async move {
//             while let Some(item) = receiver.next().await {
//                 let verified = VerifiedTx::unverified(item.tx);
//                 item.response
//                     .send(Ok(verified))
//                     .expect("Unable to send response");
//             }
//         });

//         sender
//     }

//     struct TestServer {
//         core_server: actix_web::test::TestServer,
//         api_server: actix_web::test::TestServer,
//         #[allow(dead_code)]
//         pool: ConnectionPool,
//     }

//     impl TestServer {
//         async fn new() -> anyhow::Result<(Client, Self)> {
//             let (core_client, core_server) = submit_txs_loopback();

//             let cfg = TestServerConfig::default();
//             let pool = cfg.pool.clone();
//             cfg.fill_database().await?;

//             let sign_verifier = dummy_sign_verifier();
//             let fee_ticker = dummy_fee_ticker();

//             let (api_client, api_server) = cfg.start_server(move |cfg| {
//                 api_scope(TxSender::with_client(
//                     core_client.clone(),
//                     cfg.pool.clone(),
//                     sign_verifier.clone(),
//                     fee_ticker.clone(),
//                     &cfg.config,
//                 ))
//             });

//             Ok((
//                 api_client,
//                 Self {
//                     core_server,
//                     api_server,
//                     pool,
//                 },
//             ))
//         }

//         async fn stop(self) {
//             self.api_server.stop().await;
//             self.core_server.stop().await;
//         }
//     }

//     #[actix_rt::test]
//     #[cfg_attr(
//         not(feature = "api_test"),
//         ignore = "Use `zk test rust-api` command to perform this test"
//     )]
//     async fn test_submit_txs_loopback() -> anyhow::Result<()> {
//         let (core_client, core_server) = submit_txs_loopback();

//         let signed_tx = SignedZkSyncTx {
//             tx: TestServerConfig::gen_zk_txs(0).txs[0].0.clone(),
//             eth_sign_data: None,
//         };

//         core_client.send_tx(signed_tx.clone()).await??;
//         core_client
//             .send_txs_batch(vec![signed_tx], vec![])
//             .await??;

//         core_server.stop().await;
//         Ok(())
//     }

//     #[actix_rt::test]
//     #[cfg_attr(
//         not(feature = "api_test"),
//         ignore = "Use `zk test rust-api` command to perform this test"
//     )]
//     async fn test_transactions_scope() -> anyhow::Result<()> {
//         todo!();

//     }
// }
