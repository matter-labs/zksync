extern crate rustc_hex;
extern crate web3;

use ff::{Field, PrimeField, PrimeFieldRepr};

use std::env;
use std::str::FromStr;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Sender;
use super::models::{StateKeeperRequest, ProtoBlock};
use plasma::models::{Block, BlockData, DepositTx, Engine, Fr, ExitTx};
use bigdecimal::{Num, BigDecimal};
use plasma::models::params;

use std::time;
use web3::contract::{Contract, Options};
use web3::futures::{Future};
use web3::types::{U256, H160, H256, U128, FilterBuilder, BlockNumber};
use sapling_crypto::jubjub::{edwards, Unknown};
use super::config;

use super::storage::{ConnectionPool, StorageProcessor};

type ABI = (&'static [u8], &'static str);

pub const TEST_PLASMA_ALWAYS_VERIFY: ABI = (
    include_bytes!("../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.abi"),
    include_str!("../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.bin"),
);

pub const PROD_PLASMA: ABI = (
    include_bytes!("../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.abi"),
    include_str!("../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.bin"),
);


fn u64_from_environment(name: &str) -> Option<u64> {
    let candidate = env::var(name);
    if let Ok(candidate_string) = candidate {
        if let Ok(as_u64) = candidate_string.parse::<u64>() {
            return Some(as_u64);
        }
    }

    None
}

pub struct EthWatch {
    last_processed_block: u64,
    blocks_lag: u64,
    contract_addr:  H160,
    web3_url:       String,
    contract:       ethabi::Contract,
    last_deposit_batch: U256,
    last_exit_batch: U256,
    deposit_batch_size: U256,
    exit_batch_size: U256,
    connection_pool: ConnectionPool
}

/// Watcher will accumulate requests for deposit and exits in internal memory 
/// and pass them to processing when either a required amount is accumulated
/// or a manual timeout is triggered
/// 
/// Functionality to change deposit and exit fees will not be implemented for now
impl EthWatch {

    pub fn new(start_from_block: u64, lag: u64, pool: ConnectionPool) -> Self {

        let mut start = start_from_block;
        if let Some(starting_block_from_env) = u64_from_environment("FROM_BLOCK") {
            start = starting_block_from_env;
        }

        let mut delay = lag;
        if let Some(delay_from_env) = u64_from_environment("BLOCK_DELAY") {
            delay = delay_from_env;
        }

        let mut this = Self {
            last_processed_block: start,
            blocks_lag: delay,
            web3_url:       env::var("WEB3_URL").expect("WEB3_URL env var not found"),
            contract_addr:  H160::from_str(&env::var("CONTRACT_ADDR").expect("CONTRACT_ADDR env var not found")).expect("contract address must be correct"),
            contract:       ethabi::Contract::load(TEST_PLASMA_ALWAYS_VERIFY.0).expect("contract must be loaded"),
            last_deposit_batch: U256::from(0),
            last_exit_batch: U256::from(0),
            deposit_batch_size: U256::from(config::DEPOSIT_BATCH_SIZE),
            exit_batch_size: U256::from(config::EXIT_BATCH_SIZE),
            connection_pool: pool,
        };

        let storage = this.connection_pool.access_storage().expect("db connection failed for eth watcher");;
        let last_committed_deposit = storage.load_last_committed_deposit_batch().expect("load_last_committed_deposit_batch: db must work");
        let last_committed_exit = storage.load_last_committed_exit_batch().expect("load_last_committed_exit_batch: db must work");

        let expecting_deposit_batch = (last_committed_deposit + 1) as u32;
        let expecting_exit_batch = (last_committed_exit + 1) as u32;

        this.last_deposit_batch = U256::from(expecting_deposit_batch);
        this.last_exit_batch = U256::from(expecting_exit_batch);
        
        println!("Monitoring contract {:x}", this.contract_addr);
        println!("Starting from deposit batch {}", this.last_deposit_batch);
        println!("Starting from exit batch {}", this.last_exit_batch);

        this
    }

