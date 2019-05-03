// use std::rc::Rc;

use web3::futures::Future;
use web3::types::{Log, Address, FilterBuilder, H256, U256, BlockNumber};
// use tokio_core::reactor::Core;
use ethabi::Contract;

use blocks::{BlockType, LogBlockData};
use helpers;
use helpers::InfuraEndpoint;

type ABI = (&'static [u8], &'static str);
type ComAndVerBlocksVecs = (Vec<LogBlockData>, Vec<LogBlockData>);
type BlockNumber256 = U256;

pub const PLASMA_TEST_ABI: ABI = (
    include_bytes!("../../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.abi"),
    include_str!("../../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.bin"),
);

pub const PLASMA_PROD_ABI: ABI = (
    include_bytes!("../../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.abi"),
    include_str!("../../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.bin"),
);

#[derive(Debug, Clone)]
pub struct BlockEventsFranklin {
    pub endpoint: InfuraEndpoint,
    pub http_endpoint_string: String,
    // pub ws_endpoint_string: String,
    pub franklin_abi: ABI,
    pub franklin_contract: Contract,
    pub franklin_contract_address: Address,
    pub committed_blocks: Vec<LogBlockData>,
    pub verified_blocks: Vec<LogBlockData>,
    pub last_watched_block_number: BlockNumber256,
}

// Set new
// Get last block
// Get blocks till last - delta, set last watching block
// Subscribe on new blocks
// New blocks -> last watching block ++
// Check if txs in last watching block
impl BlockEventsFranklin {
    pub fn new(network: InfuraEndpoint) -> Self {
        // let ws_infura_endpoint_str = match network {
        //     InfuraEndpoint::Mainnet => "wss://mainnet.infura.io/ws",
        //     InfuraEndpoint::Rinkeby => "wss://rinkeby.infura.io/ws",
        // };
        // let ws_infura_endpoint_string = String::from(ws_infura_endpoint_str);
        let http_infura_endpoint_str = match network {
            InfuraEndpoint::Mainnet => "https://mainnet.infura.io/",
            InfuraEndpoint::Rinkeby => "https://rinkeby.infura.io/",
        };
        let http_infura_endpoint_string = String::from(http_infura_endpoint_str);
        let address: Address = match network {
            InfuraEndpoint::Mainnet => "fddb8167fef957f7cc72686094fac1d31be5ecfe",
            InfuraEndpoint::Rinkeby => "fddb8167fef957f7cc72686094fac1d31be5ecfe",
        }.parse().unwrap();
        let abi: ABI = match network {
            InfuraEndpoint::Mainnet => PLASMA_PROD_ABI,
            InfuraEndpoint::Rinkeby => PLASMA_TEST_ABI,
        };
        let contract = ethabi::Contract::load(abi.0).unwrap();
        let this = Self {
            // ws_endpoint_string: ws_infura_endpoint_string,
            endpoint: network,
            http_endpoint_string: http_infura_endpoint_string,
            franklin_abi: abi,
            franklin_contract: contract,
            franklin_contract_address: address,
            committed_blocks: vec![],
            verified_blocks: vec![],
            last_watched_block_number: U256::from(0),
        };
        this
    }

    pub fn get_past_state_from_genesis_with_blocks_delta(network: InfuraEndpoint, genesis_block: U256, blocks_delta: U256) -> Result<Self, String> {
        let mut this = BlockEventsFranklin::new(network);
        let (blocks, to_block_number): (ComAndVerBlocksVecs, BlockNumber256) = match this.get_sorted_past_logs_from_genesis(genesis_block, blocks_delta) {
            Err(_) => return Err(String::from("Cant get sorted past logs")),
            Ok(result) => (result.0, result.1)
        };
        this.committed_blocks = blocks.0;
        this.verified_blocks = blocks.1;
        this.last_watched_block_number = U256::from(to_block_number.as_u64());
        Ok(this)
    }

    pub fn update_state_from_last_watched_block_with_blocks_delta_and_return_new_blocks(&mut self, blocks_delta: U256) -> Result<ComAndVerBlocksVecs, String> {
        let (blocks, to_block_number): (ComAndVerBlocksVecs, BlockNumber256) = match self.get_sorted_past_logs_from_last_watched_block(blocks_delta) {
            Err(_) => return Err(String::from("Cant get sorted past logs")),
            Ok(result) => (result.0, result.1)
        };
        let blocks_for_return = blocks.clone();
        self.committed_blocks.extend(blocks.0);
        self.verified_blocks.extend(blocks.1);
        self.last_watched_block_number = U256::from(to_block_number.as_u64());
        Ok(blocks_for_return)
    }

