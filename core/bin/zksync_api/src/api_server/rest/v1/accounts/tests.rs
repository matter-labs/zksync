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
    chain::operations_ext::records::{AccountOpReceiptResponse, AccountTxReceiptResponse},
    ConnectionPool, StorageProcessor,
};
use zksync_types::{tx::TxHash, AccountId, Address, BlockNumber, ExecutedOperations, H256};

// Local uses
use crate::{
    api_server::v1::{
        test_utils::{dummy_deposit_op, TestServerConfig},
        transactions::Receipt,
        Client,
    },
    core_api_client::CoreApiClient,
    utils::token_db_cache::TokenDBCache,
};

use super::{
    api_scope,
    types::{
        convert::{op_receipt_from_response, tx_receipt_from_response},
        AccountOpReceipt, AccountReceipts, AccountTxReceipt,
    },
};

type PendingOpsHandle = Arc<Mutex<serde_json::Value>>;

fn create_pending_ops_handle() -> PendingOpsHandle {
    Arc::new(Mutex::new(json!([])))
}

fn get_unconfirmed_ops_loopback(
    ops_handle: PendingOpsHandle,
    deposits_handle: PendingOpsHandle,
) -> (CoreApiClient, actix_web::test::TestServer) {
    async fn get_ops(
        data: web::Data<PendingOpsHandle>,
        _path: web::Path<String>,
    ) -> Json<serde_json::Value> {
        Json(data.lock().await.clone())
    }

    let server = actix_web::test::start(move || {
        let ops_handle = ops_handle.clone();
        let deposits_handle = deposits_handle.clone();
        App::new()
            .service(
                web::scope("unconfirmed_ops")
                    .data(ops_handle)
                    .route("{address}", web::get().to(get_ops)),
            )
            .service(
                web::scope("unconfirmed_deposits")
                    .data(deposits_handle)
                    .route("{address}", web::get().to(get_ops)),
            )
    });

    let url = server.url("").trim_end_matches('/').to_owned();
    (CoreApiClient::new(url), server)
}

struct TestServer {
    core_server: actix_web::test::TestServer,
    api_server: actix_web::test::TestServer,
    pool: ConnectionPool,
    pending_ops: PendingOpsHandle,
    pending_deposits: PendingOpsHandle,
}

impl TestServer {
    async fn new() -> anyhow::Result<(Client, Self)> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let pending_ops = create_pending_ops_handle();
        let pending_deposits = create_pending_ops_handle();
        let (core_client, core_server) =
            get_unconfirmed_ops_loopback(pending_ops.clone(), pending_deposits.clone());

        let pool = cfg.pool.clone();