    /// logic here is the following
    /// - wait for a new block
    /// - move back in time to avoid reorgs
    /// - check if the last deposit batch number is equal to the one in contract
    /// - if it's larger - collect events and send for processing
    /// - if it's not bumped but a timeout is past due - may be try to send the transaction that bumps it
    pub fn run(&mut self, tx_for_blocks: Sender<StateKeeperRequest>) {
        let (_eloop, transport) = web3::transports::Http::new(&self.web3_url).unwrap();
        let web3 = web3::Web3::new(transport);
        // let mut eloop = tokio_core::reactor::Core::new().unwrap();
        // let web3 = web3::Web3::new(web3::transports::Http::with_event_loop("http://localhost:8545", &eloop.handle(), 1).unwrap());
        let contract = Contract::new(web3.eth(), self.contract_addr.clone(), self.contract.clone());

        loop {
            std::thread::sleep(time::Duration::from_secs(1));
            let last_block_number = web3.eth().block_number().wait();
            if last_block_number.is_err() {
                continue
            }

            let last_ethereum_block = last_block_number.unwrap().as_u64();

            if  last_ethereum_block == self.last_processed_block + self.blocks_lag {
                continue
            }

            let block_number = last_ethereum_block - self.blocks_lag;

            // process deposits BEFORE exits for a rare case of deposit + full exit in the same block
            let deposits_result = self.process_deposits(block_number, &tx_for_blocks, &web3, &contract);
            if let Err(err) = deposits_result {
                println!("Failed to process deposit logs: {}", err);
                continue
            }

            let exits_result = self.process_exits(block_number, &tx_for_blocks, &web3, &contract);
            if let Err(err) = exits_result {
                println!("Failed to process exit logs: {}", err);
                continue
            }
        
            self.last_processed_block = last_ethereum_block - self.blocks_lag;
        }

        // on new deposit or exit blocks => pass them via tx_for_blocks
        // on new tx blocks do nothing for now; later we can use them to sync multiple 
        // servers (in which case we only use them to update current state)
    }

    fn process_deposits<T: web3::Transport>(& mut self, 
        block_number: u64, 
        channel: &Sender<StateKeeperRequest>,
        web3: &web3::Web3<T>,
        contract: &Contract<T>)
    -> Result<(), String>
    {
        println!("Processing deposits for block {}", block_number);
        let total_deposit_requests_result: U256 = 
            contract.query("totalDepositRequests", (), None, Options::default(), Some(BlockNumber::Number(block_number)))
            .wait()
            .map_err(|err| format!("Error getting total deposit requests {}", err))?;

        let total_deposit_requests = total_deposit_requests_result;
        println!("total_deposit_requests = {}", total_deposit_requests);

        let max_batch_number = total_deposit_requests / self.deposit_batch_size;

        if max_batch_number <= self.last_deposit_batch {
            // this watcher is not responsible for bumping a batch number
            return Ok(());
        }

        let form_batch_number = (self.last_deposit_batch + U256::from(1)).as_u64();
        let to_batch_number = (max_batch_number + U256::from(1)).as_u64();

        for batch_number in form_batch_number..to_batch_number {
            self.process_single_deposit_batch(
                batch_number, 
                channel, 
                web3,
                contract
            )?;
            self.last_deposit_batch = self.last_deposit_batch + U256::from(1);
        }

        Ok(())
    }