    pub fn get_committed_blocks(&self) -> &Vec<LogBlockData> {
        &self.committed_blocks
    }

    pub fn get_verified_blocks(&self) -> &Vec<LogBlockData> {
        &self.verified_blocks
    }

    pub fn get_last_block_number(&mut self) -> Result<BlockNumber256, String> {
        let (_eloop, transport) = match web3::transports::Http::new(self.http_endpoint_string.as_str()) {
            Err(_) => return Err(String::from("Error creating web3 with this endpoint")),
            Ok(result) => result,
        };
        let web3 = web3::Web3::new(transport);
        let last_block_number = web3.eth().block_number().wait();
        let result = match last_block_number {
            Err(_) => return Err(String::from("Error getting last block number")),
            Ok(result) => result,
        };
        Ok(result)
    }

    // returns (committed blocks logs, verified blocks logs)
    fn sort_logs(&mut self, logs: &Vec<Log>) -> Result<ComAndVerBlocksVecs, String> {
        let logs = logs.clone();
        if logs.len() == 0 {
            return Err(String::from("No logs in list"))
        }
        let mut committed_blocks: Vec<LogBlockData> = vec![];
        let mut verified_blocks: Vec<LogBlockData> = vec![];
        let block_verified_topic = "BlockVerified(uint32)";
        let block_committed_topic = "BlockCommitted(uint32)";
        let block_verified_topic_h256: H256 = helpers::get_topic_keccak_hash(block_verified_topic);
        let block_committed_topic_h256: H256 = helpers::get_topic_keccak_hash(block_committed_topic);
        for log in logs {
            let mut block: LogBlockData = LogBlockData {
                block_num: 0,
                transaction_hash : H256::zero(),
                block_type: BlockType::Unknown
            };
            // Log data
            let tx_hash = log.transaction_hash;
            let topic = log.topics[0];
            let block_num = log.topics[1];

            match tx_hash {
                Some(hash) => {
                    block.block_num = U256::from(block_num).as_u32();
                    block.transaction_hash = hash;

                    if topic == block_verified_topic_h256 {
                        block.block_type = BlockType::Verified;
                        verified_blocks.push(block);
                        // let result = self.check_committed_block_with_same_number_as_verified(&block);
                        // println!("Block exists: {:?}", result);
                        // let tx = result.unwrap().clone().transaction_hash;
                        // println!("--- Starting getting tx");
                        // let data = FranklinTransaction::get_transaction(InfuraEndpoint::Rinkeby, &tx);
                        // println!("TX data committed: {:?}", data);
                    } else if topic == block_committed_topic_h256 {
                        block.block_type = BlockType::Committed;
                        committed_blocks.push(block);
                    }
                },
                None    => println!("No tx hash"),
            };
        }
        committed_blocks.sort_by_key(|x| x.block_num);
        verified_blocks.sort_by_key(|x| x.block_num);
        Ok((committed_blocks, verified_blocks))
    }

    fn get_logs(&mut self, from_block_number: BlockNumber, to_block_number: BlockNumber) -> Result<Vec<Log>, String> {
        // Set web3
        let (_eloop, transport) = match web3::transports::Http::new(self.http_endpoint_string.as_str()) {
            Err(_) => return Err(String::from("Error creating web3 with this endpoint")),
            Ok(result) => result,
        };
        let web3 = web3::Web3::new(transport);

        // let contract = Contract::new(web3.eth(), franklin_address.clone(), franklin_contract.clone());

        // Events topics
        let block_verified_topic = "BlockVerified(uint32)";
        let block_committed_topic = "BlockCommitted(uint32)";
        let block_verified_topic_h256: H256 = helpers::get_topic_keccak_hash(block_verified_topic);
        let block_committed_topic_h256: H256 = helpers::get_topic_keccak_hash(block_committed_topic);

        let topics_vec_h256: Vec<H256> = vec![block_verified_topic_h256, block_committed_topic_h256];

        // Filter
        let filter = FilterBuilder::default()
                    .address(vec![self.franklin_contract_address.clone()])
                    .from_block(from_block_number)
                    .to_block(to_block_number)
                    .topics(
                        Some(topics_vec_h256),
                        None,
                        None,
                        None,
                    )
                    .build();

        // Filter result
        let events_filter_result = web3.eth().logs(filter).wait();
        if events_filter_result.is_err() {
            return Err(String::from("Error getting filter results"))
        }

        // Logs
        let logs = match events_filter_result {
            Err(_) => Err(String::from("Wrong events result")),
            Ok(result) => {
                if result.len()== 0 {
                    return Err(String::from("No logs in list"))
                }
                Ok(result)
            }
        };
        logs
    }

