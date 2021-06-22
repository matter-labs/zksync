// Built-in imports
use std::collections::HashMap;
// External imports
// Workspace imports
use zksync_api_types::v02::{
    pagination::{AccountTxsRequest, ApiEither, PaginationDirection, PaginationQuery},
    transaction::{Receipt, TxInBlockStatus},
};
use zksync_types::{
    aggregated_operations::{AggregatedActionType, AggregatedOperation},
    tx::TxHash,
    BlockNumber, ExecutedOperations,
};
// Local imports
use self::setup::TransactionsHistoryTestSetup;
use crate::{
    chain::block::BlockSchema,
    chain::operations::OperationsSchema,
    chain::operations_ext::SearchDirection,
    test_data::{
        dummy_ethereum_tx_hash, gen_sample_block, gen_unique_aggregated_operation,
        BLOCK_SIZE_CHUNKS,
    },
    tests::db_test,
    tokens::StoreTokenError,
    QueryResult, StorageProcessor,
};

pub mod setup;

/// Commits the data from the test setup to the database.
pub async fn commit_schema_data(
    storage: &mut StorageProcessor<'_>,
    setup: &TransactionsHistoryTestSetup,
) -> QueryResult<()> {
    for token in &setup.tokens {
        let try_insert_token = storage.tokens_schema().store_token(token.clone()).await;
        // If the token is added or it already exists in the database,
        // then we consider that the token was successfully added.
        match try_insert_token {
            Ok(..) | Err(StoreTokenError::TokenAlreadyExistsError(..)) => (),
            Err(StoreTokenError::Other(anyhow_err)) => return Err(anyhow_err),
        }
    }

    for block in &setup.blocks {
        storage
            .chain()
            .block_schema()
            .save_block_transactions(block.block_number, block.block_transactions.clone())
            .await?;
    }

    Ok(())
}

async fn confirm_eth_op(
    storage: &mut StorageProcessor<'_>,
    op: (i64, AggregatedOperation),
    op_type: AggregatedActionType,
) -> QueryResult<()> {
    let eth_tx_hash = dummy_ethereum_tx_hash(op.0);
    let response = storage
        .ethereum_schema()
        .save_new_eth_tx(op_type, Some(op), 100, 100u32.into(), Default::default())
        .await?;
    storage
        .ethereum_schema()
        .add_hash_entry(response.id, &eth_tx_hash)
        .await?;
    storage
        .ethereum_schema()
        .confirm_eth_tx(&eth_tx_hash)
        .await?;

    Ok(())
}

pub async fn commit_block(
    mut storage: &mut StorageProcessor<'_>,
    block_number: BlockNumber,
) -> QueryResult<()> {
    // Required since we use `EthereumSchema` in this test.
    storage.ethereum_schema().initialize_eth_data().await?;
    BlockSchema(&mut storage)
        .save_block(gen_sample_block(
            block_number,
            BLOCK_SIZE_CHUNKS,
            Default::default(),
        ))
        .await?;
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            block_number,
            AggregatedActionType::CommitBlocks,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;
    let (id, aggregated_op) = OperationsSchema(&mut storage)
        .get_aggregated_op_that_affects_block(AggregatedActionType::CommitBlocks, block_number)
        .await?
        .unwrap();
    storage
        .chain()
        .state_schema()
        .commit_state_update(block_number, &[], 0)
        .await?;
    confirm_eth_op(
        storage,
        (id, aggregated_op),
        AggregatedActionType::CommitBlocks,
    )
    .await?;

    Ok(())
}

pub async fn verify_block(
    mut storage: &mut StorageProcessor<'_>,
    block_number: BlockNumber,
) -> QueryResult<()> {
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            block_number,
            AggregatedActionType::ExecuteBlocks,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;
    let (id, op) = OperationsSchema(&mut storage)
        .get_aggregated_op_that_affects_block(AggregatedActionType::ExecuteBlocks, block_number)
        .await?
        .unwrap();
    confirm_eth_op(storage, (id, op), AggregatedActionType::ExecuteBlocks).await?;

    Ok(())
}

