// Built-in imports
use std::collections::HashMap;
// External imports
// Workspace imports
// Local imports
use self::setup::TransactionsHistoryTestSetup;
use crate::{
    chain::operations_ext::SearchDirection, tests::db_test, QueryResult, StorageProcessor,
};

mod setup;

/// Commits the data from the test setup to the database.
async fn commit_schema_data(
    storage: &mut StorageProcessor<'_>,
    setup: &TransactionsHistoryTestSetup,
) -> QueryResult<()> {
    for token in &setup.tokens {
        storage.tokens_schema().store_token(token.clone()).await?;
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
