use zksync_storage::StorageProcessor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut storage = StorageProcessor::establish_connection().await?;
    storage
        .chain()
        .state_schema()
        .mint_nft_updates_set_nonces()
        .await?;

    Ok(())
}
