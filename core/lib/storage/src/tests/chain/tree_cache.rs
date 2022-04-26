// External imports
// Workspace imports
use zksync_types::BlockNumber;
// Local imports
use crate::{
    chain::{
        block::BlockSchema,
        tree_cache::{TreeCacheSchemaBincode, TreeCacheSchemaJSON},
    },
    test_data::{gen_sample_block, BLOCK_SIZE_CHUNKS},
    tests::db_test,
    QueryResult, StorageProcessor,
};

/// Check that account tree cache is removed correctly.
#[db_test]
async fn test_remove_old_account_tree_cache_json(
    mut storage: StorageProcessor<'_>,
) -> QueryResult<()> {
    // Insert account tree cache for 5 blocks.
    for block_number in 1..=5 {
        BlockSchema(&mut storage)
            .save_full_block(gen_sample_block(
                BlockNumber(block_number),
                BLOCK_SIZE_CHUNKS,
                Default::default(),
            ))
            .await?;
        TreeCacheSchemaJSON(&mut storage)
            .store_account_tree_cache(
                BlockNumber(block_number),
                serde_json::Value::default().to_string(),
            )
            .await?;
    }

    // Remove account tree cache for blocks with numbers greater than 2.
    TreeCacheSchemaJSON(&mut storage)
        .remove_old_account_tree_cache(BlockNumber(3))
        .await?;

    // Check that the account tree cache for block #3 is present, and for block #1 is not.
    assert!(TreeCacheSchemaJSON(&mut storage)
        .get_account_tree_cache_block(BlockNumber(3))
        .await?
        .is_some());
    assert!(TreeCacheSchemaJSON(&mut storage)
        .get_account_tree_cache_block(BlockNumber(1))
        .await?
        .is_none());

    Ok(())
}

/// Check that account tree cache is removed correctly.
#[db_test]
async fn test_remove_new_account_tree_cache_json(
    mut storage: StorageProcessor<'_>,
) -> QueryResult<()> {
    // Insert account tree cache for 5 blocks.
    for block_number in 1..=5 {
        BlockSchema(&mut storage)
            .save_full_block(gen_sample_block(
                BlockNumber(block_number),
                BLOCK_SIZE_CHUNKS,
                Default::default(),
            ))
            .await?;
        TreeCacheSchemaJSON(&mut storage)
            .store_account_tree_cache(
                BlockNumber(block_number),
                serde_json::Value::default().to_string(),
            )
            .await?;
    }

    // Remove account tree cache for blocks with numbers greater than 2.
    TreeCacheSchemaJSON(&mut storage)
        .remove_new_account_tree_cache(BlockNumber(2))
        .await?;

    // Check if account tree cache for the 2nd block is present, and for the 3rd is not.
    assert!(TreeCacheSchemaJSON(&mut storage)
        .get_account_tree_cache_block(BlockNumber(2))
        .await?
        .is_some());
    assert!(TreeCacheSchemaJSON(&mut storage)
        .get_account_tree_cache_block(BlockNumber(3))
        .await?
        .is_none());

    Ok(())
}

/// Check that account tree cache is removed correctly.
#[db_test]
async fn test_remove_old_account_tree_cache_bincode(
    mut storage: StorageProcessor<'_>,
) -> QueryResult<()> {
    // Insert account tree cache for 5 blocks.
    for block_number in 1..=5 {
        BlockSchema(&mut storage)
            .save_full_block(gen_sample_block(
                BlockNumber(block_number),
                BLOCK_SIZE_CHUNKS,
                Default::default(),
            ))
            .await?;
        TreeCacheSchemaBincode(&mut storage)
            .store_account_tree_cache(BlockNumber(block_number), vec![1u8, 2, 3])
            .await?;
    }

    // Remove account tree cache for blocks with numbers greater than 2.
    TreeCacheSchemaBincode(&mut storage)
        .remove_old_account_tree_cache(BlockNumber(3))
        .await?;

    // Check that the account tree cache for block #3 is present, and for block #1 is not.
    assert!(TreeCacheSchemaBincode(&mut storage)
        .get_account_tree_cache_block(BlockNumber(3))
        .await?
        .is_some());
    assert!(TreeCacheSchemaBincode(&mut storage)
        .get_account_tree_cache_block(BlockNumber(1))
        .await?
        .is_none());

    Ok(())
}