/// Here we take the account transactions using `get_account_transactions` and
/// check `get_account_transactions_history` to match obtained results.
#[db_test]
async fn get_account_transactions_history(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();
    setup.add_block(1);

    let from_account_address_string = format!("{:?}", setup.from_zksync_account.address);
    let to_account_address_string = format!("{:?}", setup.to_zksync_account.address);

    let expected_behavior = {
        let mut expected_behavior = HashMap::new();
        expected_behavior.insert(
            "Deposit",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(setup.tokens[0].symbol.clone()),
                Some(setup.amount.to_string()),
            ),
        );
        expected_behavior.insert(
            "Transfer",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(setup.tokens[1].symbol.clone()),
                Some(setup.amount.to_string()),
            ),
        );
        expected_behavior.insert(
            "Withdraw",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(setup.tokens[2].symbol.clone()),
                Some(setup.amount.to_string()),
            ),
        );
        expected_behavior
    };

    // execute_operation
    commit_schema_data(&mut storage, &setup).await?;

    let from_history = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions_history(&setup.from_zksync_account.address, 0, 10)
        .await?;

    for tx in &from_history {
        let tx_type: &str = tx.tx["type"].as_str().expect("no tx_type");

        assert!(tx.hash.is_some());

        if let Some((from, to, token, amount)) = expected_behavior.get(tx_type) {
            let tx_info = match tx_type {
                "Deposit" | "FullExit" => tx.tx["priority_op"].clone(),
                _ => tx.tx.clone(),
            };
            let tx_from_addr = tx_info["from"].as_str();
            let tx_to_addr = tx_info["to"].as_str();
            let tx_token = tx_info["token"].as_str().map(String::from);
            let tx_amount = tx_info["amount"].as_str().map(String::from);

            assert_eq!(tx_from_addr, *from);
            assert_eq!(tx_to_addr, *to);
            assert_eq!(tx_token, *token);
            assert_eq!(tx_amount, *amount);
        }
    }

    let to_history = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions_history(&setup.to_zksync_account.address, 0, 10)
        .await?;

    assert_eq!(from_history.len(), 7);
    assert_eq!(to_history.len(), 4);

    Ok(())
}

/// Checks that all the transactions related to account address can be loaded
/// with the `get_account_transactions_history_from` method and the result will
/// be the same as if it'll be gotten via `get_account_transactions_history`.
#[db_test]
async fn get_account_transactions_history_from(
    mut storage: StorageProcessor<'_>,
) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();
    setup.add_block(1);
    setup.add_block(2);

    let block_size = setup.blocks[0].block_transactions.len() as u64;

    let txs_from = 7; // Amount of transactions related to "from" account.
    let txs_to = 4;

    // execute_operation
    commit_schema_data(&mut storage, &setup).await?;

    let test_vector = vec![
        // Go back from the second block and fetch all the txs of the first block.
        (1, 1, 2, 0, SearchDirection::Older),
        // Go back from the third block and fetch all the txs of the second block.
        (0, 1, 3, 0, SearchDirection::Older),
        // Go back from the third block and fetch all the txs of the first two blocks.
        (0, 2, 3, 0, SearchDirection::Older),
        // Load all the transactions newer than genesis.
        (0, 2, 0, 0, SearchDirection::Newer),
        // Load all the transactions newer than the last tx of the first block.
        (0, 1, 1, block_size, SearchDirection::Newer),
    ];

    for (start_block, n_blocks, block_id, tx_id, direction) in test_vector {
        let offset_from = start_block * txs_from;
        let limit_from = n_blocks * txs_from;
        let offset_to = start_block * txs_to;
        let limit_to = n_blocks * txs_to;

        let expected_from_history = storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history(
                &setup.from_zksync_account.address,
                offset_from,
                limit_from,
            )
            .await?;
        let expected_to_history = storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history(&setup.to_zksync_account.address, offset_to, limit_to)
            .await?;

        let from_history = storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history_from(
                &setup.from_zksync_account.address,
                (block_id, tx_id),
                direction,
                limit_from,
            )
            .await?;
        let to_history = storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history_from(
                &setup.to_zksync_account.address,
                (block_id, tx_id),
                direction,
                limit_to,
            )
            .await?;

        assert_eq!(
            from_history, expected_from_history,
            "Assertion 'from' failed for the following input: \
                [ offset {}, limit: {}, block_id: {}, tx_id: {}, direction: {:?} ]",
            offset_from, limit_from, block_id, tx_id, direction
        );
        assert_eq!(
            to_history, expected_to_history,
            "Assertion 'to' failed for the following input: \
                [ offset {}, limit: {}, block_id: {}, tx_id: {}, direction: {:?} ]",
            offset_to, limit_to, block_id, tx_id, direction
        );
    }

    Ok(())
}

