// Built-in deps
use std::time::Instant;
// External imports
// Workspace imports
use zksync_types::BlockNumber;
// Local imports
use super::records::AccountTreeCacheJSON;
use crate::{QueryResult, StorageProcessor};

/// Tree cache schema contains methods to store/load Merkle tree cache.
///
/// This schema is used to interact with caches encoded as *JSON* data.
#[derive(Debug)]
pub struct TreeCacheSchemaJSON<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> TreeCacheSchemaJSON<'a, 'c> {
    /// Stores account tree cache for a block.
    /// Expects `tree_cache` to be a valid encoded JSON.
    pub async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        tree_cache: String,
    ) -> QueryResult<()> {
        let start = Instant::now();
        if *block == 0 {
            return Ok(());
        }

        sqlx::query!(
            "
            INSERT INTO account_tree_cache (block, tree_cache)
            VALUES ($1, $2)
            ON CONFLICT (block)
            DO UPDATE SET tree_cache = $2
            ",
            *block as i64,
            tree_cache,
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.tree_cache.json.store_account_tree_cache",
            start.elapsed()
        );
        Ok(())
    }

    /// Gets the number of the latest block that has a stored cache.
    /// Returns `None` if there are no caches in the database.
    pub async fn get_last_block_with_account_tree_cache(
        &mut self,
    ) -> QueryResult<Option<BlockNumber>> {
        let start = Instant::now();

        let last_block_with_cache =
            sqlx::query!("SELECT MAX(block) FROM account_tree_cache WHERE tree_cache IS NOT NULL")
                .fetch_one(self.0.conn())
                .await?
                .max;

        metrics::histogram!(
            "sql.chain.tree_cache.json.get_last_block_with_account_tree_cache",
            start.elapsed()
        );
        Ok(last_block_with_cache.map(|block| BlockNumber(block as u32)))
    }

    /// Gets the latest stored account tree cache encoded in JSON.
    /// Returns `None` if there are no caches in the database or only existing caches are binary.
    /// Returns the block number and associated cache otherwise.
    pub async fn get_account_tree_cache(
        &mut self,
    ) -> QueryResult<Option<(BlockNumber, serde_json::Value)>> {
        let start = Instant::now();
        let account_tree_cache = sqlx::query_as!(
            AccountTreeCacheJSON,
            "
            SELECT block, tree_cache FROM account_tree_cache
            WHERE tree_cache IS NOT NULL
            ORDER BY block DESC
            LIMIT 1
            ",
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.tree_cache.json.get_account_tree_cache",
            start.elapsed()
        );
        Ok(account_tree_cache.map(|w| {
            assert!(
                w.tree_cache.is_some(),
                "JSON schema was used to fetch from table without JSON data. Entry: {:?}",
                w
            );

            (
                BlockNumber(w.block as u32),
                serde_json::from_str(
                    &w.tree_cache
                        .expect("Must be 'some' because of condition in query"),
                )
                .expect("Failed to deserialize Account Tree Cache"),
            )
        }))
    }

    /// Gets stored account tree cache for a certain block.
    /// Returns `None` if there is no cache for requested block or it's encoded in binary.
    pub async fn get_account_tree_cache_block(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Option<serde_json::Value>> {
        let start = Instant::now();
        let account_tree_cache = sqlx::query_as!(
            AccountTreeCacheJSON,
            "
            SELECT block, tree_cache FROM account_tree_cache
            WHERE block = $1 AND tree_cache IS NOT NULL
            ",
            *block as i64
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.tree_cache.json.get_account_tree_cache_block",
            start.elapsed()
        );
        Ok(account_tree_cache.map(|w| {
            serde_json::from_str(
                &w.tree_cache
                    .expect("Must be 'some' because of condition in query"),
            )
            .expect("Failed to deserialize Account Tree Cache")
        }))
    }

    // Removes account tree cache for blocks with number greater than `last_block`
    pub async fn remove_new_account_tree_cache(
        &mut self,
        last_block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM account_tree_cache WHERE block > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.tree_cache.json.remove_new_account_tree_cache",
            start.elapsed()
        );
        Ok(())
    }

    // Removes account tree cache for blocks with number less than `last_block`
    pub async fn remove_old_account_tree_cache(
        &mut self,
        last_block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM account_tree_cache WHERE block < $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.tree_cache.json.remove_old_account_tree_cache",
            start.elapsed()
        );
        Ok(())
    }
}
