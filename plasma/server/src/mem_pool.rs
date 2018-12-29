use std::sync::mpsc::{channel, Sender, Receiver};
use plasma::models::{TransferTx, TransferBlock, Block};
use super::models::StateProcessingRequest;
use super::config;

extern crate im;
use self::im::ordset::{OrdSet};

use std::collections::HashMap;

/// MemPool should keep transaction in memory in the ordered way
/// - keep transactions in the HashMap, where keys are accounts and 
/// values are some ordered data structures with ordering done by nonce
/// - provide a secondary index in a form of ordered structure where priority is determined by fee
/// and the value is just account_id, so iterator consumes the first item from the already ordered
/// by nonce set of transactions for this account
/// - this also allows to quickly lookup the latest pending nonce in the pool
/// - removal of transaction with some nonce (if we allow it) will purge all the following elements too
/// - this structure is efficient logically, but not cache-friendly, so should be changed in a future
/// while preserving public functions signatures

const MAX_TRANSACTIONS_PER_ACCOUNT: usize = 128;

pub struct MemPool {
    // Batch size
    pub batch_size: usize,

    // // Accumulated transactions
    // pub current_block: TransferBlock,

    // pool itself as a hashmap account_id => OrdSet
    per_account_info: HashMap<u32, OrdSet<TransferTx>>,

    // queue
    queue: OrdSet<PoolRecord>
}

#[derive(Clone, Debug)]
struct PoolRecord {
    fee: u128,
    nonce: u32,
    account_id: u32,
}

use std::cmp::{Ord, PartialEq, PartialOrd, Eq, Ordering};

impl Ord for PoolRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.account_id == other.account_id && self.nonce == other.nonce {
            // replace by a new nonce, handle fee on another level
            return Ordering::Equal;
        } else if self.account_id == other.account_id {
            if self.nonce < other.nonce {
                return Ordering::Less;
            } else {
                return Ordering::Greater;
            }
        }

        if self.fee >= other.fee {
            // sort by fee by default
            return Ordering::Less;
        } else {
            return Ordering::Greater;
        }
    }
}

impl PartialOrd for PoolRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PoolRecord {
    fn eq(&self, other: &Self) -> bool {
        return self.account_id == other.account_id && self.nonce == other.nonce
    }
}

impl Eq for PoolRecord {}


pub enum MempoolRequest {
    AddTransaction(TransferTx),
    GetPendingNonce(u32, Sender<Option<u32>>)
}


impl MemPool {

    pub fn new() -> Self {
        Self{
            batch_size : config::TRANSFER_BATCH_SIZE,
            // current_block: TransferBlock::default(),
            per_account_info: HashMap::new(),
            queue: OrdSet::new(),
        }
    }

    fn run(&mut self, rx_for_requests: Receiver<MempoolRequest>, tx_for_blocks: Sender<StateProcessingRequest>) {
        for req in rx_for_requests {            
            match req {
                MempoolRequest::AddTransaction(tx) => {
                    println!("adding tx to mem pool");
                    let add_result = self.add_transaction(tx);
                    if add_result.is_err() {
                        println!("Error adding transaction to mempool: {}", add_result.err().unwrap());
                    }
                    println!("Mempool queue length = {}", self.queue.len());
                    if self.queue.len() >= self.batch_size {
                        self.process_batch(&tx_for_blocks)
                    }
                },
                MempoolRequest::GetPendingNonce(account_id, channel) => {
                    self.get_pending_nonce(account_id, channel);
                },
            }
        }
    }

    fn add_transaction(&mut self, transaction: TransferTx) -> Result<(), String> {
        use bigdecimal::ToPrimitive;
        
        let from = transaction.from;
        match self.per_account_info.get_mut(&from) {
            Some(ordered_set) => {
                {   
                    println!("Accoutn {} already has a corresponding pool", from);
                    let existing_length = ordered_set.len();
                    if existing_length >= MAX_TRANSACTIONS_PER_ACCOUNT {
                        return Err("Too many transaction for this account".to_string());
                    }
                    let max = ordered_set.get_max();
                    if let Some(max_tx_nonce) = max {
                        let current_max_nonce = max_tx_nonce.nonce;
                        if transaction.nonce != current_max_nonce + 1 {
                            return Err("nonce is out of sequence".to_string());
                        }
                    }
                }
                let fee = transaction.fee.clone();
                let nonce = transaction.nonce;
                ordered_set.insert(transaction);
                let pool_record = PoolRecord{
                    fee: fee.to_u128().expect("fee must fit into 128 bits"),
                    nonce: nonce,
                    account_id: from
                };
                println!("Inserting pool record {:?}", pool_record);
                if let Some(replaced_value) = self.queue.insert(pool_record.clone()) {
                    println!("Has replaced {:?}", replaced_value);
                    if replaced_value.nonce == pool_record.nonce && 
                        replaced_value.fee > pool_record.fee 
                        {
                            self.queue.insert(replaced_value);
                        }
                }

                return Ok(());
            },
            None => {},
        }
        // here we happen to be only if current value is empty
        {
            let mut ordered_set = OrdSet::new();
            let fee = transaction.fee.clone();
            let nonce = transaction.nonce;
            ordered_set.insert(transaction);
            self.per_account_info.insert(from, ordered_set);
            let pool_record = PoolRecord{
                fee: fee.to_u128().expect("fee must fit into 128 bits"),
                nonce: nonce,
                account_id: from
            };
            println!("Inserting pool record {:?}", pool_record);
            if let Some(replaced_value) = self.queue.insert(pool_record.clone()) {
                println!("Has replaced {:?}", replaced_value);
                if replaced_value.nonce == pool_record.nonce && 
                    replaced_value.fee > pool_record.fee 
                    {
                        self.queue.insert(replaced_value);
                    }
            }

        }

        Ok(())
    }

