use futures::{channel::mpsc, SinkExt};
use std::time::Duration;
use tokio::{runtime::Runtime, time};
use zksync_contracts::zksync_contract;
use zksync_core::eth_watch::{DBStorage, EthHttpClient, EthWatch, EthWatchRequest};
use zksync_eth_client::{ETHDirectClient, EthereumGateway, MultiplexerEthereumClient};
use zksync_eth_signer::PrivateKeySigner;
use zksync_storage::ConnectionPool;

fn main() {
    let mut main_runtime = Runtime::new().expect("main runtime start");

    env_logger::init();
    log::info!("ETH watcher started");
    let web3_url = std::env::var("WEB3_URL").expect("WEB3_URL env var not found");
    let operator_eth_addr = std::env::var("OPERATOR_FEE_ETH_ADDRESS")
        .expect("OPERATOR_FEE_ETH_ADDRESS env var not found")[2..]
        .parse()
        .expect("Failed to parse OPERATOR_FEE_ETH_ADDRESS");
    let contract_address = std::env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env var not found")
        [2..]
        .parse()
        .expect("Failed to parse CONTRACT_ADDR");
    let transport = web3::transports::Http::new(&web3_url).unwrap();
    let eth_signer = PrivateKeySigner::new(Default::default());
    // TODO find pk

    let client = EthereumGateway::Multiplexed(MultiplexerEthereumClient::new().add_client(
        "Infura".to_string(),
        ETHDirectClient::new(
            transport,
            zksync_contract(),
            operator_eth_addr,
            eth_signer,
            contract_address,
            1, // TODO find chain id
            1.5f64,
        ),
    ));

    let (eth_req_sender, eth_req_receiver) = mpsc::channel(256);

    let db_pool = ConnectionPool::new(None);
    let eth_client = EthHttpClient::new(client, contract_address);

    let storage = DBStorage::new(db_pool);

    let watcher = EthWatch::new(eth_client, storage, 0);

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
