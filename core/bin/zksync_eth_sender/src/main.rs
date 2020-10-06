use zksync_config::ConfigurationOptions;
use zksync_eth_sender::start_eth_sender;
use zksync_storage::ConnectionPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `eth_sender` doesn't require many connections to the database.
    const ETH_SENDER_CONNECTION_POOL_SIZE: u32 = 2;

    let pool = ConnectionPool::new(Some(ETH_SENDER_CONNECTION_POOL_SIZE)).await;
    let config_options = ConfigurationOptions::from_env();

    start_eth_sender(pool, config_options).await?;

    Ok(())
}
