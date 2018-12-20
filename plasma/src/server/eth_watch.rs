extern crate rustc_hex;
// extern crate tokio_core;
extern crate web3;

use std::env;
use std::str::FromStr;

use std::sync::mpsc::Sender;
use crate::models::{Block};
use super::state_keeper::StateProcessingRequest;

use std::time;
use rustc_hex::{FromHex, ToHex};
use web3::contract::{Contract, Options};
use web3::futures::{Future, Stream};
use web3::types::{Address, U256, H160, H256, U128, FilterBuilder, BlockNumber};

type ABI = (&'static [u8], &'static str);

pub const TEST_PLASMA_ALWAYS_VERIFY: ABI = (
    include_bytes!("../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.abi"),
    include_str!("../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.bin"),
);

pub const PROD_PLASMA: ABI = (
    include_bytes!("../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.abi"),
    include_str!("../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.bin"),
);

pub struct EthWatch {
    last_processed_block: u64,
    blocks_lag: u64,
    contract_addr:  H160,
    web3_url:       String,
    contract:       ethabi::Contract,
    // deposit_requests: Vec<(H160, U128)>,
    // exit_requests: Vec<H160>,
    last_deposit_batch_timestamp: time::Instant,
    last_exit_batch_timestamp: time::Instant,
    batch_accumulation_duration: time::Duration,
    last_deposit_batch: U256,
    last_exit_batch: U256,
    current_deposit_batch_fee: U128,
    current_exit_batch_fee: U128,
    deposit_batch_size: U256,
    exit_batch_size: U256,
}

/// Watcher will accumulate requests for deposit and exits in internal memory 
/// and pass them to processing when either a required amount is accumulated
/// or a manual timeout is triggered
/// 
/// Functionality to change deposit and exit fees will not be implemented for now
impl EthWatch {

    pub fn new(start_from_block: u64, lag: u64) -> Self {

        let this = Self{
            last_processed_block: start_from_block,
            blocks_lag: lag,
            web3_url:       env::var("WEB3_URL").unwrap_or("http://localhost:8545".to_string()),
            contract_addr:  H160::from_str(&env::var("CONTRACT_ADDR").unwrap_or("657f6104c70a6E9e4783C2288D3a90008D0E9d58".to_string())).unwrap(),
            contract:       ethabi::Contract::load(TEST_PLASMA_ALWAYS_VERIFY.0).unwrap(),
            last_deposit_batch_timestamp: time::Instant::now(),
            last_exit_batch_timestamp: time::Instant::now(),
            batch_accumulation_duration: time::Duration::from_secs(300),
            last_deposit_batch: U256::from(0),
            last_exit_batch: U256::from(0),
            current_deposit_batch_fee: U128::from(0),
            current_exit_batch_fee: U128::from(0),
            deposit_batch_size: U256::from(1),
            exit_batch_size: U256::from(0),
        };

        // TODO read the deposit and exit batch to start

        this
    }

    /// logic here is the following
    /// - wait for a new block
    /// - move back in time to avoid reorgs
    /// - check if the last deposit batch number is equal to the one in contract
    /// - if it's larger - collect events and send for processing
    /// - if it's not bumped but a timeout is past due - may be try to send the transaction that bumps it
    pub fn run(&mut self, tx_for_blocks: Sender<StateProcessingRequest>) {
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
            println!("Last block number = {}", last_block_number.clone().unwrap().as_u64());
            if last_block_number.unwrap().as_u64() == self.last_processed_block + self.blocks_lag {
                continue
            }

            let block_number = self.last_processed_block + self.blocks_lag + 1;

            let deposits_result = self.process_deposits(block_number, &tx_for_blocks, &web3, &contract);
            if deposits_result.is_err() {
                continue
            }
           
            self.last_processed_block += 1;
        }

        // TODO: watch chain events
        // on new deposit or exit blocks => pass them via tx_for_blocks
        // on new tx blocks do nothing for now; later we can use them to sync multiple 
        // servers (in which case we only use them to update current state)
    }

    fn process_deposits<T: web3::Transport>(& mut self, 
        block_number: u64, 
        channel: &Sender<StateProcessingRequest>,
        web3: &web3::Web3<T>,
        contract: &Contract<T>)
    -> Result<(), ()>
    {
        println!("Checking for state for block {}", block_number);
        let total_deposit_requests_result: Result<U256, _> = contract.query("totalDepositRequests", (), None, Options::default(), Some(BlockNumber::Number(block_number))).wait();

        if total_deposit_requests_result.is_err() {
            println!("Error getting total deposit requets {}", total_deposit_requests_result.err().unwrap());
            return Err(());
        }

        println!("Checking a batch number");

        let total_deposit_requests = total_deposit_requests_result.unwrap();

        println!("Total deposit requests = {}", total_deposit_requests);

        let batch_number = total_deposit_requests / self.deposit_batch_size;

        println!("Batch number = {}", batch_number.clone());

        if batch_number == self.last_deposit_batch {
            if time::Instant::now() >= self.last_deposit_batch_timestamp + self.batch_accumulation_duration {
                // TODO: bump batch number or leave it for another service
            } else {
                return Ok(());
            }
        }

        let deposit_event = self.contract.event("LogDepositRequest").unwrap().clone();
        let deposit_event_topic = deposit_event.signature();

        // event LogDepositRequest(uint256 indexed batchNumber, uint24 indexed accountID, uint256 indexed publicKey, uint128 amount);

        let deposits_filter = FilterBuilder::default()
                    // .address(vec![contract.address()])
                    .topics(
                        Some(vec![deposit_event_topic]),
                        Some(vec![H256::from(self.last_deposit_batch.clone())]),
                        None,
                        None,
                    )
                    .build();

        let deposit_events_filter_result = web3.eth().logs(deposits_filter).wait();

        if deposit_events_filter_result.is_err() {
            println!("Error getting total deposit requets {}", deposit_events_filter_result.err().unwrap());
            return Err(());
        }

        let deposit_events = deposit_events_filter_result.unwrap();

        println!("Events in this block = {}", deposit_events.len());

        for deposit in deposit_events {
            let data_bytes: Vec<u8> = deposit.data.0;
            let account_id = U256::from(deposit.topics[2]);
            let public_key = U256::from(deposit.topics[3]);
            let deposit_amount = U256::from_big_endian(&data_bytes);
            println!("Deposit from {:x}, key {:x}, amount {:x}", account_id, public_key, deposit_amount);
        }

        self.last_deposit_batch = self.last_deposit_batch + U256::from(1);

        Ok(())
    }

}

#[test]
fn test_eth_watcher() {

    let mut client = EthWatch::new(0, 0);
    let (tx_for_state, rx_for_state) = std::sync::mpsc::channel::<StateProcessingRequest>();

    client.run(tx_for_state);
}