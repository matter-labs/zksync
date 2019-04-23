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

#[derive(Debug)]
pub struct LogBlockData {
    pub block_num: H256,
    pub transaction_hash: H256
}

#[derive(Debug)]
pub struct EventsFranklin {
    pub committed_blocks: Vec<LogBlockData>,
    pub verified_blocks: Vec<LogBlockData>,
}

impl EventsFranklin {
    pub fn new() -> Self {
        let this = Self {
            committed_blocks: vec![],
            verified_blocks: vec![]
        };
        this
    }

    pub fn get_committed_blocks(&self) -> Vec<LogBlockData> {
        let mut blocks: Vec<LogBlockData> = vec![];
        for block in self.committed_blocks.iter() {
            let copied_block = LogBlockData { 
                block_num: block.block_num.clone(),
                transaction_hash: block.transaction_hash.clone()
            };
            blocks.push(copied_block);
        }
        blocks
    }

    pub fn get_verified_blocks(&self) -> Vec<LogBlockData> {
        let mut blocks: Vec<LogBlockData> = vec![];
        for block in self.verified_blocks.iter() {
            let verified_block = LogBlockData { 
                block_num: block.block_num.clone(),
                transaction_hash: block.transaction_hash.clone()
            };
            blocks.push(verified_block);
        }
        blocks
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
        let mut eloop = Core::new().unwrap();
        let handle = eloop.handle();
        let w3 = Rc::new(web3::Web3::new(
            web3::transports::WebSocket::with_event_loop(enpoint, &handle)
                .unwrap(),
        ));

        // Subscription
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

        let future = w3.eth_subscribe()
            .subscribe_logs(filter)
            .and_then(|sub| {
                sub.for_each(|log| {
                    println!("---");
                    println!("got log from subscription: {:?}", log);
                    let mut block: LogBlockData = LogBlockData {
                        block_num: H256::zero(),
                        transaction_hash : H256::zero()
                    };
                    match log.transaction_hash {
                        Some(hash) => {
                            block.block_num = log.topics[1];
                            block.transaction_hash = hash;
                            self.committed_blocks.push(block);
                            println!("-");
                            println!("Blocks in storage: {:?}", self.committed_blocks);
                        },
                        None    => println!("No tx hash"),
                    };
                    Ok(())
                })
            })
            .map_err(|e| eprintln!("franklin log err: {}", e));

        // Run eloop
        if let Err(_err) = eloop.run(future) {
            eprintln!("ERROR");
        }
    }
}


