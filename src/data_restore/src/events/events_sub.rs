use std::rc::Rc;
use web3::futures;
use web3::futures::{Future, Stream};
use web3::types::{Address, FilterBuilder, H256};
use tiny_keccak::{keccak256};
use tokio_core::reactor::Core;
use std::cmp::Ordering;

#[derive(Debug, Copy, Clone)]
pub enum InfuraEndpoint {
    Mainnet,
    Rinkeby
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BlockType {
    Committed,
    Verified,
    Unknown
}

#[derive(Debug, Copy, Clone, Eq)]
pub struct LogBlockData {
    pub block_num: H256,
    pub transaction_hash: H256,
    pub block_type: BlockType
}

impl PartialOrd for LogBlockData {
    fn partial_cmp(&self, other: &LogBlockData) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LogBlockData {
    fn cmp(&self, other: &LogBlockData) -> Ordering {
        self.block_num.cmp(&other.block_num)
    }
}

impl PartialEq for LogBlockData {
    fn eq(&self, other: &LogBlockData) -> bool {
        self.block_num == other.block_num
    }
}

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

    pub fn check_committed_block_with_same_number_as_verified(&self, verified_block: &LogBlockData) -> Option<&LogBlockData> {
        let committed_blocks_iter = &mut self.committed_blocks.iter();
        let committed_block = committed_blocks_iter.find(|&&x| x.block_num == verified_block.block_num);
        return committed_block
    }

    pub fn get_committed_blocks(&self) -> Vec<LogBlockData> {
        self.committed_blocks.clone()
    }

    pub fn get_verified_blocks(&self) -> Vec<LogBlockData> {
        self.verified_blocks.clone()
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
        let block_verified_topic_h256: web3::types::H256 = self.get_topic_keccak_hash(block_verified_topic);
        let block_committed_topic_h256: web3::types::H256 = self.get_topic_keccak_hash(block_committed_topic);

        let topics_vec_h256: Vec<web3::types::H256> = vec![block_verified_topic_h256, block_committed_topic_h256];

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
        println!("subscribing to franklin logs {:?} {:?}...", block_verified_topic, block_committed_topic);

        let filter = FilterBuilder::default()
            .address(vec![franklin_address])
            .from_block(franklin_genesis_block.into())
            .topics(
                Some(topics_vec_h256),
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
                    
                    // Form block
                    let mut block: LogBlockData = LogBlockData {
                        block_num: H256::zero(),
                        transaction_hash : H256::zero(),
                        block_type: BlockType::Unknown
                    };
                    // Log data
                    let tx_hash = log.transaction_hash;
                    let topic = log.topics[0];
                    let block_num = log.topics[1];

                    match tx_hash {
                        Some(hash) => {
                            block.block_num = block_num;
                            block.transaction_hash = hash;

                            if topic == block_verified_topic_h256 {
                                println!("-");
                                println!("Verified");
                                block.block_type = BlockType::Verified;
                                self.verified_blocks.push(block);
                                let result = self.check_committed_block_with_same_number_as_verified(&block);
                                
                                println!("Block exists: {:?}", result);
                            } else if topic == block_committed_topic_h256 {
                                println!("-");
                                println!("Committed");
                                block.block_type = BlockType::Committed;
                                self.committed_blocks.push(block);
                            }
                            println!("Verified blocks in storage: {:?}", self.verified_blocks);
                            println!("Committed blocks in storage: {:?}", self.committed_blocks);
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


