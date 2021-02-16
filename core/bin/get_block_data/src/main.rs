use zksync_storage::ConnectionPool;
use zksync_types::BlockNumber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let connection_pool = ConnectionPool::new(Some(1));
    let mut storage = connection_pool.access_storage().await?;
    let block = storage
        .chain()
        .block_schema()
        .get_block(BlockNumber(5))
        .await?
        .unwrap();
    println!("{:?}", block);
    println!("{:?}", block.get_onchain_operations_block_info().1);

    Ok(())
}
