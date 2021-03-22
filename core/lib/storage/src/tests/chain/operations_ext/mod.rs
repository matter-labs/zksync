// Built-in imports
use std::collections::HashMap;
// External imports
// Workspace imports
use zksync_types::aggregated_operations::AggregatedActionType;
// Local imports
use self::setup::TransactionsHistoryTestSetup;
use crate::{
    chain::block::BlockSchema,
    chain::operations::OperationsSchema,
    chain::operations_ext::{
        records::{AccountOpReceiptResponse, AccountTxReceiptResponse},
        SearchDirection,
    },
    test_data::{
        dummy_ethereum_tx_hash, gen_sample_block, gen_unique_aggregated_operation,
        BLOCK_SIZE_CHUNKS,
    },
    tests::db_test,
    tokens::StoreTokenError,
    QueryResult, StorageProcessor,
};
use zksync_types::aggregated_operations::AggregatedOperation;
use zksync_types::BlockNumber;

mod setup;

/// Commits the data from the test setup to the database.
async fn commit_schema_data(
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

// Make first block committed and verified.
async fn update_blocks_status(mut storage: &mut StorageProcessor<'_>) -> QueryResult<()> {
    // Required since we use `EthereumSchema` in this test.
    storage.ethereum_schema().initialize_eth_data().await?;
    // Make first block committed.
    BlockSchema(&mut storage)
        .save_block(gen_sample_block(
            BlockNumber(1),
            BLOCK_SIZE_CHUNKS,
            Default::default(),
        ))
        .await?;
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            BlockNumber(1),
            AggregatedActionType::CommitBlocks,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;
    let (id, aggregated_op) = OperationsSchema(&mut storage)
        .get_aggregated_op_that_affects_block(
            AggregatedActionType::CommitBlocks,
            BlockNumber(1_u32),
        )
        .await?
        .unwrap();
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(1), &[], 0)
        .await?;
    confirm_eth_op(
        storage,
        (id, aggregated_op),
        AggregatedActionType::CommitBlocks,
    )
    .await?;

    // Make first block verified.
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            BlockNumber(1),
            AggregatedActionType::ExecuteBlocks,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;
    let (id, op) = OperationsSchema(&mut storage)
        .get_aggregated_op_that_affects_block(
            AggregatedActionType::ExecuteBlocks,
            BlockNumber(1_u32),
        )
        .await?
        .unwrap();
    confirm_eth_op(storage, (id, op), AggregatedActionType::ExecuteBlocks).await?;

    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct ReceiptRequest {
    block_number: u64,
    block_index: Option<u32>,
    limit: u64,
    direction: SearchDirection,
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct ReceiptLocation {
    block_number: i64,
    block_index: Option<i32>,
}

impl ReceiptLocation {
    fn from_tx(item: AccountTxReceiptResponse) -> Self {
        Self {
            block_number: item.block_number,
            block_index: item.block_index,
        }
    }

    fn from_op(item: AccountOpReceiptResponse) -> Self {
        Self {
            block_number: item.block_number,
            block_index: Some(item.block_index),
        }
    }
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

/// Checks that all the transaction receipts related to account address can be loaded
/// with the `get_account_transactions_receipts` method and the result will be
/// same as expected.
#[db_test]
async fn get_account_transactions_receipts(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();
    setup.add_block(1);
    setup.add_block_with_rejected_op(2);

    // execute_operation
    commit_schema_data(&mut storage, &setup).await?;

    let from = setup.from_zksync_account.address;
    let to = setup.to_zksync_account.address;
    let test_data = vec![
        (
            "Get first five transactions.",
            ReceiptRequest {
                block_number: 0,
                block_index: None,
                direction: SearchDirection::Newer,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(1),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(2),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(3),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(4),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(5),
                },
            ],
        ),
        (
            "Get a single transaction. (newer)",
            ReceiptRequest {
                block_number: 1,
                block_index: Some(2),
                direction: SearchDirection::Newer,
                limit: 1,
            },
            vec![ReceiptLocation {
                block_number: 1,
                block_index: Some(2),
            }],
        ),
        (
            "Get a failed transaction. (newer)",
            ReceiptRequest {
                block_number: 2,
                block_index: None,
                direction: SearchDirection::Newer,
                limit: 1,
            },
            vec![ReceiptLocation {
                block_number: 2,
                block_index: None,
            }],
        ),
        (
            "Get some transations from the next block.",
            ReceiptRequest {
                block_number: 1,
                block_index: Some(100),
                direction: SearchDirection::Newer,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 2,
                    block_index: None,
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(1),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(2),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(3),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(4),
                },
            ],
        ),
        (
            "Get five transactions from some index.",
            ReceiptRequest {
                block_number: 1,
                block_index: Some(3),
                direction: SearchDirection::Newer,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(3),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(4),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(5),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: None,
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(1),
                },
            ],
        ),
        // Older search direction
        (
            "Get last five transactions.",
            ReceiptRequest {
                block_number: i64::MAX as u64,
                block_index: Some(i32::MAX as u32),
                direction: SearchDirection::Older,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(4),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(3),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(2),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(1),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: None,
                },
            ],
        ),
        (
            "Get a single transaction (older).",
            ReceiptRequest {
                block_number: 1,
                block_index: Some(2),
                direction: SearchDirection::Older,
                limit: 1,
            },
            vec![ReceiptLocation {
                block_number: 1,
                block_index: Some(2),
            }],
        ),
        (
            "Get a failed transaction. (older)",
            ReceiptRequest {
                block_number: 2,
                block_index: None,
                direction: SearchDirection::Older,
                limit: 1,
            },
            vec![ReceiptLocation {
                block_number: 2,
                block_index: None,
            }],
        ),
        (
            "Get some transations from the previous block.",
            ReceiptRequest {
                block_number: 2,
                block_index: None,
                direction: SearchDirection::Older,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 2,
                    block_index: None,
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(5),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(4),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(3),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(2),
                },
            ],
        ),
        (
            "Get five transactions up to some index.",
            ReceiptRequest {
                block_number: 2,
                block_index: Some(2),
                direction: SearchDirection::Older,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(2),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(1),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: None,
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(5),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(4),
                },
            ],
        ),
    ];

    for (test_name, request, expected_resp) in test_data {
        let items = storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions_receipts(
                from,
                request.block_number,
                request.block_index,
                request.direction,
                request.limit,
            )
            .await?;

        let actual_resp = items
            .into_iter()
            .map(ReceiptLocation::from_tx)
            .collect::<Vec<_>>();

        assert_eq!(actual_resp, expected_resp, "\"{}\", failed", test_name);
    }

    // Make first block committed and verified
    update_blocks_status(&mut storage).await?;

    let receipts = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions_receipts(from, 1, Some(1), SearchDirection::Newer, 1)
        .await?;

    // Check that `commit_tx_hash` and `verify_tx_hash` now exist.
    let reciept = receipts.into_iter().next().unwrap();
    assert!(reciept.commit_tx_hash.is_some());
    assert!(reciept.verify_tx_hash.is_some());
    // Make sure that the receiver see the same receipts.
    let receipts = storage
        .chain()
        .operations_ext_schema()
        .get_account_transactions_receipts(to, 1, Some(1), SearchDirection::Newer, 1)
        .await?;
    assert_eq!(
        storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions_receipts(to, 1, Some(1), SearchDirection::Older, 1)
            .await?,
        receipts
    );

    Ok(())
}

