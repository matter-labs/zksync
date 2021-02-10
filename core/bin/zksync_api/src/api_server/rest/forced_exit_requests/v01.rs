//! Transactions part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{Duration, Utc};
use futures::channel::mpsc;
use num::{bigint::ToBigInt, BigUint};
use std::ops::Add;
use std::str::FromStr;
use std::time::Instant;
use zksync_api_client::rest::forced_exit_requests::ConfigInfo;

// Workspace uses
pub use zksync_api_client::rest::forced_exit_requests::{
    ForcedExitRegisterRequest, ForcedExitRequestStatus,
};
pub use zksync_api_client::rest::v1::{
    FastProcessingQuery, IncomingTx, IncomingTxBatch, Receipt, TxData,
};

use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;
use zksync_types::{
    forced_exit_requests::{ForcedExitRequest, ForcedExitRequestId, SaveForcedExitRequestQuery},
    TokenLike, TxFeeTypes,
};

// Local uses
use crate::api_server::rest::v1::{Error as ApiError, JsonResult};

use crate::{
    api_server::{
        forced_exit_checker::ForcedExitChecker,
        tx_sender::{SubmitError, TxSender},
    },
    fee_ticker::TickerRequest,
};

/// Shared data between `api/v1/transactions` endpoints.
#[derive(Clone)]
pub struct ApiForcedExitRequestsData {
    pub(crate) connection_pool: ConnectionPool,
    pub(crate) forced_exit_checker: ForcedExitChecker,
    pub(crate) ticker_request_sender: mpsc::Sender<TickerRequest>,

    pub(crate) is_enabled: bool,
    pub(crate) max_tokens_per_request: u8,
    pub(crate) recomended_tx_interval_millisecs: i64,
    pub(crate) max_tx_interval_millisecs: i64,
    pub(crate) price_per_token: i64,
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
            price_per_token: config.forced_exit_requests.price_per_token,
            max_tokens_per_request: config.forced_exit_requests.max_tokens_per_request,
            recomended_tx_interval_millisecs: config.forced_exit_requests.recomended_tx_interval,
            max_tx_interval_millisecs: config.forced_exit_requests.max_tx_interval,
        }
    }
}

// Server implementation

async fn get_status(
    data: web::Data<ApiForcedExitRequestsData>,
) -> JsonResult<ForcedExitRequestStatus> {
    let start = Instant::now();

    let response = if data.is_enabled {
        ForcedExitRequestStatus::Enabled(ConfigInfo {
            request_fee: BigUint::from(data.price_per_token as u64),
            max_tokens_per_request: data.max_tokens_per_request,
            recomended_tx_interval_millis: data.recomended_tx_interval_millisecs,
        })
    } else {
        ForcedExitRequestStatus::Disabled
    };

    metrics::histogram!("api.forced_exit_requests.v01.status", start.elapsed());
    Ok(Json(response))
}

pub async fn submit_request(
    data: web::Data<ApiForcedExitRequestsData>,
    params: web::Json<ForcedExitRegisterRequest>,
) -> JsonResult<ForcedExitRequest> {
    let start = Instant::now();

    let mut storage = data.connection_pool.access_storage().await.map_err(|err| {
        vlog::warn!("Internal Server Error: '{}';", err);
        return ApiError::internal("");
    })?;

    data.forced_exit_checker
        .check_forced_exit(&mut storage, params.target)
        .await
        .map_err(ApiError::from)?;

    let price_of_one_exit = BigDecimal::from(data.price_per_token);
    let price_of_request = price_of_one_exit * BigDecimal::from_usize(params.tokens.len()).unwrap();

    let user_fee = params.price_in_wei.to_bigint().unwrap();
    let user_fee = BigDecimal::from(user_fee);
    let user_scaling_coefficient = BigDecimal::from_str("1.05").unwrap();
    let user_scaled_fee = user_scaling_coefficient * user_fee;

    if user_scaled_fee < price_of_request {
        return Err(ApiError::bad_request("Not enough fee"));
    }

    if params.tokens.len() > data.max_tokens_per_request as usize {
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
            .map_err(|_| {
                return ApiError::bad_request("One of the tokens does no exist");
            })?;
    }

    let mut fe_schema = storage.forced_exit_requests_schema();

    let valid_until = Utc::now().add(Duration::milliseconds(data.max_tx_interval_millisecs));

    let saved_fe_request = fe_schema
        .store_request(SaveForcedExitRequestQuery {
            target: params.target,
            tokens: params.tokens.clone(),
            price_in_wei: params.price_in_wei.clone(),
            valid_until,
        })
        .await
        .map_err(|_| {
            return ApiError::internal("");
        })?;

    metrics::histogram!(
        "api.forced_exit_requests.v01.submit_request",
        start.elapsed()
    );
    Ok(Json(saved_fe_request))
}

