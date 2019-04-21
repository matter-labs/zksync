use env_logger;
use std::rc::Rc;
use web3::contract;
use web3::futures::{Future, Stream};
use web3::types::{Address, FilterBuilder, U256, H256};

pub const ETH_SUBSCRIPTION: &'static str = r#"{"jsonrpc":"2.0", "id": 1, "method": "eth_subscribe", "params": ["logs", {"address": "0xfddb8167fef957f7cc72686094fac1d31be5ecfe", "topics": ["0xca558d7524956f89ce1ec833efe8a265ed2b1e92b20ff4fe2fb87fb1a042e524"]}]}"#;

pub enum InfuraEndpoint {
    Mainnet,
    Rinkeby
}

fn subscribe_franklin_logs(
    franklin_address: Address,
    franklin_genesis_block: u64,
    topic: Vec<web3::types::H256>,
    w3: Rc<web3::Web3<web3::transports::WebSocket>>,
) -> impl Future<Item = (), Error = ()> {
    println!("subscribing to franklin logs...");

    let filter = FilterBuilder::default()
        .address(vec![franklin_address])
        .from_block(franklin_genesis_block.into())
        .topics(
            Some(topic),
            None,
            None,
            None,
        )
        .build();

    w3.eth_subscribe()
        .subscribe_logs(filter)
        .and_then(|sub| {
            sub.for_each(|log| {
                println!("got block verified log from subscription: {:?}", log);
                Ok(())
            })
        })
        .map_err(|e| eprintln!("franklin log err: {}", e))
}

pub fn subscribe_to_verified_blocks(on: InfuraEndpoint) {
    let enpoint = match on {
        InfuraEndpoint::Mainnet => "wss://mainnet.infura.io/ws",
        InfuraEndpoint::Rinkeby => "wss://rinkeby.infura.io/ws",
    };

    let franklin_address: Address = match on {
        InfuraEndpoint::Mainnet => "0xFddB8167fEf957F7cC72686094faC1D31BE5ECFE",
        InfuraEndpoint::Rinkeby => "0xFddB8167fEf957F7cC72686094faC1D31BE5ECFE",
    }.parse().unwrap();

    let block_verified_topic: Vec<web3::types::H256> = vec!["0xca558d7524956f89ce1ec833efe8a265ed2b1e92b20ff4fe2fb87fb1a042e524".into()];

    let franklin_genesis_block = 0;

    env_logger::init();

    let mut eloop = tokio_core::reactor::Core::new().unwrap();
    let handle = eloop.handle();
    let w3 = Rc::new(web3::Web3::new(
        web3::transports::WebSocket::with_event_loop(enpoint, &eloop.handle())
            .unwrap(),
    ));

    let subscribe_franklin_logs_future = subscribe_franklin_logs(franklin_address, franklin_genesis_block, block_verified_topic, w3.clone());

    if let Err(_err) = eloop.run(subscribe_franklin_logs_future) {
        eprintln!("ERROR");
    }
}