pub struct ReceiptRequest {
    tx_hash: TxHash,
    direction: PaginationDirection,
    limit: u32,
}

/// Checks that all the transaction related to account address can be loaded
/// with the `get_account_transactions` method and the result will be
/// same as expected.
#[db_test]
async fn get_account_transactions(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();
    let from = setup.from_zksync_account.address;
    let to = setup.to_zksync_account.address;
    setup.add_block(1);
    setup.add_block_with_rejected_op(2);

    // Check that it doesn't return not committed txs.
    let txs = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions(&PaginationQuery {
            from: AccountTxsRequest {
                address: from,
                tx_hash: ApiEither::from(setup.get_tx_hash(0, 0)),
            },
            limit: 1,
            direction: PaginationDirection::Newer,
        })
        .await?;
    assert!(txs.is_none());

    // execute_operation
    commit_schema_data(&mut storage, &setup).await?;

    // Make blocks committed
    commit_block(&mut storage, BlockNumber(1)).await?;
    commit_block(&mut storage, BlockNumber(2)).await?;

    let test_data = vec![
        (
            "Get first five transactions.",
            ReceiptRequest {
                tx_hash: setup.get_tx_hash(0, 0),
                direction: PaginationDirection::Newer,
                limit: 5,
            },
            vec![
                setup.get_tx_hash(0, 0),
                setup.get_tx_hash(0, 1),
                setup.get_tx_hash(0, 2),
                setup.get_tx_hash(0, 3),
                setup.get_tx_hash(0, 4),
            ],
        ),
        (
            "Get a single transaction. (newer)",
            ReceiptRequest {
                tx_hash: setup.get_tx_hash(0, 2),
                direction: PaginationDirection::Newer,
                limit: 1,
            },
            vec![setup.get_tx_hash(0, 2)],
        ),
        (
            "Get five transactions from some index.",
            ReceiptRequest {
                tx_hash: setup.get_tx_hash(0, 4),
                direction: PaginationDirection::Newer,
                limit: 5,
            },
            vec![
                setup.get_tx_hash(0, 4),
                setup.get_tx_hash(0, 5),
                setup.get_tx_hash(0, 6),
                setup.get_tx_hash(1, 0),
                setup.get_tx_hash(1, 1),
            ],
        ),
        (
            "Limit is more than number of txs. (Newer)",
            ReceiptRequest {
                tx_hash: setup.get_tx_hash(1, 5),
                direction: PaginationDirection::Newer,
                limit: 5,
            },
            vec![setup.get_tx_hash(1, 5), setup.get_tx_hash(1, 6)],
        ),
        // Older search direction
        (
            "Get last five transactions.",
            ReceiptRequest {
                tx_hash: setup.get_tx_hash(1, 6),
                direction: PaginationDirection::Older,
                limit: 5,
            },
            vec![
                setup.get_tx_hash(1, 6),
                setup.get_tx_hash(1, 5),
                setup.get_tx_hash(1, 4),
                setup.get_tx_hash(1, 3),
                setup.get_tx_hash(1, 2),
            ],
        ),
        (
            "Get a single transaction. (older)",
            ReceiptRequest {
                tx_hash: setup.get_tx_hash(0, 2),
                direction: PaginationDirection::Older,
                limit: 1,
            },
            vec![setup.get_tx_hash(0, 2)],
        ),
        (
            "Get some transactions from the previous block.",
            ReceiptRequest {
                tx_hash: setup.get_tx_hash(1, 2),
                direction: PaginationDirection::Older,
                limit: 5,
            },
            vec![
                setup.get_tx_hash(1, 2),
                setup.get_tx_hash(1, 1),
                setup.get_tx_hash(1, 0),
                setup.get_tx_hash(0, 6),
                setup.get_tx_hash(0, 5),
            ],
        ),
        (
            "Limit is more than number of txs. (Older)",
            ReceiptRequest {
                tx_hash: setup.get_tx_hash(0, 2),
                direction: PaginationDirection::Older,
                limit: 5,
            },
            vec![
                setup.get_tx_hash(0, 2),
                setup.get_tx_hash(0, 1),
                setup.get_tx_hash(0, 0),
            ],
        ),
    ];

    for (test_name, request, expected_resp) in test_data {
        let items = storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions(&PaginationQuery {
                from: AccountTxsRequest {
                    address: from,
                    tx_hash: ApiEither::from(request.tx_hash),
                },
                limit: request.limit,
                direction: request.direction,
            })
            .await?;
        let actual_resp: Vec<TxHash> = items.unwrap().into_iter().map(|tx| tx.tx_hash).collect();

        assert_eq!(actual_resp, expected_resp, "\"{}\", failed", test_name);
    }

    let failed_tx = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions(&PaginationQuery {
            from: AccountTxsRequest {
                address: from,
                tx_hash: ApiEither::from(setup.get_tx_hash(1, 2)),
            },
            limit: 1,
            direction: PaginationDirection::Newer,
        })
        .await?
        .unwrap();
    assert_eq!(failed_tx[0].status, TxInBlockStatus::Rejected);

    verify_block(&mut storage, BlockNumber(1)).await?;
    let txs = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions(&PaginationQuery {
            from: AccountTxsRequest {
                address: from,
                tx_hash: ApiEither::from(setup.get_tx_hash(0, 6)),
            },
            limit: 2,
            direction: PaginationDirection::Newer,
        })
        .await?
        .unwrap();
    assert_eq!(txs[0].status, TxInBlockStatus::Finalized);
    assert_eq!(txs[1].status, TxInBlockStatus::Committed);

    // Make sure that the receiver see the same receipts.
    let from_txs = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions(&PaginationQuery {
            from: AccountTxsRequest {
                address: from,
                tx_hash: ApiEither::from(setup.get_tx_hash(0, 2)),
            },
            limit: 1,
            direction: PaginationDirection::Newer,
        })
        .await?
        .unwrap();
    let to_txs = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions(&PaginationQuery {
            from: AccountTxsRequest {
                address: to,
                tx_hash: ApiEither::from(setup.get_tx_hash(0, 2)),
            },
            limit: 1,
            direction: PaginationDirection::Newer,
        })
        .await?
        .unwrap();
    let from_txs_hashes: Vec<TxHash> = from_txs.into_iter().map(|tx| tx.tx_hash).collect();
    let to_txs_hashes: Vec<TxHash> = to_txs.into_iter().map(|tx| tx.tx_hash).collect();
    assert_eq!(from_txs_hashes, to_txs_hashes);

    Ok(())
}