    pub fn get_sorted_logs_in_block(&mut self, block_number: BlockNumber256) -> Result<ComAndVerBlocksVecs, String> {
        let block_to_get_logs = BlockNumber::Number(block_number.as_u64());
        match self.get_logs(block_to_get_logs, block_to_get_logs) {
            Err(_) => {
                let message = String::from("No logs in block ") + &block_number.as_u64().to_string();
                return Err(message)
            },
            Ok(result) => {
                match self.sort_logs(&result) {
                    Err(_) => {
                        let message = String::from("Cant sort logs in block ") + &block_number.as_u64().to_string();
                        return Err(message)
                    },
                    Ok(sorted_logs) => return Ok((sorted_logs.0, sorted_logs.1)),
                };
            },
        };
    }

    fn get_past_logs(&mut self, from_block_number: U256, blocks_delta: U256) -> Result<(Vec<Log>, BlockNumber256), String> {
        // Set web3
        let last_block_number = match self.get_last_block_number() {
            Err(_) => return Err(String::from("Cant get last block number")),
            Ok(result) => result,
        };
        let to_block_numer_256 = last_block_number - blocks_delta;
        let result = to_block_numer_256.checked_sub(from_block_number);
        if result.is_none() {
            return Err(String::from("No new blocks"))
        }
        let to_block_number = BlockNumber::Number(to_block_numer_256.as_u64());
        let from_block_number = BlockNumber::Number(from_block_number.as_u64());
        // let last_block_number_u64 = last_block_number.as_u64();
        // // To block = last block - blocks delta
        // let to_block_number_u64 = last_block_number_u64 - blocks_delta;
        // let to_block_number: BlockNumber = BlockNumber::Number(to_block_number_u64);

        let logs = match self.get_logs(from_block_number, to_block_number) {
            Err(_) => return Err(String::from("Cant get past logs")),
            Ok(result) => result
        };
        Ok((logs, to_block_numer_256))
    }

    // returns (committed blocks logs, verified blocks logs) from genesis to (last block minus delta)
    pub fn get_sorted_past_logs_from_last_watched_block(&mut self, blocks_delta: U256) -> Result<(ComAndVerBlocksVecs, BlockNumber256), String> {
        let from_block_number = self.last_watched_block_number + 1;
        let (logs, to_block_number) = match self.get_past_logs(from_block_number, blocks_delta) {
            Err(_) => return Err(String::from("Cant get past logs")),
            Ok(result) => result,
        };
        let sorted_logs = match self.sort_logs(&logs) {
            Err(_) => return Err(String::from("Cant sort_logs")),
            Ok(result) => result,
        };
        Ok((sorted_logs, to_block_number))
    }

    // returns (committed blocks logs, verified blocks logs) from genesis to (last block minus delta)
    pub fn get_sorted_past_logs_from_genesis(&mut self, genesis_block: U256, blocks_delta: U256) -> Result<(ComAndVerBlocksVecs, BlockNumber256), String> {
        let from_block_number = U256::from(genesis_block);
        let (logs, to_block_number) = match self.get_past_logs(from_block_number, blocks_delta) {
            Err(_) => return Err(String::from("Cant get past logs")),
            Ok(result) => result,
        };
        let sorted_logs = match self.sort_logs(&logs) {
            Err(_) => return Err(String::from("Cant sort_logs")),
            Ok(result) => result,
        };
        Ok((sorted_logs, to_block_number))
    }

    pub fn get_only_verified_committed_blocks(&self) -> Vec<&LogBlockData> {
        let ver_blocks = &mut self.verified_blocks.iter();
        // let committed_blocks_iter = &mut self.com_blocks.iter();
        let mut ver_com_blocks = vec![];
        for block in ver_blocks {
            let find_com_block = self.check_committed_block_with_same_number_as_verified(block);
            if find_com_block.is_none() {
                continue;
            }
            ver_com_blocks.push(find_com_block.unwrap())
        }
        ver_com_blocks.sort_by_key(|&x| x.block_num);
        ver_com_blocks
    }

    pub fn check_committed_block_with_same_number_as_verified(&self, verified_block: &LogBlockData) -> Option<&LogBlockData> {
        let committed_blocks_iter = &mut self.committed_blocks.iter();
        let committed_block = committed_blocks_iter.find(|&&x| x.block_num == verified_block.block_num);
        return committed_block
    }

