use std::rc::Rc;
use web3::futures;
use web3::futures::{Future, Stream};
use web3::types::{Address, FilterBuilder, H256};
use tiny_keccak::{keccak256};
use tokio_core::reactor::Core;

pub enum InfuraEndpoint {
    Mainnet,
    Rinkeby
}

pub struct LogBlockData {
    block_num: u32,
    transaction_hash: String
}

pub struct EventsFranklin {
    committed_blocks: Vec<LogBlockData>,
    verified_blocks: Vec<LogBlockData>,
    eloop: Core,
}

impl EventsFranklin {
    pub fn new() -> Self {
        let this = Self {
            committed_blocks: vec![],
            verified_blocks: vec![],
            eloop: Core::new().unwrap(),
        };
        this
    }

    fn logs_subscription(
        &mut self, 
        franklin_address: Address,
        franklin_genesis_block: u64,
        topics: Vec<&str>,
        w3: Rc<web3::Web3<web3::transports::WebSocket>>,
    ) -> impl Future<Item = (), Error = ()> {
        println!("subscribing to franklin logs {:?}...", topics);

        let mut topics_vec: Vec<web3::types::H256> = vec![];
        for i in 0..topics.len() {
            let topic_h256: web3::types::H256 = self.get_topic_keccak_hash(topics[i]);
            topics_vec.push(topic_h256);
        }

        let filter = FilterBuilder::default()
            .address(vec![franklin_address])
            .from_block(franklin_genesis_block.into())
            .topics(
                Some(topics_vec),
                None,
                None,
                None,
            )
            .build();

        w3.eth_subscribe()
            .subscribe_logs(filter)
            .and_then(|sub| {
                sub.for_each(|log| {
                    println!("---");
                    println!("got log from subscription: {:?}", log);
                    Ok(())
                })
            })
            .map_err(|e| eprintln!("franklin log err: {}", e))
    }

    fn keccak256_hash(&mut self, bytes: &[u8]) -> Vec<u8> {
        keccak256(bytes).into_iter().cloned().collect()
    }

    fn get_topic_keccak_hash(&mut self, topic: &str) -> web3::types::H256 {
        let topic_data: Vec<u8> = From::from(topic);
        let topic_data_vec: &[u8] = topic_data.as_slice();
        let topic_keccak_data: Vec<u8> = self.keccak256_hash(topic_data_vec);
        let topic_keccak_data_vec: &[u8] = topic_keccak_data.as_slice();
        let topic_h256 = H256::from_slice(topic_keccak_data_vec);
        topic_h256
    }

    pub fn subscribe_to_logs(&mut self, on: InfuraEndpoint) {
        // Websocket Endpoint
        let enpoint = match on {
            InfuraEndpoint::Mainnet => "wss://mainnet.infura.io/ws",
            InfuraEndpoint::Rinkeby => "wss://rinkeby.infura.io/ws",
        };

        // Contract address
        let franklin_address: Address = match on {
            InfuraEndpoint::Mainnet => "fddb8167fef957f7cc72686094fac1d31be5ecfe",
            InfuraEndpoint::Rinkeby => "fddb8167fef957f7cc72686094fac1d31be5ecfe",
        }.parse().unwrap();

        // Get topic keccak hash
        let block_verified_topic = "BlockVerified(uint32)";
        let block_committed_topic = "BlockCommitted(uint32)";

        let topics: Vec<&str> = vec![block_committed_topic, block_verified_topic];

        // TODO: not working genesis block
        let franklin_genesis_block = 0;

        // Setup loop and web3
        let handle = self.eloop.handle();
        let w3 = Rc::new(web3::Web3::new(
            web3::transports::WebSocket::with_event_loop(enpoint, &handle)
                .unwrap(),
        ));

        // Subscription
        let subscribe_franklin_logs_future = self.logs_subscription(
            franklin_address,
            franklin_genesis_block,
            topics,
            w3.clone()
        );
        let all_futures = futures::future::lazy(|| {
            subscribe_franklin_logs_future
            // subscribe_franklin_block_committed_logs_future.join(subscribe_franklin_block_verified_logs_future)
        });

        // Run eloop
        if let Err(_err) = self.eloop.run(all_futures) {
            eprintln!("ERROR");
        }
    }
}



