// External imports
// Workspace imports
// Local imports
use crate::tests::db_test;
use crate::{data_restore::DataRestoreSchema, QueryResult, StorageProcessor};

/// Checks that storing and loading the last watched block number
/// works as expected.
#[db_test]
async fn last_watched_block(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Check that by default we can't obtain the block number.
    let last_watched_block_number = DataRestoreSchema(&mut storage)
        .load_last_watched_block_number()
        .await;
    assert!(
        last_watched_block_number.is_err(),
        "There should be no stored block number in the database"
    );

    // Store the block number.
    DataRestoreSchema(&mut storage)
        .update_last_watched_block_number("0")
        .await?;

    // Load it again.
    let last_watched_block_number = DataRestoreSchema(&mut storage)
        .load_last_watched_block_number()
        .await?;

    assert_eq!(last_watched_block_number.block_number, "0");

    // Repeat save/load with other values.
    DataRestoreSchema(&mut storage)
        .update_last_watched_block_number("1")
        .await?;
    let last_watched_block_number = DataRestoreSchema(&mut storage)
        .load_last_watched_block_number()
        .await?;
    assert_eq!(last_watched_block_number.block_number, "1");

    Ok(())
}