    fn process_single_deposit_batch<T: web3::Transport>(
        & mut self, 
        batch_number: u64,
        channel: &Sender<StateKeeperRequest>,
        web3: &web3::Web3<T>,
        contract: &Contract<T>)
    -> Result<(), String> {
        let deposit_event = self.contract.event("LogDepositRequest").unwrap().clone();
        let deposit_event_topic = deposit_event.signature();

        let deposit_canceled_event = self.contract.event("LogCancelDepositRequest").unwrap().clone();
        let deposit_canceled_topic = deposit_canceled_event.signature();

        // event LogDepositRequest(uint256 indexed batchNumber, uint24 indexed accountID, uint256 indexed publicKey, uint128 amount);

        let deposits_filter = FilterBuilder::default()
                    .address(vec![contract.address()])
                    .from_block(BlockNumber::Earliest)
                    .to_block(BlockNumber::Latest)
                    .topics(
                        Some(vec![deposit_event_topic]),
                        Some(vec![H256::from(self.last_deposit_batch.clone())]),
                        None,
                        None,
                    )
                    .build();

        let cancels_filter = FilterBuilder::default()
            .address(vec![contract.address()])
            .from_block(BlockNumber::Earliest)
            .to_block(BlockNumber::Latest)
            .topics(
                Some(vec![deposit_canceled_topic]),
                Some(vec![H256::from(self.last_deposit_batch.clone())]),
                None,
                None,
            )
            .build();

        let deposit_events_filter_result = web3.eth().logs(deposits_filter).wait()
            .map_err(|err| format!("deposit_events_filter error: {}", err))?;
        let cancel_events_filter_result = web3.eth().logs(cancels_filter).wait()
            .map_err(|err| format!("deposits: cancel_events_filter_result error: {}", err))?;

        let deposit_events = deposit_events_filter_result;
        let cancel_events = cancel_events_filter_result;

        // now we have to merge and apply
        let mut all_events = vec![];
        all_events.extend(deposit_events.into_iter());
        all_events.extend(cancel_events.into_iter());

        all_events = all_events.into_iter().filter(|el| el.is_removed() == false).collect();

        // sort by index

        all_events.sort_by(|l, r| {
            let l_block = l.block_number.unwrap();
            let r_block = r.block_number.unwrap();

            if l_block > r_block {
                return std::cmp::Ordering::Greater;
            } else if l_block < r_block {
                return std::cmp::Ordering::Less;
            }

            let l_index = l.log_index.unwrap();
            let r_index = r.log_index.unwrap();
            if l_index > r_index {
                return std::cmp::Ordering::Greater;
            } else if l_index < r_index {
                return std::cmp::Ordering::Less;
            }

            panic!("Logs can not have same indexes");
        }        
        );

        // hashmap accoundID => (balance, public_key)
        let mut this_batch: HashMap<U256, (U256, U256)> = HashMap::new();

        for event in all_events {
            let topic = event.topics[0];
            match () {
                () if topic == deposit_event_topic => {
                    let data_bytes: Vec<u8> = event.data.0;
                    let account_id = U256::from(event.topics[2]);
                    let public_key = U256::from(event.topics[3]);
                    let deposit_amount = U256::from_big_endian(&data_bytes);
                    let existing_record = this_batch.get(&account_id).map(|&v| v.clone());
                    if let Some(record) = existing_record {
                        let mut existing_balance = record.0;
                        existing_balance = existing_balance + deposit_amount;
                        this_batch.insert(account_id, (existing_balance, record.1));
                    } else {
                        this_batch.insert(account_id, (deposit_amount, public_key));
                    }
                    continue;
                },
                () if topic == deposit_canceled_topic => {
                    let account_id = U256::from(event.topics[2]);
                    let existing_record = this_batch.get(&account_id).map(|&v| v.clone())
                        .ok_or("existing_record not found for deposits")?;
                    this_batch.remove(&account_id);
                    continue;
                },
                _ => return Err("unexpected topic".to_owned()),
            }
        }

        let mut all_deposits = vec![];
        for (k, v) in this_batch.iter() {
            println!("Into account {:x} with public key {:x}, deposit amount = {}", k, v.1, v.0);
            let mut public_key_bytes = vec![0u8; 32];
            v.1.to_big_endian(& mut public_key_bytes);
            let x_sign = public_key_bytes[0] & 0x80 > 0;
            public_key_bytes[0] &= 0x7f;
            let mut fe_repr = Fr::zero().into_repr();
            fe_repr.read_be(public_key_bytes.as_slice()).expect("read public key point");
            let y = Fr::from_repr(fe_repr);
            if y.is_err() {
                return Err("could not parse y".to_owned());
            }
            let public_key_point = edwards::Point::<Engine, Unknown>::get_for_y(y.unwrap(), x_sign, &params::JUBJUB_PARAMS);
            if public_key_point.is_none() {
                return Err("public_key_point conversion error".to_owned());
            }

            let (pub_x, pub_y) = public_key_point.unwrap().into_xy();

            let tx: DepositTx = DepositTx{
                account: k.as_u32(),
                amount:  BigDecimal::from_str_radix(&format!("{}", v.0), 10).unwrap(),
                pub_x:   pub_x,
                pub_y:   pub_y,
            };
            all_deposits.push(tx);
        }

        let len = all_deposits.len();
        let block = ProtoBlock::Deposit(
            self.last_deposit_batch.as_u32(),
            all_deposits,
        );
        println!("Deposit batch {}, {} accounts", self.last_deposit_batch, len);
        let request = StateKeeperRequest::AddBlock(block);

        let send_result = channel.send(request).map_err(|err| format!("channel.send() failed: {}", err))?;

        Ok(())

    }

    fn process_exits<T: web3::Transport>(& mut self, 
        block_number: u64, 
        channel: &Sender<StateKeeperRequest>,
        web3: &web3::Web3<T>,
        contract: &Contract<T>)
    -> Result<(), String>
    {
        use bigdecimal::Zero;
        println!("Processing exits for block {}", block_number);
        let total_requests_result: U256 = contract.query("totalExitRequests", (), None, Options::default(), Some(BlockNumber::Number(block_number)))
            .wait()
            .map_err(|err| format!("Error getting total exit requests {}", err))?;

        let total_requests = total_requests_result;

        let max_batch_number = total_requests / self.exit_batch_size;

        if max_batch_number <= self.last_exit_batch {
            // this watcher is not responsible for bumping a batch number
            return Ok(());
        }

        let form_batch_number = (self.last_exit_batch + U256::from(1)).as_u64();
        let to_batch_number = (max_batch_number + U256::from(1)).as_u64();

        for batch_number in form_batch_number..to_batch_number {
            let res = self.process_single_exit_batch(
                batch_number, 
                channel, 
                web3,
                contract
            )?;
            self.last_exit_batch = self.last_exit_batch + U256::from(1);
        }

        Ok(())
    }

