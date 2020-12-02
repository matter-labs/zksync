//! Tests for the relevant API methods declared in the
//! `core/bin/zksync_api/src/api_server/rest/v1` module.

// Built-in uses

// External uses

// Workspace uses
use futures::prelude::*;
use zksync_api::client::Client;
use zksync_config::test_config::TestConfig;

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

pub fn wire_tests<'a>(builder: ApiTestsBuilder<'a>, monitor: &'a Monitor) -> ApiTestsBuilder<'a> {
    let builder = RestApiTestsBuilder::new(builder, monitor);

    builder
        // blocks endpoints.
        .append("blocks", |client, monitor| async move {
            let block_number = monitor.api_data_pool.read().await.random_block();
            client.block_by_id(block_number).await?;
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
        .into_inner()
}
