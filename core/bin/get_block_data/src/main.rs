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
    for op in &block.block_transactions {
        if let Some(executed_op) = op.get_executed_op() {
            if executed_op.is_processable_onchain_operation() {
                println!("{:?}", executed_op);
                println!("{:?}", executed_op.public_data());
                // processable_ops_hash =
                //     [&processable_ops_hash, executed_op.public_data().as_slice()]
                //         .concat()
                //         .keccak256();
            }
        }
    }

    Ok(())
}
