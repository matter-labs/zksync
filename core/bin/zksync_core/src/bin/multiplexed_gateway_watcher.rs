use zksync_config::ZkSyncConfig;
use zksync_eth_client::EthereumGateway;
use zksync_gateway_watcher::MultiplexedGatewayWatcher;

#[tokio::main]
async fn main() {
    vlog::init();
    let config = ZkSyncConfig::from_env();

    MultiplexedGatewayWatcher::new(
        EthereumGateway::from_config(&config),
        config.gateway_watcher.check_interval(),
        config.gateway_watcher.retry_delay(),
        config.gateway_watcher.request_timeout(),
        Some(config.gateway_watcher.request_per_task_limit()),
        Some(config.gateway_watcher.task_limit()),
    )
    .run()
    .await;
}