pub async fn get_request_by_id(
    data: web::Data<ApiForcedExitRequestsData>,
    web::Path(request_id): web::Path<ForcedExitRequestId>,
) -> JsonResult<ForcedExitRequest> {
    let start = Instant::now();

    let mut storage = data.connection_pool.access_storage().await.map_err(|err| {
        vlog::warn!("Internal Server Error: '{}';", err);
        return ApiError::internal("");
    })?;

    let mut fe_requests_schema = storage.forced_exit_requests_schema();

    metrics::histogram!(
        "api.forced_exit_requests.v01.get_request_by_id",
        start.elapsed()
    );

    let fe_request_from_db = fe_requests_schema
        .get_request_by_id(request_id)
        .await
        .map_err(ApiError::internal)?;

    match fe_request_from_db {
        Some(fe_request) => Ok(Json(fe_request)),
        None => Err(ApiError::not_found("Request with such id does not exist")),
    }
}

pub fn api_scope(
    connection_pool: ConnectionPool,
    config: &ZkSyncConfig,
    ticker_request_sender: mpsc::Sender<TickerRequest>,
) -> Scope {
    let data = ApiForcedExitRequestsData::new(connection_pool, config, ticker_request_sender);

    // `enabled` endpoint should always be there
    let scope = web::scope("v0.1")
        .data(data)
        .route("status", web::get().to(get_status));

    if config.forced_exit_requests.enabled {
        scope
            .route("/submit", web::post().to(submit_request))
            .route("/requests/{id}", web::get().to(get_request_by_id))
    } else {
        scope
    }
}

#[cfg(test)]
mod tests {
    use bigdecimal::BigDecimal;
    use futures::{channel::mpsc, StreamExt};
    use num::BigUint;

    use zksync_api_client::rest::v1::Client;
    use zksync_config::ForcedExitRequestsConfig;
    use zksync_storage::ConnectionPool;
    use zksync_types::tokens::TokenLike;
    use zksync_types::Address;

    use crate::fee_ticker::{Fee, OutputFeeType::Withdraw, TickerRequest};

    use super::*;
    use crate::api_server::v1::test_utils::TestServerConfig;

    // fn dummy_fee_ticker(zkp_fee: Option<u64>, gas_fee: Option<u64>) -> mpsc::Sender<TickerRequest> {
    //     let (sender, mut receiver) = mpsc::channel(10);

    //     let zkp_fee = zkp_fee.unwrap_or(1_u64);
    //     let gas_fee = gas_fee.unwrap_or(1_u64);

    //     actix_rt::spawn(async move {
    //         while let Some(item) = receiver.next().await {
    //             match item {
    //                 TickerRequest::GetTxFee { response, .. } => {
    //                     let fee = Ok(Fee::new(
    //                         Withdraw,
    //                         BigUint::from(zkp_fee).into(),
    //                         BigUint::from(gas_fee).into(),
    //                         1_u64.into(),
    //                         1_u64.into(),
    //                     ));

    //                     response.send(fee).expect("Unable to send response");
    //                 }
    //                 TickerRequest::GetTokenPrice { response, .. } => {
    //                     let price = Ok(BigDecimal::from(1_u64));

    //                     response.send(price).expect("Unable to send response");
    //                 }
    //                 TickerRequest::IsTokenAllowed { token, response } => {
    //                     // For test purposes, PHNX token is not allowed.
    //                     let is_phnx = match token {
    //                         TokenLike::Id(id) => id == 1,
    //                         TokenLike::Symbol(sym) => sym == "PHNX",
    //                         TokenLike::Address(_) => unreachable!(),
    //                     };
    //                     response.send(Ok(!is_phnx)).unwrap_or_default();
    //                 }
    //             }
    //         }
    //     });

    //     sender
    // }

    struct TestServer {
        api_server: actix_web::test::TestServer,
        #[allow(dead_code)]
        pool: ConnectionPool,
        #[allow(dead_code)]
        fee_ticker: mpsc::Sender<TickerRequest>,
    }

    impl TestServer {
        // It should be used in the test for submitting requests
        #[allow(dead_code)]
        async fn new() -> anyhow::Result<(Client, Self)> {
            let cfg = TestServerConfig::default();

            Self::new_with_config(cfg).await
        }

        async fn new_with_config(cfg: TestServerConfig) -> anyhow::Result<(Client, Self)> {
            let pool = cfg.pool.clone();

            let fee_ticker = dummy_fee_ticker(None, None);

            let fee_ticker2 = fee_ticker.clone();
            let (api_client, api_server) = cfg
                .start_server_with_scope(String::from("api/forced_exit_requests"), move |cfg| {
                    api_scope(cfg.pool.clone(), &cfg.config, fee_ticker2.clone())
                });

            Ok((
                api_client,
                Self {
                    api_server,
                    pool,
                    fee_ticker,
                },
            ))
        }

