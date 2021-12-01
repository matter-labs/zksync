use zksync_config::{ContractsConfig, ETHClientConfig, ETHSenderConfig, GatewayWatcherConfig};
use zksync_eth_client::EthereumGateway;
use zksync_gateway_watcher::MultiplexedGatewayWatcher;

#[tokio::main]
async fn main() {
    vlog::init();
    let contracts = ContractsConfig::from_env();
    let eth_client_config = ETHClientConfig::from_env();
    let eth_sender_config = ETHSenderConfig::from_env();
    let eth_watcher_config = GatewayWatcherConfig::from_env();

    MultiplexedGatewayWatcher::new(
        EthereumGateway::from_config(
            &eth_client_config,
            &eth_sender_config,
            contracts.contract_addr,
        ),
        eth_watcher_config.check_interval(),
        eth_watcher_config.retry_delay(),
        eth_watcher_config.request_timeout(),
        Some(eth_watcher_config.request_per_task_limit()),
        Some(eth_watcher_config.task_limit()),
    )
    .run()
    .await;
}
