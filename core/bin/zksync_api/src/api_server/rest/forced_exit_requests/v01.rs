//! Transactions part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{Duration, Utc};
use num::{bigint::ToBigInt, BigUint};
use std::time::Instant;
use std::{convert::TryInto, ops::Add};
// Workspace uses
pub use zksync_api_client::rest::forced_exit_requests::{
    ForcedExitRegisterRequest, ForcedExitRequestStatus,
};
pub use zksync_api_client::rest::v1::{
    FastProcessingQuery, IncomingTx, IncomingTxBatch, Receipt, TxData,
};

use zksync_api_client::rest::forced_exit_requests::ConfigInfo;
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;
use zksync_types::{
    forced_exit_requests::{
        ForcedExitEligibilityResponse, ForcedExitRequest, ForcedExitRequestId,
        SaveForcedExitRequestQuery,
    },
    Address, TokenLike,
};

// Local uses
use crate::api_server::rest::v1::{Error as ApiError, JsonResult};

use crate::api_server::forced_exit_checker::ForcedExitAccountAgeChecker;

/// Shared data between `/api/forced_exit_requests/v0.1/` endpoints.
pub struct ApiForcedExitRequestsData {
    pub(crate) connection_pool: ConnectionPool,
    pub(crate) forced_exit_checker: Box<dyn ForcedExitAccountAgeChecker>,

    pub(crate) is_enabled: bool,
    pub(crate) max_tokens_per_request: u8,
    pub(crate) digits_in_id: u8,
    pub(crate) recomended_tx_interval_millisecs: i64,
    pub(crate) max_tx_interval_millisecs: i64,
    pub(crate) price_per_token: i64,
    pub(crate) forced_exit_contract_address: Address,
    pub(crate) wait_confirmations: u64,
}

impl ApiForcedExitRequestsData {
    fn new(
        connection_pool: ConnectionPool,
        config: &ZkSyncConfig,
        forced_exit_checker: Box<dyn ForcedExitAccountAgeChecker>,
    ) -> Self {
        Self {
            connection_pool,
            forced_exit_checker,

            is_enabled: config.forced_exit_requests.enabled,
            price_per_token: config.forced_exit_requests.price_per_token,
            max_tokens_per_request: config.forced_exit_requests.max_tokens_per_request,
            recomended_tx_interval_millisecs: config.forced_exit_requests.recomended_tx_interval,
            max_tx_interval_millisecs: config.forced_exit_requests.max_tx_interval,
            forced_exit_contract_address: config.contracts.forced_exit_addr,
            digits_in_id: config.forced_exit_requests.digits_in_id,
            wait_confirmations: config.forced_exit_requests.wait_confirmations,
        }
    }
}

