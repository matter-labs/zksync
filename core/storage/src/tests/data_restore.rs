// External imports
// Workspace imports
// Local imports
use crate::tests::db_test;
use crate::{
    data_restore::{records::NewLastWatchedEthBlockNumber, DataRestoreSchema},
    StorageProcessor,
};

/// Checks that storing and loading the last watched block number
/// works as expected.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn last_watched_block() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Check that by default we can't obtain the block number.
        let last_watched_block_number = DataRestoreSchema(&conn).load_last_watched_block_number();
        assert!(
            last_watched_block_number.is_err(),
            "There should be no stored block number in the database"
        );

        // Store the block number.
        DataRestoreSchema(&conn).update_last_watched_block_number(
            &NewLastWatchedEthBlockNumber {
                block_number: "0".into(),
            },
        )?;

        // Load it again.
        let last_watched_block_number =
            DataRestoreSchema(&conn).load_last_watched_block_number()?;

        assert_eq!(last_watched_block_number.block_number, "0");

        // Repeat save/load with other values.
        DataRestoreSchema(&conn).update_last_watched_block_number(
            &NewLastWatchedEthBlockNumber {
                block_number: "1".into(),
            },
        )?;
        let last_watched_block_number =
            DataRestoreSchema(&conn).load_last_watched_block_number()?;
        assert_eq!(last_watched_block_number.block_number, "1");

        Ok(())
    });
}
