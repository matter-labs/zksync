use futures::{channel::mpsc, SinkExt};
use std::time::Duration;
use tokio::{runtime::Runtime, time};
use zksync_core::eth_watch::{EthWatch, EthWatchRequest};
use zksync_storage::ConnectionPool;

fn main() {
    let mut main_runtime = Runtime::new().expect("main runtime start");

    env_logger::init();
    log::info!("ETH watcher started");
    let web3_url = std::env::var("WEB3_URL").expect("WEB3_URL env var not found");
    let contract_address = std::env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env var not found")
        [2..]
        .parse()
        .expect("Failed to parse CONTRACT_ADDR");
    let transport = web3::transports::Http::new(&web3_url).unwrap();
    let web3 = web3::Web3::new(transport);

    let (eth_req_sender, eth_req_receiver) = mpsc::channel(256);

    let db_pool = main_runtime.block_on(async { ConnectionPool::new(None).await });

    let watcher = EthWatch::new(web3, contract_address, 0, eth_req_receiver, db_pool);

    main_runtime.spawn(watcher.run());
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