async fn get_status(
    data: web::Data<ApiForcedExitRequestsData>,
) -> JsonResult<ForcedExitRequestStatus> {
    let start = Instant::now();

    let response = if data.is_enabled {
        ForcedExitRequestStatus::Enabled(ConfigInfo {
            request_fee: BigUint::from(data.price_per_token as u64),
            max_tokens_per_request: data.max_tokens_per_request,
            recomended_tx_interval_millis: data.recomended_tx_interval_millisecs,
            forced_exit_contract_address: data.forced_exit_contract_address,
            wait_confirmations: data.wait_confirmations,
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

    let mut storage = data
        .connection_pool
        .access_storage()
        .await
        .map_err(warn_err)
        .map_err(ApiError::internal)?;

    if params.tokens.len() > data.max_tokens_per_request as usize {
        return Err(ApiError::bad_request(
            "Maximum number of tokens per ForcedExit request exceeded",
        ));
    }

    data.forced_exit_checker
        .validate_forced_exit(&mut storage, params.target)
        .await
        .map_err(ApiError::from)?;

    let price_of_one_exit = BigDecimal::from(data.price_per_token);
    let price_of_request = price_of_one_exit * BigDecimal::from_usize(params.tokens.len()).unwrap();

    let user_fee = params.price_in_wei.to_bigint().unwrap();
    let user_fee = BigDecimal::from(user_fee);

    if user_fee != price_of_request {
        return Err(ApiError::bad_request(
            "The amount should be exactly the price of the supplied withdrawals",
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
            .map_err(|_| ApiError::bad_request("One of the tokens does no exist"))?;
    }

    let mut fe_schema = storage.forced_exit_requests_schema();

    let created_at = Utc::now();
    let valid_until = created_at.add(Duration::milliseconds(data.max_tx_interval_millisecs));

    let saved_fe_request = fe_schema
        .store_request(SaveForcedExitRequestQuery {
            target: params.target,
            tokens: params.tokens.clone(),
            price_in_wei: params.price_in_wei.clone(),
            created_at,
            valid_until,
        })
        .await
        .map_err(|_| ApiError::internal(""))?;

    check_address_space_overflow(saved_fe_request.id, data.digits_in_id);

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

    let mut storage = data
        .connection_pool
        .access_storage()
        .await
        .map_err(warn_err)
        .map_err(ApiError::internal)?;

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

// Checks if the account is eligible for forced_exit in terms of
// existing enough time
pub async fn check_account_eligibility(
    data: web::Data<ApiForcedExitRequestsData>,
    web::Path(account): web::Path<Address>,
) -> JsonResult<ForcedExitEligibilityResponse> {
    let mut storage = data
        .connection_pool
        .access_storage()
        .await
        .map_err(warn_err)
        .map_err(ApiError::internal)?;

    let eligible = data
        .forced_exit_checker
        .check_forced_exit(&mut storage, account)
        .await
        .map_err(ApiError::from)?;

    let result = ForcedExitEligibilityResponse { eligible };

    Ok(Json(result))
}

pub fn api_scope(
    connection_pool: ConnectionPool,
    config: &ZkSyncConfig,
    fe_checker: Box<dyn ForcedExitAccountAgeChecker>,
) -> Scope {
    let data = ApiForcedExitRequestsData::new(connection_pool, config, fe_checker);

    // `enabled` endpoint should always be there
    let scope = web::scope("v0.1")
        .data(data)
        .route("status", web::get().to(get_status));

    if config.forced_exit_requests.enabled {
        scope
            .route("/submit", web::post().to(submit_request))
            .route("/requests/{id}", web::get().to(get_request_by_id))
            .route(
                "/checks/eligibility/{account}",
                web::get().to(check_account_eligibility),
            )
    } else {
        scope
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Mul;
    use std::str::FromStr;

    use num::BigUint;

    use zksync_api_client::rest::v1::Client;
    use zksync_config::ForcedExitRequestsConfig;
    use zksync_storage::ConnectionPool;
    use zksync_types::{Address, TokenId};

    use super::*;
    use crate::api_server::forced_exit_checker::DummyForcedExitChecker;
    use crate::api_server::v1::test_utils::TestServerConfig;

    struct TestServer {
        api_server: actix_web::test::TestServer,
        #[allow(dead_code)]
        pool: ConnectionPool,
    }

    impl TestServer {
        async fn from_config(cfg: TestServerConfig) -> anyhow::Result<(Client, Self)> {
            let pool = cfg.pool.clone();

            let (api_client, api_server) =
                cfg.start_server_with_scope(String::from("api/forced_exit_requests"), move |cfg| {
                    api_scope(
                        cfg.pool.clone(),
                        &cfg.config,
                        Box::new(DummyForcedExitChecker {}),
                    )
                });

            Ok((api_client, Self { api_server, pool }))
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

        let (client, server) = TestServer::from_config(test_config).await?;

        let status = client.get_forced_exit_requests_status().await?;

        assert_eq!(status, ForcedExitRequestStatus::Disabled);

        let register_request = ForcedExitRegisterRequest {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![TokenId(0)],
            price_in_wei: BigUint::from_str("1212").unwrap(),
        };

        client
            .submit_forced_exit_request(register_request)
            .await
            .expect_err("Forced-exit related requests don't fail when it's disabled");

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

        let (client, server) = TestServer::from_config(test_config).await?;

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
    async fn test_forced_exit_requests_wrong_tokens_number() -> anyhow::Result<()> {
        let forced_exit_requests_config = ForcedExitRequestsConfig::from_env();
        let test_config = get_test_config_from_forced_exit_requests(ForcedExitRequestsConfig {
            max_tokens_per_request: 5,
            ..forced_exit_requests_config
        });

        let (client, server) = TestServer::from_config(test_config).await?;

        let status = client.get_forced_exit_requests_status().await?;
        assert_ne!(status, ForcedExitRequestStatus::Disabled);

        let price_per_token = forced_exit_requests_config.price_per_token;
        // 6 tokens:
        let tokens: Vec<u32> = vec![0, 1, 2, 3, 4, 5];
        let tokens: Vec<TokenId> = tokens.iter().map(|t| TokenId(*t)).collect();
        let price_in_wei = BigUint::from_i64(price_per_token)
            .unwrap()
            .mul(tokens.len());

        let register_request = ForcedExitRegisterRequest {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens,
            price_in_wei,
        };

        client
            .submit_forced_exit_request(register_request)
            .await
            .expect_err("Api does not take the limit on the number of tokens into account");

        server.stop().await;
        Ok(())
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn test_forced_exit_requests_submit() -> anyhow::Result<()> {
        let price_per_token: i64 = 1000000000000000000;
        let max_tokens_per_request = 3;
        let server_config = get_test_config_from_forced_exit_requests(ForcedExitRequestsConfig {
            max_tokens_per_request,
            price_per_token,
            ..ForcedExitRequestsConfig::from_env()
        });

        let (client, server) = TestServer::from_config(server_config).await?;

        let status = client.get_forced_exit_requests_status().await?;
        assert!(matches!(status, ForcedExitRequestStatus::Enabled(_)));

        let tokens: Vec<u32> = vec![0, 1, 2];
        let tokens: Vec<TokenId> = tokens.iter().map(|t| TokenId(*t)).collect();

        let price_in_wei = BigUint::from_i64(price_per_token)
            .unwrap()
            .mul(tokens.len());

        let target = Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap();

        let fe_request = ForcedExitRegisterRequest {
            target,
            tokens: tokens.clone(),
            price_in_wei: price_in_wei.clone(),
        };

        let submit_result = client.submit_forced_exit_request(fe_request).await?;

        assert_eq!(submit_result.price_in_wei, price_in_wei);
        assert_eq!(submit_result.tokens, tokens);
        assert_eq!(submit_result.target, target);

        server.stop().await;
        Ok(())
    }
}

fn warn_err<T: std::fmt::Display>(err: T) -> T {
    vlog::warn!("Internal Server Error: '{}';", err);
    err
}

// Checks if the id exceeds half of the address space
// If it exceeds the half at all the alert should be triggerred
// since it it a sign of a possible DoS attack
pub fn check_address_space_overflow(id: i64, digits_in_id: u8) {
    let address_space = 10_i64.saturating_pow(digits_in_id as u32);

    let exceeding_rate = id.saturating_sub(address_space / 2);
    // Need this for metrics
    let exceeding_rate: u64 = exceeding_rate.max(0).try_into().unwrap();

    metrics::histogram!(
        "forced_exit_requests.address_space_overflow",
        exceeding_rate
    );
}
