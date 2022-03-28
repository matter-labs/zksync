use structopt::StructOpt;
use zksync_crypto::merkle_tree::parallel_smt::SparseMerkleTreeSerializableCacheBN256;
use zksync_storage::StorageProcessor;
use zksync_types::BlockNumber;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "tree_cache_updater",
    about = "Tool to migrate server tree caches to the binary format."
)]
struct Opt {
    /// Maximum amount of blocks to convert.
    #[structopt(long)]
    max_blocks: usize,
    /// Whether to remove all the old JSON caches or not.
    #[structopt(long)]
    clear_old: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();

    let mut storage = StorageProcessor::establish_connection().await?;
    let mut transaction = storage.start_transaction().await?;

    let max_block = transaction
        .chain()
        .block_schema()
        .get_last_saved_block()
        .await?;

    let min_block = std::cmp::max(max_block.0.saturating_sub(opt.max_blocks as u32), 1); // We can't go below the 1st block.

    // Go through the suggested blocks range. For each block in this range, if the cachce exists, we will load it, convert to the bincode cache,
    // and store to the binary schema.
    for block in min_block..(max_block.0) {
        if let Some(cache) = transaction
            .chain()
            .tree_cache_schema_json()
            .get_account_tree_cache_block(BlockNumber(block))
            .await?
        {
            let cache: SparseMerkleTreeSerializableCacheBN256 = serde_json::from_value(cache)?;
            let binary_cache = cache.encode_bincode();
            transaction
                .chain()
                .tree_cache_schema_bincode()
                .store_account_tree_cache(BlockNumber(block), binary_cache)
                .await?;
        }
    }

    // We've processed all the blocks. Now, if user requested, we'll remove all the old caches.
    if opt.clear_old {
        // BlockNumber(0) because range is not inclusive. Everything starting from block 1 will be removed.
        transaction
            .chain()
            .tree_cache_schema_json()
            .remove_new_account_tree_cache(BlockNumber(0))
            .await?;
    }

    transaction.commit().await?;

    Ok(())
}