    // fn process_batch(&mut self, tx_for_blocks: &Sender<StateProcessingRequest>) {

    //     // send the current block to state_keeper
    //     let block = std::mem::replace(&mut self.current_block, TransferBlock::default());
    //     let (tx, rx) = channel();
    //     let request = StateProcessingRequest::ApplyBlock(Block::Transfer(block), Some(tx));
    //     tx_for_blocks.send(request).expect("must send block processing request");

    //     // now wait for state_keeper to return a result
    //     let result = rx.recv().expect("must receive answer for block processing request");

    //     if let Err(block_purged) = result {
    //         // out block is returned purged
    //         if let Block::Transfer(block) = block_purged {
    //             self.current_block = block;
    //         }
    //     };
    // }

    fn process_batch(&mut self, tx_for_blocks: &Sender<StateProcessingRequest>) {
        println!("Will attempt to make a new block");

        if self.queue.len() < self.batch_size {
            println!("Queue length is not enough for a new block");
            return;
        }

        let max_attempts = 1000;

        let mut new_block = TransferBlock::default();

        let mut removed_items = vec![];

        for _ in 0..max_attempts {
            let item = self.queue.remove_min();
            if let Some(pool_item) = item {
                let transactions_per_account = self.per_account_info.get_mut(&pool_item.account_id).expect("transaction set must be in account info if it's in a pool");
                let transaction = transactions_per_account.remove_min().expect("transaction itself must be in a set if it's in a pool");
                removed_items.push((pool_item, transaction.clone()));

                new_block.transactions.push(transaction);

                if new_block.transactions.len() == self.batch_size {
                    println!("Has chosen enough transactions from the queue");
                    let (tx, rx) = channel();
                    let request = StateProcessingRequest::ApplyBlock(Block::Transfer(new_block.clone()), Some(tx));
                    tx_for_blocks.send(request).expect("must send block processing request");

                    // now wait for state_keeper to return a result
                    let result = rx.recv().expect("must receive answer for block processing request");

                    if let Err(block_purged) = result {
                        // out block is returned purged
                        if let Block::Transfer(block) = block_purged {
                            new_block = block;
                        }
                    } else {
                        return;
                    }
                }
            }
            else {
                break;
            }
        }

        // we did NOT assemble a block over max attempts, revert globally
        println!("Reverting a mempool");
        for removed_item in removed_items {
            let (pool_item, transaction) = removed_item;
            let account_id = pool_item.account_id;
            self.queue.insert(pool_item);
            self.per_account_info.get_mut(&account_id)
                .expect("account info set must exit at revert")
                .insert(transaction)
                .expect("inserting transaction back must work");
        }
        return;
    }

    pub fn get_pending_nonce(&self, account_id: u32, channel: Sender<Option<u32>>) {
        match self.per_account_info.get(&account_id) {
            Some(ordered_set) => {
                {
                    let max = ordered_set.get_max();
                    if let Some(max_tx_nonce) = max {
                        let current_max_nonce = max_tx_nonce.nonce;
                        channel.send(Some(current_max_nonce));
                        return;
                    }
                }
            },
            None => {},
        }

        channel.send(None);
    }

}

pub fn start_mem_pool(mut mem_pool: MemPool, rx_for_requests: Receiver<MempoolRequest>, tx_for_blocks: Sender<StateProcessingRequest>) {
        std::thread::Builder::new().name("mem_pool".to_string()).spawn(move || {  
            mem_pool.run(rx_for_requests, tx_for_blocks);
        });
}


#[test] 
fn test_set_insert() {
    let pool_record_0 = PoolRecord {
        fee: 0u128,
        nonce: 0,
        account_id: 2
    };

    let pool_record_1 = PoolRecord {
        fee: 0u128,
        nonce: 1,
        account_id: 2
    };

    let mut set = OrdSet::new();
    let r0 = set.insert(pool_record_0);
    let r1 = set.insert(pool_record_1);
    let len = set.len();
    assert_eq!(len, 2);
}