/// Check that account tree cache is removed correctly.
#[db_test]
async fn test_remove_new_account_tree_cache_bincode(
    mut storage: StorageProcessor<'_>,
) -> QueryResult<()> {
    // Insert account tree cache for 5 blocks.
    for block_number in 1..=5 {
        BlockSchema(&mut storage)
            .save_full_block(gen_sample_block(
                BlockNumber(block_number),
                BLOCK_SIZE_CHUNKS,
                Default::default(),
            ))
            .await?;
        TreeCacheSchemaBincode(&mut storage)
            .store_account_tree_cache(BlockNumber(block_number), vec![1u8, 2, 3])
            .await?;
    }

    // Remove account tree cache for blocks with numbers greater than 2.
    TreeCacheSchemaBincode(&mut storage)
        .remove_new_account_tree_cache(BlockNumber(2))
        .await?;

    // Check if account tree cache for the 2nd block is present, and for the 3rd is not.
    assert!(TreeCacheSchemaBincode(&mut storage)
        .get_account_tree_cache_block(BlockNumber(2))
        .await?
        .is_some());
    assert!(TreeCacheSchemaBincode(&mut storage)
        .get_account_tree_cache_block(BlockNumber(3))
        .await?
        .is_none());

    Ok(())
}

/// Check that methods work correctly if there are both types of cache in the database.
#[db_test]
async fn different_cache_types_interoperability(
    mut storage: StorageProcessor<'_>,
) -> QueryResult<()> {
    // Insert JSON account tree cache for 5 blocks, but insert binary cache only until 4th block.
    for block_number in 1..=5 {
        BlockSchema(&mut storage)
            .save_full_block(gen_sample_block(
                BlockNumber(block_number),
                BLOCK_SIZE_CHUNKS,
                Default::default(),
            ))
            .await?;
        TreeCacheSchemaJSON(&mut storage)
            .store_account_tree_cache(
                BlockNumber(block_number),
                serde_json::Value::default().to_string(),
            )
            .await?;

        if block_number != 5 {
            TreeCacheSchemaBincode(&mut storage)
                .store_account_tree_cache(BlockNumber(block_number), vec![1, 2, 3])
                .await?;
        }
    }

    // Check that JSON corresponds to the 5th block and bincode to the 4th.
    let last_json_cache = TreeCacheSchemaJSON(&mut storage)
        .get_account_tree_cache()
        .await?
        .unwrap()
        .0;
    let last_bincode_cache = TreeCacheSchemaBincode(&mut storage)
        .get_account_tree_cache()
        .await?
        .unwrap()
        .0;
    assert_eq!(last_json_cache, BlockNumber(5));
    assert_eq!(last_bincode_cache, BlockNumber(4));

    // The same must be true for just getting the block number.
    assert_eq!(
        TreeCacheSchemaJSON(&mut storage)
            .get_last_block_with_account_tree_cache()
            .await?
            .unwrap(),
        BlockNumber(5)
    );
    assert_eq!(
        TreeCacheSchemaBincode(&mut storage)
            .get_last_block_with_account_tree_cache()
            .await?
            .unwrap(),
        BlockNumber(4)
    );

    // Check that resolving cache by number works as expected.

    // For JSON both blocks 4 and 5 should be stored.
    assert!(TreeCacheSchemaJSON(&mut storage)
        .get_account_tree_cache_block(BlockNumber(4))
        .await?
        .is_some());
    assert!(TreeCacheSchemaJSON(&mut storage)
        .get_account_tree_cache_block(BlockNumber(5))
        .await?
        .is_some());

    // For bincode only block 4 should be available
    assert!(TreeCacheSchemaBincode(&mut storage)
        .get_account_tree_cache_block(BlockNumber(4))
        .await?
        .is_some());
    assert!(TreeCacheSchemaBincode(&mut storage)
        .get_account_tree_cache_block(BlockNumber(5))
        .await?
        .is_none());

    // Let's store the 5th block for bincode and check last blocks again.
    TreeCacheSchemaBincode(&mut storage)
        .store_account_tree_cache(BlockNumber(5), vec![1, 2, 3])
        .await?;
    let last_json_cache = TreeCacheSchemaJSON(&mut storage)
        .get_account_tree_cache()
        .await?
        .unwrap()
        .0;
    let last_bincode_cache = TreeCacheSchemaBincode(&mut storage)
        .get_account_tree_cache()
        .await?
        .unwrap()
        .0;
    assert_eq!(last_json_cache, BlockNumber(5));
    assert_eq!(last_bincode_cache, BlockNumber(5));

    // The same must be true for just getting the block number.
    assert_eq!(
        TreeCacheSchemaJSON(&mut storage)
            .get_last_block_with_account_tree_cache()
            .await?
            .unwrap(),
        BlockNumber(5)
    );
    assert_eq!(
        TreeCacheSchemaBincode(&mut storage)
            .get_last_block_with_account_tree_cache()
            .await?
            .unwrap(),
        BlockNumber(5)
    );

    Ok(())
}