/// Checks that all the operations receipts related to account address can be loaded
/// with the `get_account_operations_receipts` method and the result will be
/// same as expected.
#[db_test]
async fn get_account_operations_receipts(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut setup = TransactionsHistoryTestSetup::new();
    setup.add_block(1);
    setup.add_block_with_rejected_op(2);

    // execute_operation
    commit_schema_data(&mut storage, &setup).await?;

    let from = setup.from_zksync_account.address;
    let to = setup.to_zksync_account.address;
    let test_data = vec![
        (
            "Get first five operations.",
            ReceiptRequest {
                block_number: 0,
                block_index: Some(0),
                direction: SearchDirection::Newer,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(0),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(6),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(0),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(6),
                },
            ],
        ),
        (
            "Get a single operation. (newer)",
            ReceiptRequest {
                block_number: 1,
                block_index: Some(0),
                direction: SearchDirection::Newer,
                limit: 1,
            },
            vec![ReceiptLocation {
                block_number: 1,
                block_index: Some(0),
            }],
        ),
        (
            "Get some operations from the next block.",
            ReceiptRequest {
                block_number: 1,
                block_index: Some(100),
                direction: SearchDirection::Newer,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(0),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(6),
                },
            ],
        ),
        (
            "Get five operations from some index.",
            ReceiptRequest {
                block_number: 1,
                block_index: Some(3),
                direction: SearchDirection::Newer,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(6),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(0),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(6),
                },
            ],
        ),
        // Older search direction
        (
            "Get last five operations.",
            ReceiptRequest {
                block_number: i64::MAX as u64,
                block_index: Some(i32::MAX as u32),
                direction: SearchDirection::Older,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(6),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(0),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(6),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(0),
                },
            ],
        ),
        (
            "Get a single operation (older).",
            ReceiptRequest {
                block_number: 1,
                block_index: Some(6),
                direction: SearchDirection::Older,
                limit: 1,
            },
            vec![ReceiptLocation {
                block_number: 1,
                block_index: Some(6),
            }],
        ),
        (
            "Get some operations from the previous block.",
            ReceiptRequest {
                block_number: 2,
                block_index: Some(0),
                direction: SearchDirection::Older,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(0),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(6),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(0),
                },
            ],
        ),
        (
            "Get five operations up to some index.",
            ReceiptRequest {
                block_number: 2,
                block_index: Some(10),
                direction: SearchDirection::Older,
                limit: 5,
            },
            vec![
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(6),
                },
                ReceiptLocation {
                    block_number: 2,
                    block_index: Some(0),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(6),
                },
                ReceiptLocation {
                    block_number: 1,
                    block_index: Some(0),
                },
            ],
        ),
    ];

    for (test_name, request, expected_resp) in test_data {
        let items = storage
            .chain()
            .operations_ext_schema()
            .get_account_operations_receipts(
                from,
                request.block_number,
                request.block_index.unwrap(),
                request.direction,
                request.limit,
            )
            .await?;

        let actual_resp = items
            .into_iter()
            .map(ReceiptLocation::from_op)
            .collect::<Vec<_>>();

        assert_eq!(actual_resp, expected_resp, "\"{}\", failed", test_name);
    }

    // Make first block committed and verified
    update_blocks_status(&mut storage).await?;

    let receipts = storage
        .chain()
        .operations_ext_schema()
        .get_account_operations_receipts(from, 1, 0, SearchDirection::Newer, 1)
        .await?;

    // Check that `commit_tx_hash` and `verify_tx_hash` now exist.
    let reciept = receipts.into_iter().next().unwrap();
    assert!(reciept.commit_tx_hash.is_some());
    assert!(reciept.verify_tx_hash.is_some());
    // Make sure that the receiver see the same receipts.
    let receipts = storage
        .chain()
        .operations_ext_schema()
        .get_account_operations_receipts(to, 1, 0, SearchDirection::Newer, 1)
        .await?;
    assert_eq!(
        storage
            .chain()
            .operations_ext_schema()
            .get_account_operations_receipts(to, 1, 0, SearchDirection::Older, 1)
            .await?,
        receipts
    );

    Ok(())
}
