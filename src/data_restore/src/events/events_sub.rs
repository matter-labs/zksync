use std::rc::Rc;
use web3::futures::{Future, Stream};
use web3::types::{Log, Address, FilterBuilder, H256, U256, BlockNumber};
use tokio_core::reactor::Core;
use blocks::{BlockType, LogBlockData};
use helpers;
use helpers::InfuraEndpoint;
use ethabi::{Contract, Event, Hash};

type ABI = (&'static [u8], &'static str);

pub const PLASMA_TEST_ABI: ABI = (
    include_bytes!("../../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.abi"),
    include_str!("../../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.bin"),
);

pub const PLASMA_PROD_ABI: ABI = (
    include_bytes!("../../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.abi"),
    include_str!("../../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.bin"),
);

pub struct EventsFranklin {
    pub http_endpoint_string: String,
    pub ws_endpoint_string: String,
    pub franklin_abi: ABI,
    pub franklin_contract: Contract,
    pub franklin_contract_address: Address,
    pub committed_blocks: Vec<LogBlockData>,
    pub verified_blocks: Vec<LogBlockData>,
}

impl EventsFranklin {
    pub fn new(network: InfuraEndpoint) -> Self {
        let ws_infura_endpoint_str = match network {
            InfuraEndpoint::Mainnet => "wss://mainnet.infura.io/ws",
            InfuraEndpoint::Rinkeby => "wss://rinkeby.infura.io/ws",
        };
        let ws_infura_endpoint_string = String::from(ws_infura_endpoint_str);
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
            ws_endpoint_string: ws_infura_endpoint_string,
            http_endpoint_string: http_infura_endpoint_string,
            franklin_abi: abi,
            franklin_contract: contract,
            franklin_contract_address: address,
            committed_blocks: vec![],
            verified_blocks: vec![],
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

    pub fn get_last_block_number(&mut self) -> Result<U256, &'static str> {
        let (_eloop, transport) = match web3::transports::Http::new(self.http_endpoint_string.as_str()) {
            Err(_) => return Err("Error creating web3 with this endpoint"),
            Ok(result) => result,
        };
        let web3 = web3::Web3::new(transport);
        let last_block_number = web3.eth().block_number().wait();
        let result = match last_block_number {
            Err(_) => return Err("Error getting last block number"),
            Ok(result) => result,
        };
        Ok(result)
    }

    pub fn sort_logs(&mut self, logs: Vec<Log>) -> Result<(Vec<LogBlockData>, Vec<LogBlockData>), &'static str> {
        if logs.len() == 0 {
            return Err("No logs in list")
        }
        let mut committed_block_data: Vec<LogBlockData> = vec![];
        let mut verified_block_data: Vec<LogBlockData> = vec![];
        let block_verified_topic = "BlockVerified(uint32)";
        let block_committed_topic = "BlockCommitted(uint32)";
        let block_verified_topic_h256: H256 = helpers::get_topic_keccak_hash(block_verified_topic);
        let block_committed_topic_h256: H256 = helpers::get_topic_keccak_hash(block_committed_topic);
        for log in logs {
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
                        block.block_type = BlockType::Verified;
                        verified_block_data.push(block);
                        // let result = self.check_committed_block_with_same_number_as_verified(&block);
                        // println!("Block exists: {:?}", result);
                        // let tx = result.unwrap().clone().transaction_hash;
                        // println!("--- Starting getting tx");
                        // let data = FranklinTransaction::get_transaction(InfuraEndpoint::Rinkeby, &tx);
                        // println!("TX data committed: {:?}", data);
                    } else if topic == block_committed_topic_h256 {
                        block.block_type = BlockType::Committed;
                        committed_block_data.push(block);
                    }
                },
                None    => println!("No tx hash"),
            };
        }
        Ok((committed_block_data, verified_block_data))
    }

    pub fn get_logs(&mut self, from_block_number: BlockNumber, to_block_number: BlockNumber) -> Result<Vec<Log>, &'static str> {
        // Set web3
        let (_eloop, transport) = match web3::transports::Http::new(self.http_endpoint_string.as_str()) {
            Err(_) => return Err("Error creating web3 with this endpoint"),
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
            return Err("Error getting filter results")
        }

        // Logs
        let logs = match events_filter_result {
            Err(_) => Err("Wrong events result"),
            Ok(result) => {
                if result.len()== 0 {
                    return Err("No logs in list")
                }
                Ok(result)
            }
        };
        logs
    }

    pub fn get_past_logs(&mut self, blocks_delta: u64) -> Result<Vec<Log>, &'static str> {
        // Set web3
        let last_block_number_u64 = match self.get_last_block_number() {
            Err(_) => return Err("Cant get last block number"),
            Ok(result) => result,
        }.as_u64();
        // To block = last block - blocks delta
        let to_block_number_u64 = last_block_number_u64 - blocks_delta;
        let to_block_number: BlockNumber = BlockNumber::Number(to_block_number_u64);
        // From block
        let from_block_number = BlockNumber::Earliest;
        let logs = self.get_logs(from_block_number, to_block_number);
        logs
    }

    pub fn subscribe_to_logs(&mut self) {

        // Get topic keccak hash
        let block_verified_topic = "BlockVerified(uint32)";
        let block_committed_topic = "BlockCommitted(uint32)";
        let block_verified_topic_h256: H256 = helpers::get_topic_keccak_hash(block_verified_topic);
        let block_committed_topic_h256: H256 = helpers::get_topic_keccak_hash(block_committed_topic);

        let topics_vec_h256: Vec<H256> = vec![block_verified_topic_h256, block_committed_topic_h256];

        // Setup loop and web3
        let mut eloop = Core::new().unwrap();
        let handle = eloop.handle();
        let w3 = Rc::new(web3::Web3::new(
            web3::transports::WebSocket::with_event_loop(self.ws_endpoint_string.as_str(), &handle)
                .unwrap(),
        ));

        // Subscription
        println!("subscribing to franklin logs {:?} {:?}...", block_verified_topic, block_committed_topic);

        let filter = FilterBuilder::default()
            .address(vec![self.franklin_contract_address.clone()])
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

                    let mut sorted_blocks = self.sort_logs(vec![log]).unwrap();
                    self.committed_blocks.append(&mut sorted_blocks.0);
                    self.verified_blocks.append(&mut sorted_blocks.1);
                    // let result = self.check_committed_block_with_same_number_as_verified(&block);
                    // println!("Block exists: {:?}", result);
                    // let tx = result.unwrap().clone().transaction_hash;
                    // println!("--- Starting getting tx");
                    // let data = FranklinTransaction::get_transaction(InfuraEndpoint::Rinkeby, &tx);
                    // println!("TX data committed: {:?}", data);

                    println!("Verified blocks in storage: {:?}", self.verified_blocks);
                    println!("Committed blocks in storage: {:?}", self.committed_blocks);
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