        async fn new_with_fee_ticker(
            cfg: TestServerConfig,
            gas_fee: Option<u64>,
            zkp_fee: Option<u64>,
        ) -> anyhow::Result<(Client, Self)> {
            let pool = cfg.pool.clone();

            let fee_ticker = dummy_fee_ticker(gas_fee, zkp_fee);

            let fee_ticker2 = fee_ticker.clone();
            let (api_client, api_server) = cfg
                .start_server_with_scope(String::from("/api/forced_exit_requests"), move |cfg| {
                    api_scope(cfg.pool.clone(), &cfg.config, fee_ticker2.clone())
                });

            Ok((
                api_client,
                Self {
                    api_server,
                    pool,
                    fee_ticker,
                },
            ))
        }

        async fn stop(self) {
            self.api_server.stop().await;
        }
    }

    fn get_test_config_from_forced_exit_requests(
        forced_exit_requests: ForcedExitRequestsConfig,
    ) -> TestServerConfig {
        let config_from_env = ZkSyncConfig::from_env();
        let config = ZkSyncConfig {
            forced_exit_requests,
            ..config_from_env
        };

        TestServerConfig {
            config,
            pool: ConnectionPool::new(Some(1)),
        }
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn test_disabled_forced_exit_requests() -> anyhow::Result<()> {
        let forced_exit_requests = ForcedExitRequestsConfig::from_env();
        let test_config = get_test_config_from_forced_exit_requests(ForcedExitRequestsConfig {
            enabled: false,
            ..forced_exit_requests
        });

        let (client, server) = TestServer::new_with_config(test_config).await?;

        let status = client.get_forced_exit_requests_status().await?;

        assert_eq!(status, ForcedExitRequestStatus::Disabled);

        let should_be_disabled_msg = "Forced-exit related requests don't fail when it's disabled";
        let register_request = ForcedExitRegisterRequest {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![0],
            price_in_wei: BigUint::from_str("1212").unwrap(),
        };

        client
            .submit_forced_exit_request(register_request)
            .await
            .expect_err(should_be_disabled_msg);

        server.stop().await;
        Ok(())
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn test_forced_exit_requests_get_fee() -> anyhow::Result<()> {
        let forced_exit_requests = ForcedExitRequestsConfig::from_env();
        let test_config = get_test_config_from_forced_exit_requests(ForcedExitRequestsConfig {
            price_per_token: 1000000000,
            ..forced_exit_requests
        });

        let (client, server) = TestServer::new_with_config(test_config).await?;

        let status = client.get_forced_exit_requests_status().await?;

        match status {
            ForcedExitRequestStatus::Enabled(config_info) => {
                assert_eq!(
                    config_info.request_fee,
                    BigUint::from_u32(1000000000).unwrap()
                );
            }
            ForcedExitRequestStatus::Disabled => {
                panic!("ForcedExitRequests feature is not disabled");
            }
        }

        server.stop().await;
        Ok(())
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn test_forced_exit_requests_wrongs_tokens_number() -> anyhow::Result<()> {
        let forced_exit_requests = ForcedExitRequestsConfig::from_env();
        let test_config = get_test_config_from_forced_exit_requests(ForcedExitRequestsConfig {
            max_tokens_per_request: 5,
            ..forced_exit_requests
        });

        let (client, server) =
            TestServer::new_with_fee_ticker(test_config, Some(10000), Some(10000)).await?;

        let status = client.get_forced_exit_requests_status().await?;

        assert_ne!(status, ForcedExitRequestStatus::Disabled);

        let register_request = ForcedExitRegisterRequest {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![0, 1, 2, 3, 4, 5, 6, 7],
            price_in_wei: BigUint::from_str("1212").unwrap(),
        };

        client
            .submit_forced_exit_request(register_request)
            .await
            .expect_err("Api does not take the limit on the number of tokens into account");

        server.stop().await;
        Ok(())
    }

    // #[actix_rt::test]
    // #[cfg_attr(
    //     not(feature = "api_test"),
    //     ignore = "Use `zk test rust-api` command to perform this test"
    // )]
    // async fn test_forced_exit_requests_submit() -> anyhow::Result<()>  {
    //     let (client, server) = TestServer::new().await?;

    //     let enabled = client.are_forced_exit_requests_enabled().await?.enabled;
    //     assert_eq!(enabled, true);

    //     let fee = client.get_forced_exit_request_fee().await?.request_fee;

    //     let fe_request = ForcedExitRegisterRequest {
    //         target: ""
    //     };

    //     Ok(())
    // }
}
