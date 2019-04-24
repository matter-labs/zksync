use std::rc::Rc;
use web3::futures::{Future, Stream};
use web3::types::{Address, FilterBuilder, H256};
use tokio_core::reactor::Core;
use blocks::{BlockType, LogBlockData};
use helpers::*;

pub struct EventsFranklin {
    pub committed_blocks: Vec<LogBlockData>,
    pub verified_blocks: Vec<LogBlockData>,
    pub network: InfuraEndpoint
}

impl EventsFranklin {
    pub fn new(network: InfuraEndpoint) -> Self {
        let this = Self {
            committed_blocks: vec![],
            verified_blocks: vec![],
            network: network
        };
        this
    }

    pub fn subscribe_on_network(network: InfuraEndpoint) -> Self {
        let mut this = EventsFranklin::new(network);
        this.subscribe_to_logs();
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

    fn subscribe_to_logs(&mut self) {
        // Websocket Endpoint
        let enpoint = match self.network.clone() {
            InfuraEndpoint::Mainnet => "wss://mainnet.infura.io/ws",
            InfuraEndpoint::Rinkeby => "wss://rinkeby.infura.io/ws",
        };

        // Contract address
        let franklin_address: Address = match self.network.clone() {
            InfuraEndpoint::Mainnet => "fddb8167fef957f7cc72686094fac1d31be5ecfe",
            InfuraEndpoint::Rinkeby => "fddb8167fef957f7cc72686094fac1d31be5ecfe",
        }.parse().unwrap();

        // Get topic keccak hash
        let block_verified_topic = "BlockVerified(uint32)";
        let block_committed_topic = "BlockCommitted(uint32)";
        let block_verified_topic_h256: web3::types::H256 = get_topic_keccak_hash(block_verified_topic);
        let block_committed_topic_h256: web3::types::H256 = get_topic_keccak_hash(block_committed_topic);

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
                                // let result = self.check_committed_block_with_same_number_as_verified(&block);
                                // println!("Block exists: {:?}", result);
                                // let tx = result.unwrap().clone().transaction_hash;
                                // println!("--- Starting getting tx");
                                // let data = get_transaction_receipt(InfuraEndpoint::Rinkeby, &tx);
                                // println!("TX data committed: {:?}", data);
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