/// Test `get_tx_created_at_and_block_number` method
#[db_test]
async fn get_tx_created_at_and_block_number(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();
    setup.add_block(1);
    commit_schema_data(&mut storage, &setup).await?;

    // Get priority op created_at and block_number
    let tx_hash = setup.get_tx_hash(0, 0);
    let result = storage
        .chain()
        .operations_ext_schema()
        .get_tx_created_at_and_block_number(tx_hash)
        .await?;
    assert!(result.is_some());
    assert_eq!(result.unwrap().1, BlockNumber(1));

    // Get transaction created_at and block_number
    let tx_hash = setup.get_tx_hash(0, 1);
    let result = storage
        .chain()
        .operations_ext_schema()
        .get_tx_created_at_and_block_number(tx_hash)
        .await?;
    assert!(result.is_some());
    assert_eq!(result.unwrap().1, BlockNumber(1));

    // Try to get unexisting tx
    setup.add_block(2);
    let tx_hash = setup.get_tx_hash(1, 0);
    let result = storage
        .chain()
        .operations_ext_schema()
        .get_tx_created_at_and_block_number(tx_hash)
        .await?;
    assert!(result.is_none());

    Ok(())
}

/// Test `get_batch_info` method
#[db_test]
async fn get_batch_info(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();

    // `batch_id` will be added after we insert batch into mempool.
    setup.add_block_with_batch(1, true);
    setup.add_block_with_batch(2, false);

    for i in 0..2 {
        let txs: Vec<_> = setup.blocks[i]
            .block_transactions
            .iter()
            .map(|tx| tx.get_executed_tx().unwrap().signed_tx.clone())
            .collect();
        let batch_id = storage
            .chain()
            .mempool_schema()
            .insert_batch(&txs, Vec::new())
            .await?;
        setup.blocks[i]
            .block_transactions
            .iter_mut()
            .for_each(|tx| match tx {
                ExecutedOperations::Tx(tx) => {
                    tx.batch_id = Some(batch_id);
                }
                _ => unreachable!(),
            });
    }

    // Get batch from mempool
    let tx_hashes = vec![
        setup.get_tx_hash(0, 0),
        setup.get_tx_hash(0, 1),
        setup.get_tx_hash(0, 2),
    ];
    let batch_hash = TxHash::batch_hash(&tx_hashes);
    let batch_info = storage
        .chain()
        .operations_ext_schema()
        .get_batch_info(batch_hash)
        .await?
        .unwrap();

    let actual_tx_hashes: Vec<TxHash> = batch_info
        .transaction_hashes
        .into_iter()
        .map(|tx_hash| tx_hash.0)
        .collect();
    assert_eq!(batch_info.batch_hash, batch_hash);
    assert_eq!(actual_tx_hashes, tx_hashes);
    assert_eq!(batch_info.batch_status.last_state, TxInBlockStatus::Queued);

    // Get batch from queued block.
    commit_schema_data(&mut storage, &setup).await?;
    storage.chain().mempool_schema().collect_garbage().await?;

    let batch_info = storage
        .chain()
        .operations_ext_schema()
        .get_batch_info(batch_hash)
        .await?
        .unwrap();

    let actual_tx_hashes: Vec<TxHash> = batch_info
        .transaction_hashes
        .into_iter()
        .map(|tx_hash| tx_hash.0)
        .collect();
    assert_eq!(batch_info.batch_hash, batch_hash);
    assert_eq!(actual_tx_hashes, tx_hashes);
    assert_eq!(
        batch_info.batch_status.last_state,
        TxInBlockStatus::Committed
    );

    // Get batch from committed block.
    commit_block(&mut storage, BlockNumber(1)).await?;

    let batch_info = storage
        .chain()
        .operations_ext_schema()
        .get_batch_info(batch_hash)
        .await?
        .unwrap();
    assert_eq!(
        batch_info.batch_status.last_state,
        TxInBlockStatus::Committed
    );

    // Get batch from finalized block.
    verify_block(&mut storage, BlockNumber(1)).await?;
    let batch_info = storage
        .chain()
        .operations_ext_schema()
        .get_batch_info(batch_hash)
        .await?
        .unwrap();
    assert_eq!(
        batch_info.batch_status.last_state,
        TxInBlockStatus::Finalized
    );

    // Get failed batch.
    let tx_hashes = vec![
        setup.get_tx_hash(1, 0),
        setup.get_tx_hash(1, 1),
        setup.get_tx_hash(1, 2),
    ];
    let batch_hash = TxHash::batch_hash(&tx_hashes);
    let batch_info = storage
        .chain()
        .operations_ext_schema()
        .get_batch_info(batch_hash)
        .await?
        .unwrap();
    assert_eq!(
        batch_info.batch_status.last_state,
        TxInBlockStatus::Rejected
    );

    Ok(())
}

