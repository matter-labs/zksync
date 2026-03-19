use crate::zksync_tree::restore_tree;

pub fn run_restore_tree_from_db() -> anyhow::Result<()> {
    println!("Restoring ZKSYNC Merkle tree from the verified database state...");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let mut storage = zksync_storage::StorageProcessor::establish_connection()
            .await
            .expect("Failed to connect to the database");
        restore_from_verified_db(&mut storage).await;
    });
    Ok(())
}

/// Restores the account tree from the verified database state.
/// # Arguments
/// * `storage` - Mutable reference to the storage processor
async fn restore_from_verified_db(storage: &mut zksync_storage::StorageProcessor<'_>) {
    let (last_block, account_map) = storage
        .chain()
        .state_schema()
        .load_verified_state()
        .await
        .expect("There are no last verified state in storage");
    let account_tree = restore_tree(account_map);
    println!("Restoring tree to block number: {}", last_block.0);
    println!("Restored tree root hash: {}", account_tree.root_hash());
}
