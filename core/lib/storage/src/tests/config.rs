// External imports
// Workspace imports
// Local imports
use crate::tests::db_test;
use crate::{QueryResult, StorageProcessor};

/// Server config should be loaded without errors.
#[db_test]
async fn test_load_config(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    storage.config_schema().load_config().await?;

    Ok(())
}