    // // - Get new block
    // // - Need to watch block + 1
    // // - Get events from need to watch block
    // // - Sort them to committed and verified
    // // - Write to committed_blocks and verified_blocks
    // pub fn make_new_sorted_logs_subscription(&mut self, eloop: &mut Core) {
    //     // Setup loop and web3
    //     // let mut eloop = Core::new().unwrap();
    //     let handle = eloop.handle();
    //     let web3_instance = Rc::new(web3::Web3::new(
    //         web3::transports::WebSocket::with_event_loop(self.ws_endpoint_string.as_str(), &handle)
    //             .unwrap(),
    //     ));

    //     // Subscription
    //     println!("subscribing to new blocks");

    //     let future = web3_instance.eth_subscribe()
    //         .subscribe_new_heads()
    //         .and_then(|sub| {
    //             sub.for_each(|log| {
    //                 println!("---");
    //                 println!("Got block number {:?}", log.number);
    //                 let number_to_watch = self.last_watched_block_number + 1;
    //                 self.last_watched_block_number = number_to_watch;
    //                 println!("Block to watch {:?}", &number_to_watch);
    //                 match self.get_sorted_logs_in_block(number_to_watch) {
    //                     Ok(mut result) => {
    //                         println!("Old committed blocks array len: {:?}", &self.committed_blocks.len());
    //                         println!("Old verified blocks array len: {:?}", &self.verified_blocks.len());
    //                         println!("Got sorted logs");
    //                         println!("Committed: {:?}", &result.0);
    //                         println!("Verified: {:?}", &result.1);
    //                         self.committed_blocks.append(&mut result.0);
    //                         self.verified_blocks.append(&mut result.1);
    //                         println!("New committed blocks array len: {:?}", &self.committed_blocks.len());
    //                         println!("New verified blocks array len: {:?}", &self.verified_blocks.len());

    //                     },
    //                     Err(_) => {
    //                         println!("No new blocks");
    //                     }
    //                 };
    //                 Ok(())
    //             })
    //         })
    //         .map_err(|e| eprintln!("franklin log err: {}", e));

    //     // Run eloop
    //     if let Err(_err) = eloop.run(future) {
    //         eprintln!("Cant run eloop");
    //     }
    // }

    // pub fn subscribe_to_logs(&mut self) {

    //     // Get topic keccak hash
    //     let block_verified_topic = "BlockVerified(uint32)";
    //     let block_committed_topic = "BlockCommitted(uint32)";
    //     let block_verified_topic_h256: H256 = helpers::get_topic_keccak_hash(block_verified_topic);
    //     let block_committed_topic_h256: H256 = helpers::get_topic_keccak_hash(block_committed_topic);

    //     let topics_vec_h256: Vec<H256> = vec![block_verified_topic_h256, block_committed_topic_h256];

    //     // Setup loop and web3
    //     let mut eloop = Core::new().unwrap();
    //     let handle = eloop.handle();
    //     let w3 = Rc::new(web3::Web3::new(
    //         web3::transports::WebSocket::with_event_loop(self.ws_endpoint_string.as_str(), &handle)
    //             .unwrap(),
    //     ));

    //     // Subscription
    //     println!("subscribing to franklin logs {:?} {:?}...", block_verified_topic, block_committed_topic);

    //     let filter = FilterBuilder::default()
    //         .address(vec![self.franklin_contract_address.clone()])
    //         .topics(
    //             Some(topics_vec_h256),
    //             None,
    //             None,
    //             None,
    //         )
    //         .build();

    //     let future = w3.eth_subscribe()
    //         .subscribe_logs(filter)
    //         .and_then(|sub| {
    //             sub.for_each(|log| {
    //                 println!("---");
    //                 println!("got log from subscription: {:?}", log);

    //                 let mut sorted_blocks = self.sort_logs(&vec![log]).unwrap();
    //                 self.committed_blocks.append(&mut sorted_blocks.0);
    //                 self.verified_blocks.append(&mut sorted_blocks.1);
    //                 // let result = self.check_committed_block_with_same_number_as_verified(&block);
    //                 // println!("Block exists: {:?}", result);
    //                 // let tx = result.unwrap().clone().transaction_hash;
    //                 // println!("--- Starting getting tx");
    //                 // let data = FranklinTransaction::get_transaction(InfuraEndpoint::Rinkeby, &tx);
    //                 // println!("TX data committed: {:?}", data);

    //                 println!("Verified blocks in storage: {:?}", self.verified_blocks);
    //                 println!("Committed blocks in storage: {:?}", self.committed_blocks);
    //                 Ok(())
    //             })
    //         })
    //         .map_err(|e| eprintln!("franklin log err: {}", e));

    //     // Run eloop
    //     if let Err(_err) = eloop.run(future) {
    //         eprintln!("ERROR");
    //     }
    // }
}

