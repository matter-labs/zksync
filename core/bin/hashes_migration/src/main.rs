#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut storage = zksync_storage::StorageProcessor::establish_connection().await?;
    storage
        .chain()
        .operations_schema()
        .calculate_priority_ops_hashes()
        .await?;
    println!("Priority op hashes were calculated");

    storage
        .chain()
        .operations_schema()
        .calculate_batch_hashes()
        .await?;

    println!("Tx hashes for priority ops and batches are successfully calculated");
    Ok(())
}
