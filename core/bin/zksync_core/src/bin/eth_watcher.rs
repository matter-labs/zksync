use std::time::Duration;

use futures::{channel::mpsc, SinkExt};
use tokio::{runtime::Runtime, time};

use zksync_config::ZkSyncConfig;
use zksync_core::eth_watch::{EthHttpClient, EthWatch, EthWatchRequest};
use zksync_eth_client::EthereumGateway;

fn main() {
    let mut main_runtime = Runtime::new().expect("main runtime start");

    let _sentry_guard = vlog::init();
    vlog::info!("ETH watcher started");

    let config = ZkSyncConfig::from_env();
    let client = EthereumGateway::from_config(&config);

    let (eth_req_sender, eth_req_receiver) = mpsc::channel(256);

    let eth_client = EthHttpClient::new(
        client,
        config.contracts.contract_addr,
        config.contracts.governance_addr,
    );
    let watcher = EthWatch::new(eth_client, 0);

    main_runtime.spawn(watcher.run(eth_req_receiver));
    main_runtime.block_on(async move {
        let mut timer = time::interval(Duration::from_secs(1));

        loop {
            timer.tick().await;
            eth_req_sender
                .clone()
                .send(EthWatchRequest::PollETHNode)
                .await
                .expect("ETH watch receiver dropped");
        }
    });
}
