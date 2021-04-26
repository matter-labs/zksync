// Built-in uses
// External uses
// Workspace uses
use zksync_types::{
    aggregated_operations::AggregatedActionType,
    event::{block::BlockStatus, EventData, ZkSyncEvent},
    BlockNumber,
};
// Local uses
use super::db_test;
use crate::{
    test_data::{
        dummy_ethereum_tx_hash, gen_sample_block, gen_unique_aggregated_operation,
        BLOCK_SIZE_CHUNKS,
    },
    QueryResult, StorageProcessor,
};

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
    let block_event = match &event.data {
        EventData::Block(block_event) => block_event,
        _ => panic!("block event expected"),
    };
    assert_eq!(block_event.status, block_status);
    assert_eq!(block_event.block_details.block_number, *block_number as i64);
}

/// Checks that block events are created correctly and can be deserialized.
/// The test does the following:
/// 1. Commit 3 blocks, after each commit fetch a single "block committed" event.
/// 2. Commit 4th block.
/// 3. Finalize first 3 blocks then fetch 1 "block committed" event and 3 "block finalized"
/// in a single query.
#[db_test]
async fn test_block_events(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    storage.ethereum_schema().initialize_eth_data().await?;
    let mut last_event_id = 0;
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
    let block_number = BlockNumber(4);
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
    // Fetch populated events.
    let events = storage
        .event_schema()
        .fetch_new_events(last_event_id)
        .await?;
    assert_eq!(events.len(), 4);
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

    Ok(())
}
