use futures::{channel::mpsc, SinkExt};
use std::time::Duration;
use tokio::{runtime::Runtime, time};
use zksync_config::configs::ZkSyncConfig;
use zksync_contracts::zksync_contract;
use zksync_core::eth_watch::{DBStorage, EthHttpClient, EthWatch, EthWatchRequest};
use zksync_eth_client::{ETHDirectClient, EthereumGateway, MultiplexerEthereumClient};
use zksync_eth_signer::PrivateKeySigner;
use zksync_storage::ConnectionPool;

fn main() {
    let mut main_runtime = Runtime::new().expect("main runtime start");

    env_logger::init();
    log::info!("ETH watcher started");
    let config = ZkSyncConfig::from_env();
    let transport = web3::transports::Http::new(&config.eth_client.web3_url).unwrap();
    let client = EthereumGateway::Multiplexed(MultiplexerEthereumClient::new().add_client(
        "Infura".to_string(),
        ETHDirectClient::new(
            transport,
            zksync_contract(),
            config.eth_sender.sender.operator_commit_eth_addr,
            PrivateKeySigner::new(config.eth_sender.sender.operator_private_key),
            config.contracts.contract_addr,
            config.eth_client.chain_id,
            config.eth_client.gas_price_factor,
        ),
    ));

    let (eth_req_sender, eth_req_receiver) = mpsc::channel(256);

    let db_pool = ConnectionPool::new(Some(config.db.pool_size as u32));

    let storage = DBStorage::new(db_pool);
    let eth_client = EthHttpClient::new(client, config.contracts.contract_addr);
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
