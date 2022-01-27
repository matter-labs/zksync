use zksync_storage::StorageProcessor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut storage = StorageProcessor::establish_connection().await?;
    storage
        .chain()
        .operations_schema()
        .remove_outstanding_tx_filters()
        .await?;
    Ok(())
}
