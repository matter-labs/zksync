// Built-in deps
use std::time::Instant;
// External imports
// Workspace imports
use zksync_types::BlockNumber;
// Local imports
use super::records::AccountTreeCache;
use crate::{QueryResult, StorageProcessor};

/// Tree cache schema contains methods to store/load Merkle tree cache.
///
/// This schema is used to interact with caches encoded as *binary* data (using `bincode` protocol).
#[derive(Debug)]
pub struct TreeCacheSchemaBincode<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> TreeCacheSchemaBincode<'a, 'c> {
    /// Stores account tree cache for a block.
    /// Expects `tree_cache` to be a byte sequence encoded according to the `bincode` protocol.
    pub async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        tree_cache: Vec<u8>,
    ) -> QueryResult<()> {
        let start = Instant::now();
        if *block == 0 {
            return Ok(());
        }

        sqlx::query!(
            "
            INSERT INTO account_tree_cache_new (block, tree_cache_binary)
            VALUES ($1, $2)
            ON CONFLICT (block)
            DO NOTHING
            ",
            *block as i64,
            tree_cache,
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.tree_cache.bincode.store_account_tree_cache",
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

        let last_block_with_cache = sqlx::query!(
            r#"
                SELECT MAX(block) FROM (
                    SELECT MAX(block) as block FROM account_tree_cache WHERE tree_cache_binary IS NOT NULL
                    UNION ALL
                    SELECT MAX(block) as block FROM account_tree_cache_new 
                ) AS max_block
            "#
        )
        .fetch_one(self.0.conn())
        .await?
        .max;

        metrics::histogram!(
            "sql.chain.tree_cache.bincode.get_last_block_with_account_tree_cache",
            start.elapsed()
        );
        Ok(last_block_with_cache.map(|block| BlockNumber(block as u32)))
    }

    /// Gets the latest stored account tree cache encoded in binary.
    /// Returns `None` if there are no caches in the database or they are encoded in JSON.
    /// Returns the block number and associated cache otherwise.
    pub async fn get_account_tree_cache(&mut self) -> QueryResult<Option<(BlockNumber, Vec<u8>)>> {
        let start = Instant::now();

        let last_block = self.get_last_block_with_account_tree_cache().await?;
        let account_tree_cache = if let Some(last_block) = last_block {
            Some((
                last_block,
                self.get_account_tree_cache_block(last_block).await?.expect(
                    "Must be 'some' because we checked that there is a cache for this block",
                ),
            ))
        } else {
            None
        };
        metrics::histogram!(
            "sql.chain.tree_cache.bincode.get_account_tree_cache",
            start.elapsed()
        );
        Ok(account_tree_cache)
    }

    /// Gets stored account tree cache for a certain block.
    /// Returns `None` if there is no cache for requested block or it's encoded in JSON.
    pub async fn get_account_tree_cache_block(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Option<Vec<u8>>> {
        let start = Instant::now();
        let account_tree_cache = sqlx::query_as!(
            AccountTreeCache,
            "
            SELECT * FROM account_tree_cache_new
            WHERE block = $1
            ",
            *block as i64
        )
        .fetch_optional(self.0.conn())
        .await?;
        if let Some(account_tree_cache) = account_tree_cache {
            return Ok(Some(
                account_tree_cache
                    .tree_cache_binary
                    .expect("Must be 'some' because of condition in query"),
            ));
        }

        let account_tree_cache = sqlx::query_as!(
            AccountTreeCache,
            "
            SELECT block, tree_cache_binary FROM account_tree_cache
            WHERE block = $1 AND tree_cache_binary IS NOT NULL
            ",
            *block as i64
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.tree_cache.bincode.get_account_tree_cache_block",
            start.elapsed()
        );
        Ok(account_tree_cache.map(|w| {
            w.tree_cache_binary
                .expect("Must be 'some' because of condition in query")
        }))
    }

    // Removes account tree cache for blocks with number greater than `last_block`
    pub async fn remove_new_account_tree_cache(
        &mut self,
        last_block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM account_tree_cache_new WHERE block > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.tree_cache.bincode.remove_new_account_tree_cache",
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

        loop {
            let res = sqlx::query!(
                "DELETE 
                FROM account_tree_cache_new
                WHERE block < $1
                AND ctid IN
                (
                    SELECT ctid
                    FROM account_tree_cache_new
                    WHERE block < $1
                    LIMIT 2
                )
              returning true 
            ",
                *last_block as i64
            )
            .fetch_optional(self.0.conn())
            .await?;
            if res.is_none() {
                break;
            }
        }

        metrics::histogram!(
            "sql.chain.tree_cache.bincode.remove_old_account_tree_cache",
            start.elapsed()
        );
        Ok(())
    }
}