/// Test `get_account_transactions_count` method
#[db_test]
async fn account_transactions_count(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();
    setup.add_block(1);
    commit_schema_data(&mut storage, &setup).await?;

    let count_before_commit = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions_count(setup.from_zksync_account.address)
        .await?;
    assert_eq!(count_before_commit, 0);

    commit_block(&mut storage, BlockNumber(1)).await?;

    let count_after_commit = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions_count(setup.from_zksync_account.address)
        .await?;
    assert_eq!(count_after_commit, 7);

    Ok(())
}

/// Test `get_account_last_tx_hash` method
#[db_test]
async fn account_last_tx_hash(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();

    // Checks that it returns None for unexisting account
    let last_tx_hash = storage
        .chain()
        .operations_ext_schema()
        .get_account_last_tx_hash(setup.from_zksync_account.address)
        .await?;
    assert!(last_tx_hash.is_none());

    setup.add_block(1);
    commit_schema_data(&mut storage, &setup).await?;

    let last_tx_hash = storage
        .chain()
        .operations_ext_schema()
        .get_account_last_tx_hash(setup.from_zksync_account.address)
        .await?;
    assert_eq!(last_tx_hash, Some(setup.get_tx_hash(0, 6)));

    Ok(())
}

/// Test `get_block_last_tx_hash` method
#[db_test]
async fn block_last_tx_hash(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();

    // Checks that it returns None for unexisting block
    let last_tx_hash = storage
        .chain()
        .operations_ext_schema()
        .get_block_last_tx_hash(BlockNumber(1))
        .await?;
    assert!(last_tx_hash.is_none());

    setup.add_block(1);
    commit_schema_data(&mut storage, &setup).await?;

    let last_tx_hash = storage
        .chain()
        .operations_ext_schema()
        .get_block_last_tx_hash(BlockNumber(1))
        .await?;
    assert_eq!(last_tx_hash, Some(setup.get_tx_hash(0, 6)));
    Ok(())
}

