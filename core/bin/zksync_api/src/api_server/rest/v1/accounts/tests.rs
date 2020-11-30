// Built-in uses
use std::sync::Arc;

// External uses
use actix_web::{
    web::{self, Json},
    App,
};
use serde_json::json;
use tokio::sync::Mutex;

// Workspace uses
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::{Address, H256};

// Local uses
use crate::{
    api_server::v1::{
        client::{Client, TxReceipt},
        test_utils::TestServerConfig,
    },
    core_api_client::CoreApiClient,
    utils::token_db_cache::TokenDBCache,
};

use super::{api_scope, types::AccountReceipts};

type DepositsHandle = Arc<Mutex<serde_json::Value>>;

fn get_unconfirmed_deposits_loopback(
    handle: DepositsHandle,
) -> (CoreApiClient, actix_web::test::TestServer) {
    async fn get_unconfirmed_deposits(
        data: web::Data<DepositsHandle>,
        _path: web::Path<String>,
    ) -> Json<serde_json::Value> {
        Json(data.lock().await.clone())
    };

    let server = actix_web::test::start(move || {
        let handle = handle.clone();
        App::new().data(handle).route(
            "unconfirmed_deposits/{address}",
            web::get().to(get_unconfirmed_deposits),
        )
    });

    let mut url = server.url("");
    url.pop(); // Pop last '/' symbol.

    (CoreApiClient::new(url), server)
}

struct TestServer {
    core_server: actix_web::test::TestServer,
    api_server: actix_web::test::TestServer,
    pool: ConnectionPool,
    deposits: DepositsHandle,
}

impl TestServer {
    async fn new() -> anyhow::Result<(Client, Self)> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let deposits = DepositsHandle::new(Mutex::new(json!([])));
        let (core_client, core_server) = get_unconfirmed_deposits_loopback(deposits.clone());

        let pool = cfg.pool.clone();

        let (api_client, api_server) = cfg.start_server(move |cfg| {
            api_scope(
                &cfg.env_options,
                TokenDBCache::new(cfg.pool.clone()),
                core_client.clone(),
            )
        });

        Ok((
            api_client,
            Self {
                core_server,
                api_server,
                pool,
                deposits,
            },
        ))
    }

    async fn account_address(storage: &mut StorageProcessor<'_>) -> anyhow::Result<Address> {
        let transactions = storage
            .chain()
            .block_schema()
            .get_block_transactions(1)
            .await?;

        let tx = &transactions[0];
        let op = tx.op.as_object().unwrap();

        let address = serde_json::from_value(op["to"].clone()).unwrap();
        Ok(address)
    }

    async fn stop(self) {
        self.api_server.stop().await;
        self.core_server.stop().await;
    }
}

#[actix_rt::test]
async fn test_get_unconfirmed_deposits_loopback() -> anyhow::Result<()> {
    let (client, server) =
        get_unconfirmed_deposits_loopback(DepositsHandle::new(Mutex::new(json!([]))));

    client.get_unconfirmed_deposits(Address::default()).await?;

    server.stop().await;
    Ok(())
}

#[actix_rt::test]
async fn test_accounts_scope() -> anyhow::Result<()> {
    let (client, server) = TestServer::new().await?;

    // Get account information.
    let address = TestServer::account_address(&mut server.pool.access_storage().await?).await?;

    let account_info = client.account_info(address).await?.unwrap();
    let id = account_info.id;
    assert_eq!(client.account_info(id).await?, Some(account_info));

    // Provide unconfirmed deposits
    let deposits = json!([
        [
            5,
            {
                "serial_id": 1,
                "data": {
                    "type": "Deposit",
                    "account_id": id,
                    "amount": "100500",
                    "from": Address::default(),
                    "to": address,
                    "token": 0,
                },
                "deadline_block": 10,
                "eth_hash": H256::default().as_ref().to_vec(),
                "eth_block": 5,
            },
        ]
    ]);
    *server.deposits.lock().await = deposits;

    // Check account information about unconfirmed deposits.
    let account_info = client.account_info(id).await?.unwrap();

    let depositing_balances = &account_info.depositing.balances["ETH"];
    assert_eq!(depositing_balances.expected_accept_block, 5);
    assert_eq!(depositing_balances.amount.0, 100_500_u64.into());

    // Get account tx receipts.
    let receipts = client
        .account_receipts(address, AccountReceipts::newer_than(0, 0), 100)
        .await?;
    assert_eq!(receipts[0].location.block, 1);
    assert_eq!(receipts[0].location.index, None);
    assert_eq!(receipts[0].receipt, TxReceipt::Verified { block: 1 });

    // Get account pending receipts.
    let pending_receipts = client.account_pending_receipts(id).await?;
    assert_eq!(pending_receipts[0].block, 5);
    assert_eq!(pending_receipts[0].hash, H256::default());

    server.stop().await;
    Ok(())
}
