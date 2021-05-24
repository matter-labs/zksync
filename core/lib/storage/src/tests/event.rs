// Built-in uses
// External uses
// Workspace uses
use zksync_types::{
    aggregated_operations::AggregatedActionType,
    event::{
        account::AccountStateChangeStatus, block::BlockStatus, EventData, EventId, ZkSyncEvent,
    },
    AccountMap, BlockNumber,
};
// Local uses
use super::{chain::apply_random_updates, create_rng, db_test, ACCOUNT_MUTEX};
use crate::{
    test_data::{
        dummy_ethereum_tx_hash, gen_sample_block, gen_unique_aggregated_operation,
        BLOCK_SIZE_CHUNKS,
    },
    QueryResult, StorageProcessor,
};

// TODO: add a test for transaction events.
// Trying to store non-empty block concurrently causes a dead-lock in tests.
// (inserts into `executed_priority_operations` and `eth_account_types`)
// Also, a more generic setup is needed to check a deposit event which
// doesn't store an account id.

/// Helper method for populating block events in the database.
/// Since `store_block_event` relies on `load_block_range` method,
/// it's necessary to have a confirmed Eth transaction in the db,
/// otherwise the query fetching `BlockDetails` will return `None`.
async fn store_operation(
    storage: &mut StorageProcessor<'_>,
    action_type: AggregatedActionType,
    block_number: BlockNumber,
) -> QueryResult<()> {
    storage
        .chain()
        .operations_schema()
        .store_aggregated_action(gen_unique_aggregated_operation(
            block_number,
            action_type,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;

    let (id, op) = storage
        .chain()
        .operations_schema()
        .get_aggregated_op_that_affects_block(action_type, block_number)
        .await?
        .unwrap();
    let eth_tx_hash = dummy_ethereum_tx_hash(id);
    let response = storage
        .ethereum_schema()
        .save_new_eth_tx(
            action_type,
            Some((id, op)),
            100,
            100u32.into(),
            Default::default(),
        )
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

fn check_block_event(event: &ZkSyncEvent, block_status: BlockStatus, block_number: BlockNumber) {
    assert_eq!(event.block_number, block_number);
    let block_event = match &event.data {
        EventData::Block(block_event) => block_event,
        _ => panic!("block event expected"),
    };
    assert_eq!(block_event.status, block_status);
    assert_eq!(block_event.block_details.block_number, block_number);
}

/// Checks that block events are created correctly and can be deserialized.
/// The test does the following:
/// 1. Commit 3 blocks, after each commit fetch a single "block committed" event.
/// 2. Commit 4th block.
/// 3. Finalize first 3 blocks then fetch 1 "block committed" event and 3 "block finalized"
/// in a single query.
/// 4. Revert all 4 blocks and expect new "block reverted" events.
#[db_test]
async fn test_block_events(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut last_event_id = EventId(0);
    assert!(
        storage
            .event_schema()
            .fetch_new_events(last_event_id)
            .await?
            .is_empty(),
        "database should be empty"
    );

    storage.ethereum_schema().initialize_eth_data().await?;
    const FROM_BLOCK: u32 = 1;
    const TO_BLOCK: u32 = 3;

    for block_number in FROM_BLOCK..=TO_BLOCK {
        let block_number = BlockNumber(block_number);
        // It's important not to store any operations inside the block, otherwise
        // transaction events will be created too causing the test to fail.
        storage
            .chain()
            .block_schema()
            .save_block(gen_sample_block(
                block_number,
                BLOCK_SIZE_CHUNKS,
                Vec::new(),
            ))
            .await?;
        // Commit the block.
        store_operation(
            &mut storage,
            AggregatedActionType::CommitBlocks,
            block_number,
        )
        .await?;
        // Expect a single block event with the `Committed` status.
        let events = storage
            .event_schema()
            .fetch_new_events(last_event_id)
            .await?;

        assert_eq!(events.len(), 1);
        check_block_event(&events[0], BlockStatus::Committed, block_number);
        last_event_id = events[0].id;
    }
    // Commit one more block.
    let block_number = BlockNumber(TO_BLOCK + 1);
    storage
        .chain()
        .block_schema()
        .save_block(gen_sample_block(
            block_number,
            BLOCK_SIZE_CHUNKS,
            Vec::new(),
        ))
        .await?;
    store_operation(
        &mut storage,
        AggregatedActionType::CommitBlocks,
        block_number,
    )
    .await?;
    // Finalize first pack.
    for block_number in FROM_BLOCK..=TO_BLOCK {
        let block_number = BlockNumber(block_number);
        store_operation(
            &mut storage,
            AggregatedActionType::ExecuteBlocks,
            block_number,
        )
        .await?;
    }
    // Fetch new events.
    let events = storage
        .event_schema()
        .fetch_new_events(last_event_id)
        .await?;
    // Update the offset.
    last_event_id = events.last().unwrap().id;
    let expected_len = TO_BLOCK as usize + 1;
    assert_eq!(events.len(), expected_len);
    // The first event is "block committed".
    let mut events_iter = events.into_iter();
    let block_event = events_iter.next().unwrap();
    check_block_event(&block_event, BlockStatus::Committed, block_number);
    // The rest is "block finalized".
    for block_number in FROM_BLOCK..=TO_BLOCK {
        let block_number = BlockNumber(block_number);
        let event = events_iter.next().unwrap();
        check_block_event(&event, BlockStatus::Finalized, block_number);
    }
    // Revert all blocks.
    storage
        .chain()
        .block_schema()
        .remove_blocks(BlockNumber(0))
        .await?;
    let mut events = storage
        .event_schema()
        .fetch_new_events(last_event_id)
        .await?
        .into_iter();
    // Check the status for each event.
    for block_number in FROM_BLOCK..=TO_BLOCK + 1 {
        let block_number = BlockNumber(block_number);
        let event = events.next().unwrap();
        check_block_event(&event, BlockStatus::Reverted, block_number);
    }

    Ok(())
}

fn check_account_event(event: &ZkSyncEvent, status: AccountStateChangeStatus) -> bool {
    match &event.data {
        EventData::Account(account_event) => account_event.status == status,
        _ => false,
    }
}

/// Checks the creation of account events in the database.
/// The test flow is as follows:
/// 1. Commit 1 state update and fetch all new events.
/// 2. Commit another update and finalize the first one, fetch
/// new events in a single query and verify their correctness.
#[db_test]
async fn test_account_events(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let _lock = ACCOUNT_MUTEX.lock().await;
    let last_event_id = EventId(0);
    assert!(
        storage
            .event_schema()
            .fetch_new_events(last_event_id)
            .await?
            .is_empty(),
        "database should be empty"
    );

    storage.ethereum_schema().initialize_eth_data().await?;

    let mut rng = create_rng();
    let (accounts_block_1, updates_block_1) = apply_random_updates(AccountMap::default(), &mut rng);
    // To create account events we have to commit a block. It will
    // also create a block event which is expected to be inserted first.

    // Commit state update and confirm Ethereum operation.
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(1), &updates_block_1, 0)
        .await?;
    storage
        .chain()
        .block_schema()
        .save_block(gen_sample_block(
            BlockNumber(1),
            BLOCK_SIZE_CHUNKS,
            Vec::new(),
        ))
        .await?;
    store_operation(
        &mut storage,
        AggregatedActionType::CommitBlocks,
        BlockNumber(1),
    )
    .await?;
    // Load new events. The first event should be "block committed",
    // the rest is "state updated".
    let events = storage
        .event_schema()
        .fetch_new_events(last_event_id)
        .await?;
    assert!(!events.is_empty());
    assert_eq!(events.len(), updates_block_1.len() + 1);
    // For all events the status is `Committed`.
    assert!(events
        .iter()
        .skip(1) // Skip block event.
        .all(|event| check_account_event(event, AccountStateChangeStatus::Committed)));
    // And the block number is correct too.
    assert!(events
        .iter()
        .all(|event| event.block_number == BlockNumber(1)));
    // Update the offset.
    let last_event_id = events.last().unwrap().id;
    // New pack of updates. Commit it and apply the previous one.
    let (_, updates_block_2) = apply_random_updates(accounts_block_1.clone(), &mut rng);
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(2), &updates_block_2, 0)
        .await?;
    storage
        .chain()
        .block_schema()
        .save_block(gen_sample_block(
            BlockNumber(2),
            BLOCK_SIZE_CHUNKS,
            Vec::new(),
        ))
        .await?;
    store_operation(
        &mut storage,
        AggregatedActionType::CommitBlocks,
        BlockNumber(2),
    )
    .await?;
    // Finalize updates for the first block.
    storage
        .chain()
        .state_schema()
        .apply_state_update(BlockNumber(1))
        .await?;
    store_operation(
        &mut storage,
        AggregatedActionType::ExecuteBlocks,
        BlockNumber(1),
    )
    .await?;
    // Load new events.
    let events = storage
        .event_schema()
        .fetch_new_events(last_event_id)
        .await?;
    assert_eq!(
        events.len(),
        updates_block_1.len() + updates_block_2.len() + 2
    );
    assert!(events
        .iter()
        .skip(1)
        .take(updates_block_2.len())
        .all(|event| event.block_number == BlockNumber(2)
            && check_account_event(event, AccountStateChangeStatus::Committed)));
    assert!(events
        .iter()
        .skip(updates_block_2.len() + 2)
        .all(|event| event.block_number == BlockNumber(1)
            && check_account_event(event, AccountStateChangeStatus::Finalized)));
    Ok(())
}
