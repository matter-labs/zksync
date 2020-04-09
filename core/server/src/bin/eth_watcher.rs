use futures::{channel::mpsc, SinkExt};
use log::*;
use server::eth_watch::{EthWatch, EthWatchRequest};
use std::time::Duration;
use storage::ConnectionPool;
use tokio::{runtime::Runtime, time};

fn main() {
    let mut main_runtime = Runtime::new().expect("main runtime start");

    env_logger::init();
    info!("ETH watcher started");
    let web3_url = std::env::var("WEB3_URL").expect("WEB3_URL env var not found");
    let governance_addr = std::env::var("GOVERNANCE_ADDR").expect("GOVERNANCE_ADDR env not found")
        [2..]
        .parse()
        .expect("Failed to parse GOVERNANCE_ADDR");
    // let priority_queue_address = std::env::var("PRIORITY_QUEUE_ADDR")
    //     .expect("PRIORITY_QUEUE_ADDR env var not found")[2..]
    //     .parse()
    //     .expect("Failed to parse PRIORITY_QUEUE_ADDR");
    let contract_address = std::env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env var not found")
        [2..]
        .parse()
        .expect("Failed to parse CONTRACT_ADDR");
    let (web3_event_loop_handle, transport) = web3::transports::Http::new(&web3_url).unwrap();
    let web3 = web3::Web3::new(transport);

    let (eth_req_sender, eth_req_receiver) = mpsc::channel(256);

    let watcher = EthWatch::new(
        web3,
        web3_event_loop_handle,
        ConnectionPool::new(),
        governance_addr,
        //priority_queue_address,
        contract_address,
        0,
        eth_req_receiver,
    );

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
