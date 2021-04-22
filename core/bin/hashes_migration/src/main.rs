#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut storage = zksync_storage::StorageProcessor::establish_connection().await?;
    let mut transaction = storage.start_transaction().await?;
    transaction
        .chain()
        .operations_schema()
        .calculate_priority_ops_hashes()
        .await?;

    transaction
        .chain()
        .operations_schema()
        .calculate_batch_hashes()
        .await?;

    transaction.commit().await?;
    println!("Tx hashes for priority ops and batches are successfully calculated");
    Ok(())
}