/// Test `tx_receipt_api_v02` method
#[db_test]
async fn tx_receipt(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();

    // Checks that it returns None for unexisting tx
    let receipt = storage
        .chain()
        .operations_ext_schema()
        .tx_receipt_api_v02(&[0xDE, 0xAD, 0xBE, 0xEF])
        .await?;
    assert!(receipt.is_none());

    setup.add_block(1);
    commit_schema_data(&mut storage, &setup).await?;

    // Test receipt for L1 op.
    let (expected_id, eth_hash) = match setup.blocks[0].block_transactions[0].clone() {
        ExecutedOperations::PriorityOp(op) => (op.priority_op.serial_id, op.priority_op.eth_hash),
        ExecutedOperations::Tx(_) => {
            panic!("Should be L1 op")
        }
    };

    let l1_receipt_by_tx_hash = storage
        .chain()
        .operations_ext_schema()
        .tx_receipt_api_v02(setup.get_tx_hash(0, 0).as_ref())
        .await?;
    match l1_receipt_by_tx_hash.unwrap() {
        Receipt::L1(receipt) => {
            assert_eq!(receipt.id, expected_id);
        }
        Receipt::L2(_) => {
            panic!("Should be L1 receipt");
        }
    }

    let l1_receipt_by_eth_hash = storage
        .chain()
        .operations_ext_schema()
        .tx_receipt_api_v02(eth_hash.as_ref())
        .await?;
    match l1_receipt_by_eth_hash.unwrap() {
        Receipt::L1(receipt) => {
            assert_eq!(receipt.id, expected_id);
        }
        Receipt::L2(_) => {
            panic!("Should be L1 receipt");
        }
    }

    // Test receipt for executed L2 tx.
    let l2_receipt = storage
        .chain()
        .operations_ext_schema()
        .tx_receipt_api_v02(setup.get_tx_hash(0, 2).as_ref())
        .await?;
    match l2_receipt.unwrap() {
        Receipt::L2(receipt) => {
            assert_eq!(receipt.tx_hash, setup.get_tx_hash(0, 2));
        }
        Receipt::L1(_) => {
            panic!("Should be L2 receipt");
        }
    }

    // Test receipt for tx from mempool.
    setup.add_block(2);
    let tx = match setup.blocks[1].block_transactions[2].clone() {
        ExecutedOperations::Tx(tx) => tx.signed_tx,
        ExecutedOperations::PriorityOp(_) => {
            panic!("Should be L2 tx")
        }
    };
    storage.chain().mempool_schema().insert_tx(&tx).await?;
    let l2_receipt = storage
        .chain()
        .operations_ext_schema()
        .tx_receipt_api_v02(tx.hash().as_ref())
        .await?;
    match l2_receipt.unwrap() {
        Receipt::L2(receipt) => {
            assert_eq!(receipt.tx_hash, tx.hash());
        }
        Receipt::L1(_) => {
            panic!("Should be L2 receipt");
        }
    }

    Ok(())
}