        let (api_client, api_server) = cfg.start_server(move |cfg| {
            api_scope(
                cfg.pool.clone(),
                &cfg.config,
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
                pending_ops,
                pending_deposits,
            },
        ))
    }

    async fn account_id(
        storage: &mut StorageProcessor<'_>,
        block: BlockNumber,
    ) -> anyhow::Result<AccountId> {
        let transactions = storage
            .chain()
            .block_schema()
            .get_block_transactions(block)
            .await?;

        let tx = &transactions[1];
        let op = tx.op.as_object().unwrap();

        let id = if op.contains_key("accountId") {
            serde_json::from_value(op["accountId"].clone()).unwrap()
        } else {
            serde_json::from_value(op["creatorId"].clone()).unwrap()
        };
        Ok(id)
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
        get_unconfirmed_ops_loopback(create_pending_ops_handle(), create_pending_ops_handle());

    client.get_unconfirmed_deposits(Address::default()).await?;
    client.get_unconfirmed_ops(Address::default()).await?;

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
    let account_id =
        TestServer::account_id(&mut server.pool.access_storage().await?, BlockNumber(1)).await?;

    let account_info = client.account_info(account_id).await?.unwrap();
    let address = account_info.address;
    assert_eq!(client.account_info(address).await?, Some(account_info));

    // Provide unconfirmed pending deposits.
    *server.pending_deposits.lock().await = json!([
        {
            "serial_id": 1,
            "data": {
                "type": "Deposit",
                "account_id": account_id,
                "amount": "100500",
                "from": Address::default(),
                "to": address,
                "token": 0,
            },
            "deadline_block": 10,
            "eth_hash": vec![0u8; 32],
            "eth_block": 5,
        },
    ]);

    // Check account information about unconfirmed deposits.
    let account_info = client.account_info(account_id).await?.unwrap();

    let depositing_balances = &account_info.depositing.balances["ETH"];
    assert_eq!(*depositing_balances.expected_accept_block, 5);
    assert_eq!(depositing_balances.amount.0, 100_500_u64.into());

    // Get account transaction receipts.
    let receipts = client
        .account_tx_receipts(
            address,
            AccountReceipts::newer_than(BlockNumber(0), None),
            10,
        )
        .await?;

    assert_eq!(receipts[0].index, None);
    assert_eq!(
        receipts[0].receipt,
        Receipt::Rejected {
            reason: Some("Unknown token".to_string())
        }
    );
    assert_eq!(receipts[2].index, Some(3));
    assert_eq!(
        receipts[2].receipt,
        Receipt::Verified {
            block: BlockNumber(1)
        }
    );

    // Get a reversed list of receipts with requests from the end.
    let receipts: Vec<_> = receipts.into_iter().rev().collect();
    assert_eq!(
        client
            .account_tx_receipts(address, AccountReceipts::Latest, 10)
            .await?,
        receipts
    );
    assert_eq!(
        client
            .account_tx_receipts(
                address,
                AccountReceipts::older_than(BlockNumber(10), Some(0)),
                10
            )
            .await?,
        receipts
    );

    // Save priority operation in block.
    let deposit_op = dummy_deposit_op(address, account_id, 10234, 1);
    server
        .pool
        .access_storage()
        .await?
        .chain()
        .block_schema()
        .save_block_transactions(
            BlockNumber(1),
            vec![ExecutedOperations::PriorityOp(Box::new(deposit_op))],
        )
        .await?;

    // Get account operation receipts.
    let receipts = client
        .account_op_receipts(
            address,
            AccountReceipts::newer_than(BlockNumber(1), Some(0)),
            10,
        )
        .await?;

    assert_eq!(
        receipts[0],
        AccountOpReceipt {
            hash: H256::default(),
            index: 1,
            receipt: Receipt::Verified {
                block: BlockNumber(1)
            }
        }
    );
    assert_eq!(
        client
            .account_op_receipts(
                address,
                AccountReceipts::newer_than(BlockNumber(1), Some(0)),
                10
            )
            .await?,
        receipts
    );
    assert_eq!(
        client
            .account_op_receipts(
                address,
                AccountReceipts::older_than(BlockNumber(2), Some(0)),
                10
            )
            .await?,
        receipts
    );
    assert_eq!(
        client
            .account_op_receipts(
                account_id,
                AccountReceipts::newer_than(BlockNumber(1), Some(0)),
                10
            )
            .await?,
        receipts
    );
    assert_eq!(
        client
            .account_op_receipts(
                account_id,
                AccountReceipts::older_than(BlockNumber(2), Some(0)),
                10
            )
            .await?,
        receipts
    );

    // Get account pending receipts.
    *server.pending_ops.lock().await = json!([
        {
            "serial_id": 1,
            "data": {
                "type": "Deposit",
                "account_id": account_id,
                "amount": "100500",
                "from": Address::default(),
                "to": address,
                "token": 0,
            },
            "deadline_block": 10,
            "eth_hash": vec![0u8; 32],
            "eth_block": 5,
        },
        {
            "serial_id": 2,
            "data": {
                "type": "FullExit",
                "account_id": account_id,
                "eth_address": Address::default(),
                "token": 0
            },
            "deadline_block": 0,
            "eth_hash": vec![1u8; 32],
            "eth_block": 5
        }
    ]);
    let pending_receipts = client.account_pending_ops(account_id).await?;

    assert_eq!(pending_receipts[0].eth_block, 5);
    assert_eq!(pending_receipts[0].hash, [0u8; 32].into());
    assert_eq!(pending_receipts[1].eth_block, 5);
    assert_eq!(pending_receipts[1].hash, [1u8; 32].into());

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
                receipt: Receipt::Executed,
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
                receipt: Receipt::Executed,
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
                receipt: Receipt::Rejected {
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
                receipt: Receipt::Committed {
                    block: BlockNumber(1),
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
                verify_tx_hash: Some(empty_hash()),
                tx_hash: empty_hash(),
            },
            AccountTxReceipt {
                index: Some(1),
                hash: TxHash::default(),
                receipt: Receipt::Verified {
                    block: BlockNumber(1),
                },
            },
        ),
    ];

    for (resp, expected_receipt) in cases {
        let actual_receipt = tx_receipt_from_response(resp);
        assert_eq!(actual_receipt, expected_receipt);
    }
}

#[test]
fn account_op_response_to_receipt() {
    fn empty_hash() -> Vec<u8> {
        H256::default().as_bytes().to_vec()
    }

    let cases = vec![
        (
            AccountOpReceiptResponse {
                block_index: 1,
                block_number: 1,
                commit_tx_hash: None,
                verify_tx_hash: None,
                eth_hash: empty_hash(),
            },
            AccountOpReceipt {
                index: 1,
                hash: H256::default(),
                receipt: Receipt::Executed,
            },
        ),
        (
            AccountOpReceiptResponse {
                block_index: 1,
                block_number: 1,
                commit_tx_hash: Some(empty_hash()),
                verify_tx_hash: None,
                eth_hash: empty_hash(),
            },
            AccountOpReceipt {
                index: 1,
                hash: H256::default(),
                receipt: Receipt::Committed {
                    block: BlockNumber(1),
                },
            },
        ),
        (
            AccountOpReceiptResponse {
                block_index: 1,
                block_number: 1,
                commit_tx_hash: Some(empty_hash()),
                verify_tx_hash: Some(empty_hash()),
                eth_hash: empty_hash(),
            },
            AccountOpReceipt {
                index: 1,
                hash: H256::default(),
                receipt: Receipt::Verified {
                    block: BlockNumber(1),
                },
            },
        ),
    ];

    for (resp, expected_receipt) in cases {
        let actual_receipt = op_receipt_from_response(resp);
        assert_eq!(actual_receipt, expected_receipt);
    }
}
