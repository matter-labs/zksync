use std::sync::{Arc, mpsc::{channel, Sender, Receiver}};
use plasma::models::{TransferTx, TransferBlock, Block, AccountId, Nonce};
use fnv::{FnvHashMap, FnvHashSet};
use super::models::{StateProcessingRequest, BlockAssemblyResponse, InPoolTransaction, TransactionPickerResponse};
use super::config;
use super::state_keeper::PlasmaStateKeeper;
use priority_queue::PriorityQueue;
use bigdecimal::BigDecimal;
use im::ordmap::OrdMap;
use num_traits::Zero;
use std::borrow::BorrowMut;
use std::sync::mpsc::{sync_channel, SyncSender};

const MAX_QUEUE_SIZE: usize = 1 << 16;
const MAX_TRANSACTIONS_PER_ACCOUNT: usize = 16;
const MAX_SEARCH_DEPTH: usize = 4;
const TX_LIFETIME: std::time::Duration = std::time::Duration::from_secs(3600);
const RETUTATION_PRICE: u128 = 0;
const MAX_GAP: u32 = 4;

use plasma::models::{Account};

impl Default for InPoolTransaction {
    fn default() -> Self {
        Self{
            timestamp: std::time::Instant::now(),
            lifetime: TX_LIFETIME,
            transaction: TransferTx::default(),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct UniqueTxFilter {
    pub set: FnvHashSet<Vec<u8>>
}

#[derive(Default, Debug, Clone)]
pub struct TxQueue {
    filter: UniqueTxFilter,
    queues: FnvHashMap<AccountId, PerAccountQueue>,
    order:  PriorityQueue<AccountId, BigDecimal>,
    len:    usize,
}

pub struct MemPool {
    // Batch size
    batch_requested:    bool,
    buffer:             (SyncSender<InPoolTransaction>, Receiver<InPoolTransaction>),
    queue:              TxQueue,
}

impl Default for MemPool {
    fn default() -> Self {
        Self {
            batch_requested: false,
            buffer: sync_channel(MAX_QUEUE_SIZE),
            queue: TxQueue::default(),
        }
    }
}

impl MemPool {
    pub fn new(state_keeper: &PlasmaStateKeeper) -> Self {
        let mut queue = TxQueue::default();
        for (k, v) in &state_keeper.state.balance_tree.items {
            let new_per_account = PerAccountQueue::new(v.clone());
            println!("Creating individual queue for an account {}", *k);
            queue.queues.insert(*k, new_per_account);
        }

        Self {
            batch_requested: false,
            buffer: sync_channel(MAX_QUEUE_SIZE),
            queue: queue,
        }
    }
}

impl NonceClient for MemPool {
    /// Get minimal nonce for this account that would allow replacement
    fn min_nonce(&self, account: u32) -> Nonce {
        if let Some(per_account_queue) = self.queue.queues.get(&account) {
            return per_account_queue.min_nonce();
        }
        
        0
    }


    /// Get max nonce already in the queue, including gapped txes
    fn max_nonce(&self, account: u32) -> Option<Nonce> {
        if let Some(per_account_queue) = self.queue.queues.get(&account) {
            return per_account_queue.max_nonce();
        }
        
        None
    }

    /// Get next expected nonce without gaps
    fn next_nonce(&self, account: u32) -> Nonce {
        if let Some(per_account_queue) = self.queue.queues.get(&account) {
            return per_account_queue.next_nonce();
        }
        
        0
    }
}

pub enum MempoolRequest {
    AddTransaction(TransferTx, Sender<Result<(), String>>),
    GetPendingNonce(AccountId, Sender<Option<Nonce>>),
    ProcessBatch,
}

// this memory pool is responsible for both accumulation of transactions 
// and for continuous attempts to find an optimal next batch
pub struct TransactionPicker {
    pub in_memory_state: FnvHashMap<u32, Account>,
    pub request_queue: Receiver<()>,
    pub padding_pool: Vec<TransferTx>,
}

#[derive(Default, Debug, Clone)]
struct PerAccountQueue {
    reputation: i32,
    queue: OrdMap<Nonce, InPoolTransaction>,
    pointer: u32,
    // minimal nonce allowed for this account
    minimal_nonce: Nonce,
    // the last taken nonce
    current_nonce: Nonce,
    // max nonce that has no gaps before it
    next_nonce_without_gaps: Nonce,
}

pub trait NonceClient {
    /// Get minimal nonce for this account that would allow replacement
    fn min_nonce(&self, account: u32) -> Nonce;

    /// Get max nonce already in the queue, including gapped txes
    fn max_nonce(&self, account: u32) -> Option<Nonce>;

    /// Get next expected nonce without gaps
    fn next_nonce(&self, account: u32) -> Nonce;
}

impl PerAccountQueue {
    // Create a new per account queue from the account state
    pub fn new(account_state: Account) -> Self {
        let current_nonce = account_state.nonce;

        println!("Created with starting nonces = {}", current_nonce);

        Self {
            reputation: 0i32,
            queue: OrdMap::new(),
            pointer: 0,
            minimal_nonce: current_nonce,
            current_nonce: current_nonce,
            next_nonce_without_gaps: current_nonce,
        }
    }

    /// Returns true if new item added
    pub fn insert(&mut self, tx: TransferTx) -> Result<bool, String> {
        let nonce = tx.nonce;
        let from = tx.from;

        let mut value = None;

        {
            value = self.queue.get(&nonce).cloned();
        }

        if let Some(value) = value {
            if tx.fee > value.transaction.fee {
                // replacement by fee
                let in_pool_tx = InPoolTransaction {
                    timestamp: std::time::Instant::now(),
                    lifetime: TX_LIFETIME,
                    transaction: tx
                };

                self.queue.insert(nonce, in_pool_tx);
                println!("Replaced transaction for account {}, nonce {} by fee", from, nonce);
                return Ok(false);
            }

            return Err(format!("Replacement transaction is underpriced"));
        } else {
            let in_pool_tx = InPoolTransaction {
                timestamp: std::time::Instant::now(),
                lifetime: TX_LIFETIME,
                transaction: tx
            };

            if nonce < self.minimal_nonce {
                // no insertion of pre-taken or outdated transactions
                println!("Trying to insert a transaction with too old nonce");
                return Err(format!("Trying to insert into the part already booked for previous batches"));
            }
            
            if self.queue.len() >= MAX_TRANSACTIONS_PER_ACCOUNT {
                println!("Transaction length is too large");
                return Err(format!("Too many pending transaction per account"));
            }

            if nonce == self.next_nonce_without_gaps {
                println!("Increased in-order transaction nonce");
                self.next_nonce_without_gaps += 1;
                println!("New nonce without gaps = {}", self.next_nonce_without_gaps);
                // check, we may have had transactions in the pool after the gap and now can fill it
                loop {
                    if self.queue.get(&self.next_nonce_without_gaps).is_some() {
                        self.next_nonce_without_gaps += 1;
                        println!("New nonce without gaps = {}", self.next_nonce_without_gaps);
                    } else {
                        break;
                    }
                }
            }
            // else if nonce > self.next_nonce_without_gaps {
            //     return Err(format!("Inserting nonce out of sequence is not allowed for now"));
            // }

            if nonce > self.next_nonce_without_gaps + MAX_GAP {
                println!("Inserting this far into the future is pointless");
                return Err(format!("Inserting nonce too far into the future"));
            }

            if self.queue.insert(nonce, in_pool_tx).is_none() {
                println!("Successfully inserted a fresh transaction in the pool");
                return Ok(true);
            } else {
                // println!("Replaced some old tx");
                // return Ok(false);
                println!("Failed to insert a transaction");
                return Err(format!("Could not insert a transaction for some reason"));
            }
        }        
    }

    /// Get fee for nonce
    pub fn get_fee(&self, nonce: Nonce) -> Option<BigDecimal> {
        self.queue.get(&nonce).map(|v| v.transaction.fee.clone())
    }

    /// Get minimal expected nonce in the queue
    fn min_nonce(&self) -> Nonce {
        self.minimal_nonce
        // self.queue.values().next().map(|v| v.transaction.nonce).unwrap_or(self.minimal_nonce)
        // self.queue.get_min().map(|(k,_)| *k).unwrap_or(self.default_nonce)
    }

    /// Get nonce already in the queue
    fn max_nonce(&self) -> Option<Nonce> {
        self.queue.values().last().map(|v| v.transaction.nonce)
    }

    /// Get next expected nonce without gaps
    fn next_nonce(&self) -> Nonce {
        self.next_nonce_without_gaps
    }

    fn order_and_clear(&mut self) {
        assert_eq!(self.pointer, 0, "cleanup must not happen when there is batch processing in place");
        if self.pointer != 0 {
            // reorg mut not happed during batch processing, when something is taken
            return;
        }
        let begining = self.minimal_nonce;
        let end = self.max_nonce().unwrap_or(begining) + 1;
        let mut in_order_candidate = begining;
        for i in begining..end {
            if let Some(in_pool_tx) = self.queue.get(&i).cloned() {
                let alive = in_pool_tx.timestamp + in_pool_tx.lifetime > std::time::Instant::now();
                if (alive) {
                    if in_pool_tx.transaction.nonce == in_order_candidate {
                        in_order_candidate += 1;
                    }
                } else {
                    self.queue.remove(&i);
                }
            }
        }
        println!("After cleanup new nonce without gaps = {}", in_order_candidate);
        self.next_nonce_without_gaps = in_order_candidate;
    }

    pub fn next_fee(&self) -> Option<BigDecimal> {
        // println!("Current nonce = {}", self.current_nonce);

        self.queue.get(&self.current_nonce).map(|v| v.transaction.fee.clone())
        // self.queue.values().next().map(|v| v.transaction.fee.clone())
    }

    // take an item from the queue. Move the queue pointer to this nonce value and do nothing else
    pub fn next(&mut self, expected_nonce: Nonce) -> Option<InPoolTransaction> {
        if expected_nonce >= self.next_nonce_without_gaps {
            return None;
        }
        // there were no gaps before, so it's allowed to take

        if self.current_nonce != expected_nonce {
            // can not take not the next one
            return None;
        }
        // we've may be taken some transactions from the per-account pool already, so give the next one
        if let Some(tx) = self.queue.get(&self.current_nonce) {
            self.current_nonce += 1;
            self.pointer += 1;
            return Some(tx.clone());
        }

        None
    }

    // reorganize the queue due to transaction being accepted, temporary or completely rejected
    pub fn reorganize(&mut self, reason: TransactionPickerResponse) {
        match reason {
            TransactionPickerResponse::Included(transaction) => {
                println!("Removing included transaction form the pool");
                // all calls here are expected to be ordered by nonce
                let old_length = self.queue.len();
                let nonce = transaction.transaction.nonce;
                if nonce != self.minimal_nonce {
                    panic!("Account queue is in inconsistent state!");
                }
                self.minimal_nonce += 1;
                self.queue.remove(&nonce);
                if self.current_nonce > self.minimal_nonce {
                    self.current_nonce = self.minimal_nonce;
                }
                self.pointer = 0;

                let new_length = self.queue.len();
                assert_eq!(old_length, new_length + 1);
            },
            TransactionPickerResponse::ValidButNotIncluded(transaction) => {
                println!("Returning transaction to the pool without prejustice");
                let old_length = self.queue.len();
                let nonce = transaction.transaction.nonce;
                println!("Current nonce = {}", self.current_nonce);
                println!("Returned nonce = {}", nonce);
                if nonce > self.current_nonce {
                    // no action is required
                    println!("Returned transaction is with the nonce higher than the current, do nothing");
                    return;
                }
                if nonce < self.minimal_nonce {
                    panic!("Account queue is in inconsistent state!");
                }
                if nonce <= self.current_nonce {
                    // assert!(self.pointer != 0, "on queue resets it should have something taken out");
                    // this transaction was either current or somewhere before, so we reset the queue
                    self.pointer = 0;
                    self.current_nonce = self.minimal_nonce;
                }
                let new_length = self.queue.len();
                assert_eq!(old_length, new_length);
                // no actions about the queue

            },
            TransactionPickerResponse::TemporaryRejected(transaction) => {
                // don't need to check for a first item, just check how far from the begining transactions
                // were rejected and if any one of those should be pushed out from the pool - just purge the rest too
                println!("Returning transaction to the pool with penalties");
                let nonce = transaction.transaction.nonce;
                if nonce < self.minimal_nonce {
                    panic!("Account queue is in inconsistent state!");
                }

                let old_length = self.queue.len();
                            
                if transaction.timestamp + transaction.lifetime <= std::time::Instant::now() {
                    self.queue.remove(&nonce);
                    if nonce <= self.next_nonce_without_gaps {
                        self.next_nonce_without_gaps = nonce - 1;
                    }
                    let new_length = self.queue.len();
                    assert_eq!(old_length, new_length+1);
                    // this transaction is dead, so purge it
                } else {
                    let nonce = transaction.transaction.nonce;
                    if self.queue.get(&nonce).is_some() {
                        self.queue.insert(nonce, transaction);
                    }           
                    let new_length = self.queue.len();
                    assert_eq!(old_length, new_length);         
                }
                if nonce > self.current_nonce {
                    // do nothing, it was purged already, 
                    return;
                }
                if nonce <= self.current_nonce {
                    // assert!(self.pointer != 0, "on queue resets it should have something taken out");
                    // this transaction was either current or somewhere before, so we reset the queue
                    self.pointer = 0;
                    self.current_nonce = self.minimal_nonce;
                }
            },
            TransactionPickerResponse::RejectedCompletely(transaction) => {
                println!("Removing transaction from the pool due to rejection");
                // just delete this one and all after
                let old_length = self.queue.len();
                let mut nonce = transaction.transaction.nonce;
                if nonce < self.minimal_nonce {
                    panic!("Account queue is in inconsistent state!");
                }
                self.queue.remove(&nonce);
                let new_length = self.queue.len();
                assert_eq!(old_length, new_length+1); 
                if nonce > self.current_nonce {
                    // do nothing, it was purged already, 
                    return;
                }
                if nonce <= self.next_nonce_without_gaps {
                    self.next_nonce_without_gaps = nonce - 1;
                }
                if nonce <= self.current_nonce {
                    // assert!(self.pointer != 0, "on queue resets it should have something taken out");
                    // this transaction was either current or somewhere before, so we reset the queue
                    self.pointer = 0;
                    self.current_nonce = self.minimal_nonce;
                }
            }
        }
        // TODO: this is inefficient and may be moved to higher level, but for now it's ok
        self.order_and_clear();
    }

    pub fn len(&self) -> usize {
        (self.next_nonce_without_gaps - self.minimal_nonce) as usize
        // self.queue.len()
    } 

}

// // For state_keeper::create_transfer_block()
impl TxQueue {

    pub fn peek_next(&self) -> Option<AccountId> {
        self.order.peek().map(|(&id, _)| id)
    }

    /// next() must be called immediately after peek_next(), so that the queue for account_id exists
    pub fn next(&mut self, account_id: AccountId, next_nonce: Nonce) -> Option<InPoolTransaction> {
        println!("Picking next transaction from the queue for account {} and nonce {}", account_id, next_nonce);
        if self.peek_next().is_none() {
            println!("Peek returned none");
            return None;
        }
        assert_eq!(account_id, self.peek_next().unwrap());
        let (tx, next_fee) = {
            let queue = self.queues.get_mut(&account_id).unwrap();
            let tx = queue.next(next_nonce);
            let ejected = if tx.is_some() {1} else {0};
            if ejected == 1 {
                println!("peeked transaction from priority queue");
            }
            // self.len -= ejected;
            (tx, queue.next_fee())
        };
        if let Some(next_fee) = next_fee {
            // update priority
            // pushing a duplicate is equivalent to update
            println!("There is a next fee for this account, so update");
            self.order.change_priority(&account_id, next_fee);
        } else {
            println!("There is no next fee for this account, pop from the queue");
            // remove current account from the queue
            self.order.pop();
        }

        tx
    }
}

impl TxQueue {

    fn ensure_queue(&mut self, account_id: AccountId)  {
        if self.queues.get(&account_id).is_none() {
            self.queues.insert(account_id, PerAccountQueue::default());
            self.order.push(account_id, BigDecimal::zero());
        }
    }

    fn insert(&mut self, tx: TransferTx) -> Result<(), String> {
        let tx_data = tx.tx_data();
        if tx_data.is_none() {
            println!("Trying to insert a malformed tx");
            return Err(format!("Trying to insert malformed transaction"));
        }
        let data = tx_data.unwrap();
        if self.filter.set.get(&data).is_some() {
            println!("Trying to insert a duplicate");
            return Err(format!("Trying to add a complete duplicate"));
        }

        let from = tx.from;
        self.ensure_queue(from);
        let queue = self.queues.get_mut(&from).expect("queue must be ensured");
        let old_length = queue.len();
        let insertion_result = queue.insert(tx);
        if insertion_result.is_err() {
            println!("Failed to insert a transaction");
            return Err(insertion_result.err().unwrap());
        }
        let next_fee = queue.next_fee();

        if insertion_result.unwrap() {
            println!("Inserted a new transaction");
            if let Some(next_fee) = next_fee {
                println!("Next fee for account {} = {}", from, next_fee);
                self.order.push(from, next_fee);
            }
        } else {
            println!("Replaced some transaction");
            if let Some(next_fee) = next_fee {
                println!("Next fee for account {} = {}", from, next_fee);
                self.order.push(from, next_fee);
            }
        }

        self.filter.set.insert(data);
        let new_length = queue.len();
        println!("Inserted something into the queue, old len = {}, new len = {}", old_length, new_length);

        self.len += new_length;
        self.len -= old_length;

        Ok(())
    }

    fn process_response(&mut self, response: BlockAssemblyResponse, block_was_assembled: bool) -> bool {
        let BlockAssemblyResponse {included, valid_but_not_included, temporary_rejected, completely_rejected, affected_accounts} = response;
        let mut old_lengths: FnvHashMap<u32, usize> = FnvHashMap::default();

        let initial_length = self.len();
        let mut total_removed: usize = 0;

        println!("Total affected account in this block = {}", affected_accounts.len());

        for from in affected_accounts.clone() {
            let queue = self.queues.get(&from).expect("queue is never discarded even when empty");
            let old_length = queue.len();
            old_lengths.insert(from, old_length);
        }

        if block_was_assembled {
            // accept all transacitons
            for pool_tx in included {
                let from = pool_tx.transaction.from;
                let queue = self.queues.get_mut(&from).expect("queue is never discarded even when empty");
                let tx_data = pool_tx.transaction.tx_data().expect("transaction in response is always almost valid");
                queue.reorganize(TransactionPickerResponse::Included(pool_tx));
                self.filter.set.remove(&tx_data);
                total_removed += 1;
            }
        } else {
            // return transactions
            for pool_tx in valid_but_not_included {
                let from = pool_tx.transaction.from;
                let queue = self.queues.get_mut(&from).expect("queue is never discarded even when empty");
                let tx_data = pool_tx.transaction.tx_data().expect("transaction in response is always almost valid");
                queue.reorganize(TransactionPickerResponse::ValidButNotIncluded(pool_tx));
            }
        }
        for pool_tx in temporary_rejected {
            // modify the transaction lifetime
            let mut modified_tx = pool_tx.clone();
            let from = pool_tx.transaction.from;
            let queue = self.queues.get_mut(&from).expect("queue is never discarded even when empty");
            let tx_data = pool_tx.transaction.tx_data().expect("transaction in response is always almost valid");
            modified_tx.lifetime = modified_tx.lifetime / 2;
            queue.reorganize(TransactionPickerResponse::TemporaryRejected(modified_tx));
        }

        for pool_tx in completely_rejected {
            let from = pool_tx.transaction.from;
            let queue = self.queues.get_mut(&from).expect("queue is never discarded even when empty");
            let tx_data = pool_tx.transaction.tx_data().expect("transaction in response is always almost valid");
            queue.reorganize(TransactionPickerResponse::RejectedCompletely(pool_tx));
            self.filter.set.remove(&tx_data);
            total_removed += 1;
        }

        println!("Updating priorities for affected accounts");
        for account in affected_accounts {
            let queue = self.queues.get(&account).expect("queue is never discarded even when empty");
            if let Some(fee) = queue.next_fee() {
                self.order.push(account, fee);
            }
        }

        for (k, v) in old_lengths {
            let queue = self.queues.get(&k).expect("queue is never discarded even when empty");
            let new_length = queue.len();
            println!("Queue length for account {} changed from {} to {}", k, v, new_length);
            self.len += new_length;
            self.len -= v;
        }

        let final_length = self.len();

        println!("Done processing reponse, old queue length = {}, new queue length = {}, total remove = {}", 
            initial_length, final_length, total_removed);

        if total_removed == 0 && final_length == initial_length {
            // should not try again immediately
            return false;
        }

        true

    }

    fn len(&self) -> usize {
        self.len
    }
}

impl MemPool {
    fn run(&mut self, 
        tx_for_requests: Sender<MempoolRequest>,
        rx_for_requests: Receiver<MempoolRequest>, 
        tx_for_blocks: Sender<StateProcessingRequest>) 
    {
        for req in rx_for_requests {            
            match req {
                MempoolRequest::AddTransaction(tx, sender) => {
                    let add_result = self.add_transaction(tx);
                    if let Err(err) = add_result {
                        println!("error adding transaction to mempool: {}", err);
                        sender.send(Err(err));
                        // TODO: return error message to api server
                    } else {
                        sender.send(Ok(()));
                        println!("mempool queue length = {}", self.queue.len());
                        // TODO: also check that batch is now possible (e.g. that Ethereum queue is not too long)
                        if !self.batch_requested && self.queue.len() >= config::TRANSFER_BATCH_SIZE {
                            println!("batch processing requested");
                            self.batch_requested = true;
                            tx_for_requests.send(MempoolRequest::ProcessBatch);
                        }
                    }
                },
                MempoolRequest::ProcessBatch => {
                    self.batch_requested = false;
                    let do_padding = false; // TODO: use when neccessary
                    if !self.batch_requested && self.queue.len() >= config::TRANSFER_BATCH_SIZE {
                        let may_try_again = self.process_batch(do_padding, &tx_for_blocks);
                        if !self.batch_requested && self.queue.len() >= config::TRANSFER_BATCH_SIZE && may_try_again {
                            println!("After previous response processing we can already make a new one");
                            self.batch_requested = true;
                            tx_for_requests.send(MempoolRequest::ProcessBatch);
                        }
                    }
                },
                MempoolRequest::GetPendingNonce(account_id, channel) => {
                    channel.send(Some(self.next_nonce(account_id)));
                },
            }
        }
    }

    fn add_transaction(&mut self, transaction: TransferTx) -> Result<(), String> {
        println!("adding tx to mem pool");

        let result = self.queue.insert(transaction);
        if result.is_err() {
            return result;
        }
        // TODO: commit to database
        Ok(())
    }

    fn process_batch(&mut self, do_padding: bool, tx_for_blocks: &Sender<StateProcessingRequest>) -> bool{

        // send request to state_keeper
        let (tx, rx) = channel();

        // move ownership of queue to the state_keeper thread 
        let queue = std::mem::replace(&mut self.queue, TxQueue::default());

        let request = StateProcessingRequest::CreateTransferBlock(queue, do_padding, tx);
        tx_for_blocks.send(request).expect("must send block processing request");

        // now wait for state_keeper to return a result
        let (queue, result) = rx.recv().expect("must receive answer for block processing request");

        // take ownership of queue back
        self.queue = queue;

        match result {
            Ok((response, block_number)) => {
                println!("created transfer block: {} transactions rejected, {} accepted, {} returned back to queue", 
                    response.completely_rejected.len(), 
                    response.included.len(),
                    response.temporary_rejected.len()
                );
                let may_try_again = self.queue.process_response(response, true);

                return may_try_again;
                // TODO: remove applied, block_number, wait here for committer instead
            },
            Err(response) => {
                println!("creating transfer block failed: {} transactions rejected, {} going back to queue", 
                    response.completely_rejected.len(), 
                    response.temporary_rejected.len() + response.valid_but_not_included.len()
                );
                let may_try_again = self.queue.process_response(response, false);

                return may_try_again;
                // TODO: remove invalid transactions from db
            },
        };
    }
}

pub fn start_mem_pool(mut mem_pool: MemPool, 
    tx_for_requests: Sender<MempoolRequest>, 
    rx_for_requests: Receiver<MempoolRequest>, 
    tx_for_blocks: Sender<StateProcessingRequest>) 
{
    std::thread::Builder::new().name("mem_pool".to_string()).spawn(move || {  
        mem_pool.run(tx_for_requests, rx_for_requests, tx_for_blocks);
    });
}


#[cfg(test)]
mod test {

    use plasma::models::*;
    use bigdecimal::BigDecimal;

    pub fn tx(from: AccountId, nonce: u32,  fee: u32) -> TransferTx {
        let mut tx = TransferTx::default();
        tx.from = from;
        tx.nonce = nonce;
        tx.fee = BigDecimal::from(fee);
        tx
    }

}

#[test] 
fn test_per_account_queue() {

    let mut acc = Account::default();
    acc.nonce = 5;

    let mut q = PerAccountQueue::new(acc);

    assert_eq!(q.min_nonce(), 5, "minimal nonce mismatch");
    assert_eq!(q.next_nonce(), 5, "next nonce mismatch");
    assert_eq!(q.max_nonce(), None, "max nonce mismatch");

    assert_eq!(q.next_fee(), None, "next fee mismatch");

    // insert some tx for nonce = 5
    assert!(q.insert(test::tx(1, 5, 20)).is_ok(), "must insert a new tx");
    assert_eq!(q.len(), 1, "queue length mismatch after one insert");
    assert!(q.insert(test::tx(1, 5, 20)).is_err(), "must not insert a new tx without replacement");
    assert_eq!(q.len(), 1, "queue length must not change");
    assert_eq!(q.next_fee().unwrap(), BigDecimal::from(20), "next fee mismatch");

    // next nonce is at 6
    assert_eq!(q.next_nonce(), 6, "next expected nonce mismatch");

    // allow to insert nonce = 7 even while out of order
    assert!(q.insert(test::tx(1, 7, 40)).is_ok(), "should allow to insert out of order");
    assert!(q.queue.get(&7).is_some(), "insertion must in fact happen");
    assert_eq!(q.len(), 1, "queue len must not change if transaction is out of order");
    assert_eq!(q.next_fee().unwrap(), BigDecimal::from(20), "next fee must not change");

    assert_eq!(q.get_fee(7).unwrap(), BigDecimal::from(40), "can get fee for some place in queue");
    assert_eq!(q.get_fee(5).unwrap(), BigDecimal::from(20), "can get fee for some place in queue");
    // there is no tx for 6, so it's none
    assert_eq!(q.get_fee(6), None, "must be empty fee");

    // one can not take transactions for nonce 6 or 7
    assert!(q.next(6).is_none(), "must not take out of order tx");
    assert!(q.next(7).is_none(), "must not take out of order tx");

    assert_eq!(q.current_nonce, 5, "current nonce must be at the begining");

    // one take take a transaction number 5
    let next = q.next(5);
    assert!(next.is_some(), "must take first transaction in the queue");
    assert_eq!(q.current_nonce, 6, "must update current nonce");
    assert_eq!(q.pointer, 1, "must update pointer");
    // but not twice
    assert!(q.next(5).is_none(), "must not take same transaction twice");
    
    let _q = q;
    let mut q = _q.clone();

    let next = next.unwrap();

    let response = TransactionPickerResponse::Included(next.clone());
    q.reorganize(response);
    assert_eq!(q.len(), 0, "queue length is not empty");
    assert_eq!(q.current_nonce, 6, "current nonce does not change");
    assert_eq!(q.min_nonce(), 6, "minimal nonce changes");
    assert_eq!(q.pointer, 0, "pointer is decreased");
    assert!(q.insert(test::tx(1, 6, 30)).is_ok(), "should allow to insert in order");
    assert_eq!(q.next_nonce_without_gaps, 8, "recalculate next nonce without gaps");
    assert_eq!(q.len(), 2, "queue length must be updated on insert");
    assert_eq!(q.next_fee().unwrap(), BigDecimal::from(30), "next fee must match");

    let mut q = _q.clone();
    let response = TransactionPickerResponse::ValidButNotIncluded(next.clone());
    q.reorganize(response);
    assert_eq!(q.len(), 1, "queue length is not empty");
    assert_eq!(q.current_nonce, 5, "current nonce does not change");
    assert_eq!(q.min_nonce(), 5, "minimal nonce changes");
    assert_eq!(q.pointer, 0, "pointer is decreased");
    assert!(q.insert(test::tx(1, 6, 30)).is_ok(), "should allow to insert in order");
    assert_eq!(q.next_nonce_without_gaps, 8, "recalculate next nonce without gaps after return of the valid tx");
    assert_eq!(q.len(), 3, "queue length must be updated on insert");
    assert_eq!(q.next_fee().unwrap(), BigDecimal::from(20), "next must not update");

    let mut q = _q.clone();
    let response = TransactionPickerResponse::TemporaryRejected(next.clone());
    q.reorganize(response);
    assert_eq!(q.len(), 1, "queue length is not empty");
    assert_eq!(q.current_nonce, 5, "current nonce does not change");
    assert_eq!(q.min_nonce(), 5, "minimal nonce changes");
    assert_eq!(q.pointer, 0, "pointer is decreased");
    assert!(q.insert(test::tx(1, 6, 30)).is_ok(), "should allow to insert in order");
    assert_eq!(q.next_nonce_without_gaps, 8, "recalculate next nonce without gaps after return of the valid tx");
    assert_eq!(q.len(), 3, "queue length must be updated on insert");
    assert_eq!(q.next_fee().unwrap(), BigDecimal::from(20), "next must not update");

    let mut q = _q.clone();
    let mut modified_next = next.clone();
    modified_next.timestamp = modified_next.timestamp - modified_next.lifetime;
    let response = TransactionPickerResponse::TemporaryRejected(modified_next);
    q.reorganize(response);
    assert_eq!(q.len(), 0, "queue length is empty");
    assert_eq!(q.current_nonce, 5, "current nonce does not change");
    assert_eq!(q.min_nonce(), 5, "minimal nonce changes");
    assert_eq!(q.pointer, 0, "pointer is decreased");
    assert!(q.insert(test::tx(1, 6, 30)).is_ok(), "should allow to insert out of order");
    assert_eq!(q.next_nonce_without_gaps, 5, "nonce without gaps should point to the begining");
    assert_eq!(q.len(), 0, "queue length must be updated on insert");
    assert_eq!(q.next_fee(), None, "next fee must be missing");

    let mut q = _q.clone();
    let response = TransactionPickerResponse::RejectedCompletely(next.clone());
    q.reorganize(response);
    assert_eq!(q.len(), 0, "queue length is empty");
    assert_eq!(q.current_nonce, 5, "current nonce does not change");
    assert_eq!(q.min_nonce(), 5, "minimal nonce changes");
    assert_eq!(q.pointer, 0, "pointer is decreased");
    assert!(q.insert(test::tx(1, 6, 30)).is_ok(), "should allow to insert out of order");
    assert_eq!(q.next_nonce_without_gaps, 5, "nonce without gaps should point to the begining");
    assert_eq!(q.len(), 0, "queue length must be updated on insert");
    assert_eq!(q.next_fee(), None, "next fee must be missing");
}

// #[test] 
// fn test_tx_queue() {
//     let mut q = TxQueue::default();
//     assert_eq!(q.peek_next(), None);

//     q.insert(test::tx(1, 5, 20));
//     assert_eq!(q.len(), 1);
//     assert_eq!(q.peek_next().unwrap(), 1);

//     q.insert(test::tx(2, 0, 40));
//     assert_eq!(q.len(), 2);
//     assert_eq!(q.peek_next().unwrap(), 2);

//     q.insert(test::tx(1, 6, 50));
//     assert_eq!(q.len(), 3);
//     assert_eq!(q.peek_next().unwrap(), 2);

//     q.insert(test::tx(1, 5, 50));
//     assert_eq!(q.len(), 3);
//     assert_eq!(q.peek_next().unwrap(), 1);

//     let _q = q;

//     let mut q = _q.clone();
//     let (rejected, tx) = q.next(1, 5);
//     assert_eq!(rejected.len(), 0);
//     assert_eq!(tx.as_ref().unwrap().from, 1);
//     assert_eq!(tx.as_ref().unwrap().nonce, 5);
//     assert_eq!(tx.as_ref().unwrap().fee, BigDecimal::from(50));
//     assert_eq!(q.len(), 2);
//     assert_eq!(q.peek_next().unwrap(), 1);

//     let mut q = _q.clone();
//     let (rejected, tx) = q.next(1, 6);
//     assert_eq!(rejected.len(), 1);
//     assert_eq!(tx, None);
//     assert_eq!(q.len(), 2);
//     assert_eq!(q.peek_next().unwrap(), 1);

//     let (rejected, tx) = q.next(1, 6);
//     assert_eq!(rejected.len(), 0);
//     assert_eq!(tx.as_ref().unwrap().from, 1);
//     assert_eq!(tx.as_ref().unwrap().nonce, 6);
//     assert_eq!(tx.as_ref().unwrap().fee, BigDecimal::from(50));
//     assert_eq!(q.len(), 1);
//     assert_eq!(q.peek_next().unwrap(), 2);

//     let (rejected, tx) = q.next(2, 0);
//     assert_eq!(rejected.len(), 0);
//     assert_eq!(tx.as_ref().unwrap().from, 2);
//     assert_eq!(tx.as_ref().unwrap().nonce, 0);
//     assert_eq!(q.len(), 0);
//     assert_eq!(q.peek_next(), None);
// }