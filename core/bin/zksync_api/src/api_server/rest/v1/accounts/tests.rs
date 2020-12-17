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
use zksync_storage::{
    chain::operations_ext::records::AccountTxReceiptResponse, ConnectionPool, StorageProcessor,
};
use zksync_types::{tx::TxHash, Address, BlockNumber, H256};

// Local uses
use crate::{
    api_server::v1::{
        client::{Client, TxReceipt},
        test_utils::TestServerConfig,
    },
    core_api_client::CoreApiClient,
    utils::token_db_cache::TokenDBCache,
};

use super::{
    api_scope,
    types::{AccountReceipts, AccountTxReceipt},
};

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

    let url = server.url("").trim_end_matches('/').to_owned();

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
                cfg.pool.clone(),
                TokenDBCache::new(),
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

    async fn account_address(
        storage: &mut StorageProcessor<'_>,
        block: BlockNumber,
    ) -> anyhow::Result<Address> {
        let transactions = storage
            .chain()
            .block_schema()
            .get_block_transactions(block)
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
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn unconfirmed_deposits_loopback() -> anyhow::Result<()> {
    let (client, server) =
        get_unconfirmed_deposits_loopback(DepositsHandle::new(Mutex::new(json!([]))));

    client.get_unconfirmed_deposits(Address::default()).await?;

    server.stop().await;
    Ok(())
}

#[actix_rt::test]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn accounts_scope() -> anyhow::Result<()> {
    let (client, server) = TestServer::new().await?;

    // Get account information.
    let address = TestServer::account_address(&mut server.pool.access_storage().await?, 1).await?;

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
        .account_receipts(address, AccountReceipts::newer_than(1, 0), 10)
        .await?;

    assert_eq!(receipts[0].index, Some(2));
    assert_eq!(receipts[0].receipt, TxReceipt::Verified { block: 1 });

    // Get same receipts by the different requests.
    assert_eq!(
        client
            .account_receipts(address, AccountReceipts::Latest, 10)
            .await?,
        receipts
    );
    assert_eq!(
        client
            .account_receipts(address, AccountReceipts::older_than(10, 0), 10)
            .await?,
        receipts
    );

    // Get account pending receipts.
    let pending_receipts = client.account_pending_receipts(id).await?;
    assert_eq!(pending_receipts[0].block, 5);
    assert_eq!(pending_receipts[0].hash, H256::default());

    server.stop().await;
    Ok(())
}

#[test]
fn account_tx_response_to_receipt() {
    fn empty_hash() -> Vec<u8> {
        TxHash::default().as_ref().to_vec()
    }

    let cases = vec![
        (
            AccountTxReceiptResponse {
                block_index: Some(1),
                block_number: 1,
                success: true,
                fail_reason: None,
                commit_tx_hash: None,
                verify_tx_hash: None,
                tx_hash: empty_hash(),
            },
            AccountTxReceipt {
                index: Some(1),
                hash: TxHash::default(),
                receipt: TxReceipt::Executed,
            },
        ),
        (
            AccountTxReceiptResponse {
                block_index: None,
                block_number: 1,
                success: true,
                fail_reason: None,
                commit_tx_hash: None,
                verify_tx_hash: None,
                tx_hash: empty_hash(),
            },
            AccountTxReceipt {
                index: None,
                hash: TxHash::default(),
                receipt: TxReceipt::Executed,
            },
        ),
        (
            AccountTxReceiptResponse {
                block_index: Some(1),
                block_number: 1,
                success: false,
                fail_reason: Some("Oops".to_string()),
                commit_tx_hash: None,
                verify_tx_hash: None,
                tx_hash: empty_hash(),
            },
            AccountTxReceipt {
                index: Some(1),
                hash: TxHash::default(),
                receipt: TxReceipt::Rejected {
                    reason: Some("Oops".to_string()),
                },
            },
        ),
        (
            AccountTxReceiptResponse {
                block_index: Some(1),
                block_number: 1,
                success: true,
                fail_reason: None,
                commit_tx_hash: Some(empty_hash()),
                verify_tx_hash: None,
                tx_hash: empty_hash(),
            },
            AccountTxReceipt {
                index: Some(1),
                hash: TxHash::default(),
                receipt: TxReceipt::Committed { block: 1 },
            },
        ),
        (
            AccountTxReceiptResponse {
                block_index: Some(1),
                block_number: 1,
                success: true,
                fail_reason: None,
                commit_tx_hash: Some(empty_hash()),
                verify_tx_hash: Some(empty_hash()),
                tx_hash: empty_hash(),
            },
            AccountTxReceipt {
                index: Some(1),
                hash: TxHash::default(),
                receipt: TxReceipt::Verified { block: 1 },
            },
        ),
    ];

    for (resp, expected_receipt) in cases {
        let actual_receipt = AccountTxReceipt::from(resp);
        assert_eq!(actual_receipt, expected_receipt);
    }
}