/// Test `tx_data_api_v02` method
#[db_test]
async fn tx_data(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();

    // Checks that it returns None for unexisting tx
    let data = storage
        .chain()
        .operations_ext_schema()
        .tx_data_api_v02(&[0xDE, 0xAD, 0xBE, 0xEF])
        .await?;
    assert!(data.is_none());

    setup.add_block(1);
    commit_schema_data(&mut storage, &setup).await?;

    // Test data for L1 op.
    let eth_hash = match setup.blocks[0].block_transactions[0].clone() {
        ExecutedOperations::PriorityOp(op) => op.priority_op.eth_hash,
        ExecutedOperations::Tx(_) => {
            panic!("Should be L1 op")
        }
    };

    let l1_data_by_tx_hash = storage
        .chain()
        .operations_ext_schema()
        .tx_data_api_v02(setup.get_tx_hash(0, 0).as_ref())
        .await?;
    assert_eq!(
        l1_data_by_tx_hash.unwrap().tx.tx_hash,
        setup.get_tx_hash(0, 0)
    );

    let l1_data_by_eth_hash = storage
        .chain()
        .operations_ext_schema()
        .tx_data_api_v02(eth_hash.as_ref())
        .await?;
    assert_eq!(
        l1_data_by_eth_hash.unwrap().tx.tx_hash,
        setup.get_tx_hash(0, 0)
    );

    // Test data for executed L2 tx.
    let l2_data = storage
        .chain()
        .operations_ext_schema()
        .tx_data_api_v02(setup.get_tx_hash(0, 2).as_ref())
        .await?;
    assert_eq!(l2_data.unwrap().tx.tx_hash, setup.get_tx_hash(0, 2));

    // Test data for tx from mempool.
    setup.add_block(2);
    let tx = match setup.blocks[1].block_transactions[2].clone() {
        ExecutedOperations::Tx(tx) => tx.signed_tx,
        ExecutedOperations::PriorityOp(_) => {
            panic!("Should be L2 tx")
        }
    };
    storage.chain().mempool_schema().insert_tx(&tx).await?;
    let l2_data = storage
        .chain()
        .operations_ext_schema()
        .tx_data_api_v02(tx.hash().as_ref())
        .await?;
    assert_eq!(l2_data.unwrap().tx.tx_hash, tx.hash());

    Ok(())
}
