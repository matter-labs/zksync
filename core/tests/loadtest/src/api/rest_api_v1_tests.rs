//! Tests for the relevant API methods declared in the
//! `core/bin/zksync_api/src/api_server/rest/v1` module.

// Built-in uses
use std::str::FromStr;

// External uses
use futures::prelude::*;
use rand::{thread_rng, Rng};

// Workspace uses
use zksync_api_client::rest::v1::{
    accounts::{AccountQuery, AccountReceipts},
    Client, TokenPriceKind, MAX_LIMIT,
};
use zksync_config::test_config::TestConfig;
use zksync_types::{Address, TokenId, TokenLike};

// Local uses
use super::{ApiDataPool, ApiTestsBuilder};
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

async fn random_account_query(pool: &ApiDataPool) -> AccountQuery {
    let (address, account_id);
    // We should only use accounts with the settled account ID.
    let mut attempts: u32 = 0;
    loop {
        let inner = pool.read().await;
        let data = inner.random_address();
        if let Some(id) = data.1.account_id {
            address = data.0;
            account_id = id;
            break;
        }

        attempts += 1;
        if attempts >= MAX_LIMIT {
            unreachable!(
                "Unable to find the appropriate account {} attempts.",
                MAX_LIMIT
            );
        }
    }

    if thread_rng().gen::<bool>() {
        AccountQuery::Id(account_id)
    } else {
        AccountQuery::Address(address)
    }
}

async fn random_account_receipts_query(pool: &ApiDataPool) -> AccountReceipts {
    let location = pool.read().await.random_tx_location();
    match thread_rng().gen_range(0, 3) {
        0 => AccountReceipts::older_than(location.0, Some(location.1 as u32)),
        1 => AccountReceipts::newer_than(location.0, Some(location.1 as u32)),
        2 => AccountReceipts::Latest,
        _ => unreachable!(),
    }
}

pub fn wire_tests<'a>(builder: ApiTestsBuilder<'a>, monitor: &'a Monitor) -> ApiTestsBuilder<'a> {
    let builder = RestApiTestsBuilder::new(builder, monitor);

    // Prebuilt token-like requests
    let tokens = [
        // Ethereum.
        TokenLike::Id(TokenId(0)),
        TokenLike::Symbol("ETH".to_string()),
        TokenLike::Address(Address::default()),
        // PHNX, see rest/v1/test_utils.rs
        TokenLike::Id(TokenId(1)),
        TokenLike::Symbol("PHNX".to_string()),
        TokenLike::Address(Address::from_str("38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7").unwrap()),
    ];

    builder
        // accounts endpoints.
        .append("accounts/info", |client, monitor| async move {
            let address = random_account_query(&monitor.api_data_pool).await;
            client.account_info(address).await?;
            Ok(())
        })
        .append(
            "accounts/transactions/receipts",
            |client, monitor| async move {
                let address = random_account_query(&monitor.api_data_pool).await;
                let receipts = random_account_receipts_query(&monitor.api_data_pool).await;
                client
                    .account_tx_receipts(address, receipts, MAX_LIMIT)
                    .await?;
                Ok(())
            },
        )
        .append(
            "accounts/operations/receipts",
            |client, monitor| async move {
                let address = random_account_query(&monitor.api_data_pool).await;
                let receipts = random_account_receipts_query(&monitor.api_data_pool).await;
                client
                    .account_op_receipts(address, receipts, MAX_LIMIT)
                    .await?;
                Ok(())
            },
        )
        .append(
            "accounts/operations/pending_receipts",
            |client, monitor| async move {
                let address = random_account_query(&monitor.api_data_pool).await;
                client.account_pending_ops(address).await?;
                Ok(())
            },
        )
        // blocks endpoints.
        .append("blocks/info", |client, monitor| async move {
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
        // operations endpoints.
        .append(
            "operations/receipt/by_serial_id",
            |client, monitor| async move {
                let op = monitor.api_data_pool.read().await.random_priority_op();
                client.priority_op(op.serial_id).await?;
                Ok(())
            },
        )
        .append(
            "operations/receipt/eth_hash",
            |client, monitor| async move {
                let op = monitor.api_data_pool.read().await.random_priority_op();
                client.priority_op(op.eth_hash).await?;
                Ok(())
            },
        )
        .append(
            "operations/data/by_serial_id",
            |client, monitor| async move {
                let op = monitor.api_data_pool.read().await.random_priority_op();
                client.priority_op_data(op.serial_id).await?;
                Ok(())
            },
        )
        .append("operations/data/eth_hash", |client, monitor| async move {
            let op = monitor.api_data_pool.read().await.random_priority_op();
            client.priority_op_data(op.eth_hash).await?;
            Ok(())
        })
        // search endpoints.
        .append("search", |client, monitor| async move {
            let block_id = monitor.api_data_pool.read().await.random_block();
            client.search_block(block_id).await?;
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
        // transactions enpoints.
        .append("transactions/status", move |client, monitor| async move {
            let tx_hash = monitor.api_data_pool.read().await.random_tx_hash();
            client.tx_status(tx_hash).await?;
            Ok(())
        })
        .append("transactions/data", move |client, monitor| async move {
            let tx_hash = monitor.api_data_pool.read().await.random_tx_hash();
            client.tx_data(tx_hash).await?;
            Ok(())
        })
        .append("transactions/receipts", move |client, monitor| async move {
            let tx_hash = monitor.api_data_pool.read().await.random_tx_hash();
            let range = monitor.api_data_pool.read().await.random_block_range().0;
            client.tx_receipts(tx_hash, range, MAX_LIMIT).await?;
            Ok(())
        })
        .append("transactions/receipt", move |client, monitor| async move {
            let tx_hash = monitor.api_data_pool.read().await.random_tx_hash();
            let receipt_id = thread_rng().gen_range(0, MAX_LIMIT);
            client.tx_receipt_by_id(tx_hash, receipt_id).await?;
            Ok(())
        })
        .into_inner()
}
