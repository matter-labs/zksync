//! Fee part of API implementation.

// Built-in uses

use std::time::Instant;
// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_api_types::v02::fee::{ApiFee, BatchFeeRequest, TxFeeRequest};

// Local uses
use super::{error::Error, response::ApiResult};
use crate::{
    api_server::tx_sender::{SubmitError, TxSender},
    api_try,
};

/// Shared data between `api/v0.2/fee` endpoints.
#[derive(Clone)]
struct ApiFeeData {
    tx_sender: TxSender,
}

impl ApiFeeData {
    fn new(tx_sender: TxSender) -> Self {
        Self { tx_sender }
    }
}

async fn get_tx_fee(
    data: web::Data<ApiFeeData>,
    Json(body): Json<TxFeeRequest>,
) -> ApiResult<ApiFee> {
    let start = Instant::now();
    let token_allowed = api_try!(data
        .tx_sender
        .ticker
        .token_allowed_for_fees(body.token_like.clone())
        .await
        .map_err(Error::from));
    if !token_allowed {
        return Error::from(SubmitError::InappropriateFeeToken).into();
    }
    // TODO implement subsidies for v02 api ZKS-888
    let res = data
        .tx_sender
        .ticker
        .get_fee_from_ticker_in_wei(body.tx_type.into(), body.token_like, body.address)
        .await
        .map(|fee| fee.normal_fee.into())
        .map_err(Error::from)
        .into();
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "get_tx_fee");
    res
}

async fn get_batch_fee(
    data: web::Data<ApiFeeData>,
    Json(body): Json<BatchFeeRequest>,
) -> ApiResult<ApiFee> {
    let start = Instant::now();
    let token_allowed = api_try!(data
        .tx_sender
        .ticker
        .token_allowed_for_fees(body.token_like.clone())
        .await
        .map_err(Error::from));
    if !token_allowed {
        return Error::from(SubmitError::InappropriateFeeToken).into();
    }
    let txs = body
        .transactions
        .into_iter()
        .map(|tx| (tx.tx_type.into(), tx.address))
        .collect();
    let res = data
        .tx_sender
        .ticker
        .get_batch_from_ticker_in_wei(body.token_like, txs)
        .await
        .map(|fee| fee.normal_fee.into())
        .map_err(Error::from)
        .into();
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "get_batch_fee");
    res
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiFeeData::new(tx_sender);

    web::scope("fee")
        .app_data(web::Data::new(data))
        .route("", web::post().to(get_tx_fee))
        .route("/batch", web::post().to(get_batch_fee))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_server::rest::v02::{
        test_utils::{
            deserialize_response_result, dummy_fee_ticker, dummy_sign_verifier, TestServerConfig,
        },
        SharedData,
    };
    use crate::fee_ticker::validator::cache::TokenInMemoryCache;
    use chrono::Utc;
    use futures::channel::mpsc;
    use num::rational::Ratio;
    use num::BigUint;
    use std::collections::HashMap;
    use zksync_api_types::v02::{
        fee::{ApiTxFeeTypes, TxInBatchFeeRequest},
        ApiVersion,
    };
    use zksync_types::{
        tokens::{TokenLike, TokenMarketVolume},
        Address, ChainId, Token, TokenId, TokenKind,
    };

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn fee_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();

        let (mempool_tx_request_sender, _mempool_tx_request_receiver) = mpsc::channel(100);

        let shared_data = SharedData {
            net: cfg.config.chain.eth.network,
            api_version: ApiVersion::V02,
        };

        let mut tokens = HashMap::new();
        tokens.insert(
            TokenLike::Id(TokenId(1)),
            Token::new(TokenId(1), Default::default(), "", 18, TokenKind::ERC20),
        );
        tokens.insert(
            TokenLike::Id(TokenId(2)),
            Token::new(TokenId(2), Default::default(), "", 18, TokenKind::ERC20),
        );
        let mut market = HashMap::new();
        market.insert(
            TokenId(2),
            TokenMarketVolume {
                market_volume: Ratio::from_integer(BigUint::from(400u32)),
                last_updated: Utc::now(),
            },
        );
        let prices = vec![
            (TokenLike::Id(TokenId(0)), 10_u64.into()),
            (TokenLike::Id(TokenId(1)), 10_u64.into()),
            (TokenLike::Id(TokenId(2)), 10000_u64.into()),
        ];

        let cache = TokenInMemoryCache::new()
            .with_tokens(tokens)
            .with_market(market);
        let (client, server) = cfg.start_server(
            move |cfg: &TestServerConfig| {
                api_scope(TxSender::new(
                    cfg.pool.clone(),
                    dummy_sign_verifier(),
                    dummy_fee_ticker(&prices, Some(cache.clone())),
                    &cfg.config.api.common,
                    &cfg.config.api.token_config,
                    mempool_tx_request_sender.clone(),
                    ChainId(cfg.config.eth_client.chain_id),
                ))
            },
            Some(shared_data),
        );

        let tx_type = ApiTxFeeTypes::Withdraw;
        let address = Address::default();
        let not_allowed_token = TokenLike::Id(TokenId(1));

        let response = client
            .get_txs_fee(tx_type.clone(), address, not_allowed_token)
            .await?;
        let expected_error = Error::from(SubmitError::InappropriateFeeToken);
        let error = serde_json::from_value::<Error>(response.error.unwrap()).unwrap();
        assert_eq!(error, expected_error);

        let allowed_token = TokenLike::Id(TokenId(2));

        let response = client
            .get_txs_fee(tx_type, address, allowed_token.clone())
            .await?;
        let api_fee: ApiFee = deserialize_response_result(response)?;
        assert_eq!(api_fee.gas_fee, BigUint::from(1u32));
        assert_eq!(api_fee.zkp_fee, BigUint::from(1u32));
        assert_eq!(api_fee.total_fee, BigUint::from(2u32));

        let tx = TxInBatchFeeRequest {
            tx_type: ApiTxFeeTypes::Withdraw,
            address: Address::default(),
        };
        let txs = vec![tx.clone(), tx.clone(), tx];

        let response = client.get_batch_fee(txs, allowed_token).await?;
        let api_batch_fee: ApiFee = deserialize_response_result(response)?;
        assert_eq!(api_batch_fee.gas_fee, BigUint::from(1u32));
        assert_eq!(api_batch_fee.zkp_fee, BigUint::from(1u32));
        assert_eq!(api_batch_fee.total_fee, BigUint::from(2u32));

        server.stop().await;
        Ok(())
    }
}
