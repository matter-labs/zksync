use zksync_config::ZkSyncConfig;
use zksync_core::gateway_watcher::GatewayWatcher;
use zksync_eth_client::EthereumGateway;

#[tokio::main]
async fn main() {
    vlog::init();
    let config = ZkSyncConfig::from_env();

    GatewayWatcher::new(
        EthereumGateway::from_config(&config),
        Some(config.gateway_watcher.request_per_task_limit()),
        Some(config.gateway_watcher.task_limit()),
        config.gateway_watcher.check_interval(),
        config.gateway_watcher.request_timeout(),
        config.gateway_watcher.retry_delay(),
    )
    .run()
    .await;
}
