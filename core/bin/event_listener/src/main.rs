use zksync_event_listener::EventListener;
// use zksync_config::ZkSyncConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // let config = ZkSyncConfig::from_env();
    // let mut listener = EventListener::new(config).await?;
    let mut listener = EventListener::new().await?;

    listener.run().await?;

    Ok(())
}