    fn process_single_exit_batch<T: web3::Transport>(
        & mut self, 
        batch_number: u64, 
        channel: &Sender<StateKeeperRequest>,
        web3: &web3::Web3<T>,
        contract: &Contract<T>)
    -> Result<(), String>
    {
        use bigdecimal::Zero;

        let exit_event = self.contract.event("LogExitRequest").unwrap().clone();
        let exit_event_topic = exit_event.signature();

        let exit_canceled_event = self.contract.event("LogCancelExitRequest").unwrap().clone();
        let exit_canceled_topic = exit_canceled_event.signature();

        // event LogDepositRequest(uint256 indexed batchNumber, uint24 indexed accountID, uint256 indexed publicKey, uint128 amount);

        let exits_filter = FilterBuilder::default()
                    .address(vec![contract.address()])
                    .from_block(BlockNumber::Earliest)
                    .to_block(BlockNumber::Latest)
                    .topics(
                        Some(vec![exit_event_topic]),
                        Some(vec![H256::from(self.last_exit_batch.clone())]),
                        None,
                        None,
                    )
                    .build();

        let cancels_filter = FilterBuilder::default()
            .address(vec![contract.address()])
            .from_block(BlockNumber::Earliest)
            .to_block(BlockNumber::Latest)
            .topics(
                Some(vec![exit_canceled_topic]),
                Some(vec![H256::from(self.last_exit_batch.clone())]),
                None,
                None,
            )
            .build();

        let exit_events_filter_result = web3.eth().logs(exits_filter).wait()
            .map_err(|err| format!("exit_events_filter_result error: {}", err))?;
        let cancel_events_filter_result = web3.eth().logs(cancels_filter).wait()
            .map_err(|err| format!("exits: cancel_events_filter_result error: {}", err))?;

        let exit_events = exit_events_filter_result;
        let cancel_events = cancel_events_filter_result;

        // now we have to merge and apply
        let mut all_events = vec![];
        all_events.extend(exit_events.into_iter());
        all_events.extend(cancel_events.into_iter());

        all_events = all_events.into_iter().filter(|el| el.is_removed() == false).collect();

        // sort by index

        all_events.sort_by(|l, r| {
            let l_block = l.block_number.unwrap();
            let r_block = r.block_number.unwrap();

            if l_block > r_block {
                return std::cmp::Ordering::Greater;
            } else if l_block < r_block {
                return std::cmp::Ordering::Less;
            }

            let l_index = l.log_index.unwrap();
            let r_index = r.log_index.unwrap();
            if l_index > r_index {
                return std::cmp::Ordering::Greater;
            } else if l_index < r_index {
                return std::cmp::Ordering::Less;
            }

            panic!("Logs can not have same indexes");
        }        
        );

        // hashmap accoundID => (balance, public_key)
        let mut this_batch: HashSet<U256> = HashSet::new();

        for event in all_events {
            let topic = event.topics[0];
            match () {
                () if topic == exit_event_topic => {
                    let account_id = U256::from(event.topics[2]);
                    let existing_record = this_batch.get(&account_id).map(|&v| v.clone());
                    if let Some(record) = existing_record {
                        // double exit should not be possible due to SC
                        return Err("double exit should not be possible due to SC".to_owned());
                    } else {
                        this_batch.insert(account_id);
                    }
                    continue;
                },
                () if topic == exit_canceled_topic => {
                    let account_id = U256::from(event.topics[2]);
                    let existing_record = this_batch.get(&account_id).map(|&v| v.clone()).ok_or("existing_record fetch failed".to_owned())?;
                    this_batch.remove(&account_id);
                    continue;
                },
                _ => return Err("exit logs: unexpected topic".to_owned()),
            }
        }

        let mut all_exits = vec![];
        for k in this_batch.iter() {
            println!("Exit from account {:x}", k);

            let tx: ExitTx = ExitTx {
                account: k.as_u32(),
                amount:  BigDecimal::zero(),
            };
            all_exits.push(tx);
        }

        let block = ProtoBlock::Exit(
            self.last_exit_batch.as_u32(),
            all_exits,
        );
        let request = StateKeeperRequest::AddBlock(block);

        let send_result = channel.send(request);

        if send_result.is_err() {
            return Err("Couldn't send for processing".to_owned());
        }

        Ok(())
    }

}

pub fn start_eth_watch(mut eth_watch: EthWatch, tx_for_blocks: Sender<StateKeeperRequest>) {
    std::thread::Builder::new().name("eth_watch".to_string()).spawn(move || {
        eth_watch.run(tx_for_blocks);
    });
}