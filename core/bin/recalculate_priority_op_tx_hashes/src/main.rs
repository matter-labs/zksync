#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut storage = zksync_storage::StorageProcessor::establish_connection().await?;
    storage
        .chain()
        .operations_schema()
        .recalculate_tx_hashes_to_existing_priority_ops()
        .await?;

    println!("Tx hashes for priority ops are successfully recalculated");
    Ok(())
}
