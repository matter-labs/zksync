//! Tests for the relevant API methods declared in the
//! `core/bin/zksync_api/src/api_server/rest/v1` module.

// Built-in uses
use std::str::FromStr;

// External uses

// Workspace uses
use futures::prelude::*;
use rand::{thread_rng, Rng};
use zksync_api::client::{Client, TokenPriceKind};
use zksync_config::test_config::TestConfig;
use zksync_types::{Address, TokenLike};

// Local uses
use super::ApiTestsBuilder;
use crate::monitor::Monitor;

struct RestApiTestsBuilder<'a> {
    inner: ApiTestsBuilder<'a>,
    monitor: &'a Monitor,
    client: Client,
}

impl<'a> RestApiTestsBuilder<'a> {
    fn new(inner: ApiTestsBuilder<'a>, monitor: &'a Monitor) -> Self {
        let rest_api_url = TestConfig::load().api.rest_api_url;
        let client = Client::new(rest_api_url);

        Self {
            inner,
            monitor,
            client,
        }
    }

    fn append<F, Fut>(self, category: &str, factory: F) -> Self
    where
        F: Fn(Client, &'a Monitor) -> Fut + Send + 'a,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'a,
    {
        let monitor = self.monitor;
        let client = self.client.clone();

        let category = format!("rest/v1/{}", category);
        let inner = self
            .inner
            .append(&category, move || factory(client.clone(), monitor));

        Self {
            inner,
            monitor,
            client: self.client,
        }
    }

    fn into_inner(self) -> ApiTestsBuilder<'a> {
        self.inner
    }
}

fn random_token(tokens: &[TokenLike]) -> TokenLike {
    let mut rng = thread_rng();

    let index = rng.gen_range(0, tokens.len());
    tokens[index].clone()
}

pub fn wire_tests<'a>(builder: ApiTestsBuilder<'a>, monitor: &'a Monitor) -> ApiTestsBuilder<'a> {
    let builder = RestApiTestsBuilder::new(builder, monitor);

    // Prebuilt token-like requests
    let tokens = [
        // Ethereum.
        TokenLike::Id(0),
        TokenLike::Symbol("ETH".to_string()),
        TokenLike::Address(Address::default()),
        // PHNX, see rest/v1/test_utils.rs
        TokenLike::Id(1),
        TokenLike::Symbol("PHNX".to_string()),
        TokenLike::Address(Address::from_str("38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7").unwrap()),
    ];

    builder
        // blocks endpoints.
        .append("blocks/by_id", |client, monitor| async move {
            let block_number = monitor.api_data_pool.read().await.random_block();
            client.block_by_id(block_number).await?;
            Ok(())
        })
        .append("blocks/range", |client, monitor| async move {
            let (pagination, limit) = monitor.api_data_pool.read().await.random_block_range();
            client.blocks_range(pagination, limit).await?;
            Ok(())
        })
        .append("blocks/transactions", |client, monitor| async move {
            let block_number = monitor.api_data_pool.read().await.random_block();
            client.block_transactions(block_number).await?;
            Ok(())
        })
        // config endpoints.
        .append("config/contracts", |client, _monitor| async move {
            client.contracts().await?;
            Ok(())
        })
        .append(
            "config/deposit_confirmations",
            |client, _monitor| async move {
                client.deposit_confirmations().await?;
                Ok(())
            },
        )
        .append("config/network", |client, _monitor| async move {
            client.network().await?;
            Ok(())
        })
        // tokens endpoints.
        .append("tokens/list", |client, _monitor| async move {
            client.tokens().await?;
            Ok(())
        })
        .append("tokens/by_id", {
            let tokens = tokens.clone();
            move |client, _monitor| {
                let tokens = tokens.clone();
                async move {
                    let token = random_token(&tokens);
                    client.token_by_id(&token).await?;
                    Ok(())
                }
            }
        })
        .append("tokens/price", move |client, _monitor| {
            let tokens = tokens.clone();
            async move {
                let token = random_token(&tokens);
                client.token_price(&token, TokenPriceKind::Currency).await?;
                Ok(())
            }
        })
        // Transactions enpoints.
        .into_inner()
}
