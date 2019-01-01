use std::sync::{Arc, mpsc::{channel, Sender, Receiver}};
use plasma::models::{TransferTx, TransferBlock, Block, AccountId, Nonce};
use fnv::FnvHashMap;
use super::models::{StateProcessingRequest, AppliedTransactions, RejectedTransactions};
use super::config;
use priority_queue::PriorityQueue;
use bigdecimal::BigDecimal;
use im::ordmap::OrdMap;
use num_traits::Zero;
use std::borrow::BorrowMut;

const MAX_TRANSACTIONS_PER_ACCOUNT: usize = 128;
const MAX_SEARCH_DEPTH: usize = 4;
const TX_LIFETIME: std::time::Duration = std::time::Duration::from_secs(3600);

use plasma::models::{Account};

#[derive(Debug, Clone)]
struct InPoolTransaction{
    pub timestamp: std::time::Instant,
    pub lifetime: std::time::Duration,
    pub transaction: TransferTx,
}

enum TransactionPickerRequest{
    ProvideBlock(TransferTx),
}

enum TransactionPickerResponse{
    Accepted(Vec<InPoolTransaction>),
    TemporaryRejected(Vec<InPoolTransaction>),
    RejectedCompletely(Vec<InPoolTransaction>),
}

impl Default for InPoolTransaction {
    fn default() -> Self {
        Self{
            timestamp: std::time::Instant::now(),
            lifetime: TX_LIFETIME,
            transaction: TransferTx::default(),
        }
    }
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
    queue: OrdMap<Nonce, InPoolTransaction>,
    current_nonce: Nonce,
    distance: u32,
    default_nonce: Nonce,
    in_order_nonce: Nonce,
}


impl PerAccountQueue {
    // Create a new per account queue from the account state
    pub fn new(account_state: Account) -> Self {
        let current_nonce = account_state.nonce;

        Self {
            queue: OrdMap::new(),
            current_nonce: current_nonce,
            distance: 0,
            default_nonce: current_nonce,
            in_order_nonce: current_nonce,
        }
    }

    /// Returns true if new item added
    pub fn insert(&mut self, tx: TransferTx) -> bool {
        let nonce = tx.nonce;

        let mut value = None;
        {
            value = self.queue.get(&nonce).cloned();
        }

        if value.is_some() {
            // TODO: implement replacement by fee
            // uniqueness check to prevent spam must be done on higher level
            return false;
        } else {
            let in_pool_tx = InPoolTransaction {
                timestamp: std::time::Instant::now(),
                lifetime: TX_LIFETIME,
                transaction: tx
            };

            if nonce < self.default_nonce {
                // no insertion of pre-taken or outdated transactions
                return false;
            }
            if nonce == self.in_order_nonce {
                self.in_order_nonce += 1;
            }

            return self.queue.insert(nonce, in_pool_tx).is_none();
        }

        
    }

    /// Get fee for nonce
    pub fn get_fee(&self, nonce: Nonce) -> Option<BigDecimal> {
        self.queue.get(&nonce).map(|v| v.transaction.fee.clone())
    }

    /// Get minimal expected nonce in the queue
    fn min_nonce(&self) -> Nonce {
        // self.default_nonce

        self.queue.values().next().map(|v| v.transaction.nonce).unwrap_or(self.default_nonce)
        // self.queue.get_min().map(|(k,_)| *k).unwrap_or(self.default_nonce)
    }

    /// Get nonce already in the queue
    fn max_nonce(&self) -> Option<Nonce> {
        // if self.queue.len() == 0 {
        //     return None;
        // }

        // Some(self.current_nonce)

        self.queue.values().last().map(|v| v.transaction.nonce)
        // self.queue.get_max().map(|(k,_)| *k + 1).unwrap_or(self.current_nonce)
    }

    /// Get next expected nonce without gaps
    fn pending_nonce(&self) -> Nonce {
        self.in_order_nonce

        // self.queue.values().last().map(|v| v.transaction.nonce + 1).unwrap_or(self.current_nonce)
        // self.queue.get_max().map(|(k,_)| *k + 1).unwrap_or(self.current_nonce)
    }

    pub fn next_fee(&self) -> Option<BigDecimal> {
        self.queue.values().next().map(|v| v.transaction.fee.clone())
    }

    // take an item from the queue. Move the queue pointer to this nonce value and do nothing else
    pub fn take(&mut self, expected_nonce: Nonce) -> Option<InPoolTransaction> {
        if self.in_order_nonce >= expected_nonce {
            // there were no gaps before, so it's allowed to take

            if self.current_nonce != expected_nonce {
                // can not take not the next one
                return None;
            }
            // we've may be taken some transactions from the per-account pool already, so give the next one
            if let Some(tx) = self.queue.get(&self.current_nonce) {
                self.current_nonce += 1;
                self.distance += 1;
                return Some(tx.clone());
            }

            return None;
        }

        // it's not allowed to take nonce with gaps
        None
    }

    // reorganize the queue due to transaction being accepted, temporary or completely rejected
    pub fn reorganize(&mut self, reason: TransactionPickerResponse) {
        match reason {
            TransactionPickerResponse::Accepted(transactions) => {
                // check that an array of accepted transactions starts with the same nonce as the current queue
                if let Some(transaction) = transactions.first() {
                    let nonce = transaction.transaction.nonce;
                    if nonce != self.min_nonce() {
                        panic!("Account queue is in inconsistent state!");
                    }
                }
                // just move a virtual pointer to the begining of the queue
                if let Some(transaction) = transactions.last() {
                    let nonce = transaction.transaction.nonce;
                    let distance_to_skip = self.distance + nonce - self.current_nonce;
                    self.queue = self.queue.skip(distance_to_skip as usize);
                    self.distance = 0;
                    self.current_nonce = nonce;
                    self.default_nonce = nonce;
                }
            },
            TransactionPickerResponse::TemporaryRejected(transactions) => {
                // don't need to check for a first item, just check how far from the begining transactions
                // were rejected and if any one of those should be pushed out from the pool - just purge the rest too
                let mut max_alive_nonce = self.default_nonce;
                for tx in transactions {
                    if tx.timestamp + tx.lifetime <= std::time::Instant::now() {
                        break
                    } else {
                        max_alive_nonce = tx.transaction.nonce;
                    }
                }

                if max_alive_nonce != self.current_nonce {
                    // one nonce is already dead, so purge everything after
                    let distance = self.distance + max_alive_nonce - self.current_nonce;
                    self.queue = self.queue.take(distance as usize);
                    self.distance = distance;
                    self.current_nonce = max_alive_nonce;
                }
            },
            TransactionPickerResponse::RejectedCompletely(transactions) => {
                if let Some(tx) = transactions.first() {
                    let distance = self.distance + tx.transaction.nonce - self.current_nonce;
                    self.queue = self.queue.take(distance as usize);
                    self.distance = distance;
                    self.current_nonce = tx.transaction.nonce;
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    } 

}

// #[derive(Default, Debug, Clone)]
// pub struct TxQueue {
//     queues: FnvHashMap<AccountId, AccountTxQueue>,
//     order:  PriorityQueue<AccountId, BigDecimal>,
//     len:    usize,
// }

// // For state_keeper::create_transfer_block()
// impl TxQueue {

//     pub fn peek_next(&self) -> Option<AccountId> {
//         self.order.peek().map(|(&id, _)| id)
//     }

//     /// next() must be called immediately after peek_next(), so that the queue for account_id exists
//     pub fn next(&mut self, account_id: AccountId, next_nonce: Nonce) -> (RejectedTransactions, Option<TransferTx>) {
//         assert_eq!(account_id, self.peek_next().unwrap());
//         let (rejected, tx, next_fee) = {
//             let queue = self.queues.get_mut(&account_id).unwrap();
//             let (rejected, tx) = queue.pop(next_nonce);
//             let ejected = rejected.len() + if tx.is_some() {1} else {0};
//             self.len -= ejected;
//             (rejected, tx, queue.next_fee())
//         };
//         if let Some(next_fee) = next_fee {
//             // update priority
//             self.order.change_priority(&account_id, next_fee);
//         } else {
//             // remove empty queue
//             self.order.pop();
//             self.queues.remove(&account_id);
//         }
//         (rejected, tx)
//     }
// }

// impl TxQueue {

//     fn ensure_queue(&mut self, account_id: AccountId)  {
//         if self.queues.get(&account_id).is_none() {
//             self.queues.insert(account_id, AccountTxQueue::default());
//             self.order.push(account_id, BigDecimal::zero());
//         }
//     }

//     fn insert(&mut self, tx: TransferTx) {
//         let from = tx.from;
//         self.ensure_queue(from);
//         let queue = self.queues.get_mut(&from).unwrap();
//         if queue.insert(tx) {
//             self.len += 1;
//         }
//         self.order.change_priority(&from, queue.next_fee().unwrap());
//     }

//     fn batch_insert(&mut self, list: Vec<TransferTx>) {
//         // TODO: optimize performance: group by accounts, then update order once per account
//         for tx in list.into_iter() {
//             self.insert(tx);
//         }
//     }

//     fn pending_nonce(&self, account_id: AccountId) -> Option<Nonce> {
//         self.queues.get(&account_id).map(|queue| queue.pending_nonce())
//     }

//     fn len(&self) -> usize {
//         self.len
//     }
// }


// #[derive(Default)]
// pub struct MemPool {
//     // Batch size
//     batch_requested:    bool,
//     queue:              TxQueue,
// }

// pub enum MempoolRequest {
//     AddTransaction(TransferTx),
//     GetPendingNonce(AccountId, Sender<Option<Nonce>>),
//     ProcessBatch,
// }

// impl MemPool {

//     fn run(&mut self, 
//         tx_for_requests: Sender<MempoolRequest>,
//         rx_for_requests: Receiver<MempoolRequest>, 
//         tx_for_blocks: Sender<StateProcessingRequest>) 
//     {
//         for req in rx_for_requests {            
//             match req {
//                 MempoolRequest::AddTransaction(tx) => {
//                     let add_result = self.add_transaction(tx);
//                     if let Err(err) = add_result {
//                         println!("error adding transaction to mempool: {}", err);
//                         // TODO: return error message to api server
//                     } else {
//                         println!("mempool queue length = {}", self.queue.len());
//                         // TODO: also check that batch is now possible (e.g. that Ethereum queue is not too long)
//                         if !self.batch_requested && self.queue.len() >= config::TRANSFER_BATCH_SIZE {
//                             println!("batch processing requested");
//                             self.batch_requested = true;
//                             tx_for_requests.send(MempoolRequest::ProcessBatch);
//                         }
//                     }
//                 },
//                 MempoolRequest::ProcessBatch => {
//                     self.batch_requested = false;
//                     let do_padding = false; // TODO: use when neccessary
//                     self.process_batch(do_padding, &tx_for_blocks);
//                 },
//                 MempoolRequest::GetPendingNonce(account_id, channel) => {
//                     channel.send(self.queue.pending_nonce(account_id));
//                 },
//             }
//         }
//     }

//     fn add_transaction(&mut self, transaction: TransferTx) -> TxResult<()> {
//         println!("adding tx to mem pool");

//         if let Some(queue) = self.queue.queues.get(&transaction.from) {
//             if queue.len() >= MAX_TRANSACTIONS_PER_ACCOUNT {
//                 return Err(format!("Too many transactions in the queue for this account"))
//             }

//             if let Some(existing_fee) = queue.get_fee(transaction.nonce) {
//                 if existing_fee > transaction.fee {
//                     return Err(format!("Transaction for nonce {} already in the pool with higher fee {} (new fee is {})", 
//                         transaction.nonce, existing_fee, transaction.fee))
//                 }
//             } else {
//                 let pending_nonce = queue.pending_nonce();
//                 if transaction.nonce != pending_nonce {
//                     return Err(format!("Nonce is out of sequence: expected {}, got {}", pending_nonce, transaction.nonce))
//                 }
//             }
//         }

//         self.queue.insert(transaction);
//         // TODO: commit to database
//         Ok(())
//     }

//     fn process_batch(&mut self, do_padding: bool, tx_for_blocks: &Sender<StateProcessingRequest>) {

//         // send request to state_keeper
//         let (tx, rx) = channel();

//         // move ownership of queue to the state_keeper thread 
//         let queue = std::mem::replace(&mut self.queue, TxQueue::default());

//         let request = StateProcessingRequest::CreateTransferBlock(queue, do_padding, tx);
//         tx_for_blocks.send(request).expect("must send block processing request");

//         // now wait for state_keeper to return a result
//         let (queue, result) = rx.recv().expect("must receive answer for block processing request");

//         // take ownership of queue back
//         self.queue = queue;

//         match result {
//             Ok((applied, block_number)) => {
//                 // TODO: remove applied, block_number, wait here for committer instead
//             },
//             Err((valid, invalid)) => {
//                 println!("creating transfer block failed: {} transactions rejected, {} going back to queue", invalid.len(), valid.len());
//                 self.queue.batch_insert(valid)
//                 // TODO: remove invalid transactions from db
//             },
//         };
//     }

// }

// pub fn start_mem_pool(mut mem_pool: MemPool, 
//     tx_for_requests: Sender<MempoolRequest>, 
//     rx_for_requests: Receiver<MempoolRequest>, 
//     tx_for_blocks: Sender<StateProcessingRequest>) 
// {
//     std::thread::Builder::new().name("mem_pool".to_string()).spawn(move || {  
//         mem_pool.run(tx_for_requests, rx_for_requests, tx_for_blocks);
//     });
// }


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

    assert_eq!(q.min_nonce(), 5);
    assert_eq!(q.max_nonce(), None);

    assert_eq!(q.next_fee(), None);

    // insert some tx for nonce = 5
    assert_eq!(q.insert(test::tx(1, 5, 20)), true);
    assert_eq!(q.len(), 1);
    assert_eq!(q.insert(test::tx(1, 5, 20)), false);
    assert_eq!(q.len(), 1);
    assert_eq!(q.next_fee().unwrap(), BigDecimal::from(20));

    // next nonce is at 6
    assert_eq!(q.pending_nonce(), 6);

    // allow to insert nonce = 7 even while out of order
    assert_eq!(q.insert(test::tx(1, 7, 40)), true);
    assert_eq!(q.len(), 2);
    assert_eq!(q.next_fee().unwrap(), BigDecimal::from(20));

    assert_eq!(q.get_fee(7).unwrap(), BigDecimal::from(40));
    assert_eq!(q.get_fee(5).unwrap(), BigDecimal::from(20));
    // there is no tx for 6, so it's none
    assert_eq!(q.get_fee(6), None);

    // one can not take transactions for nonce 6 or 7
    assert!(q.take(6).is_none());
    assert!(q.take(7).is_none());

    // one take take a transaction number 5
    assert!(q.take(5).is_some());
    // but not twice
    assert!(q.take(5).is_none());
    

    // let _q = q;

    // let mut q = _q.clone();
    // let (rejected, tx) = q.pop(5);
    // assert_eq!(rejected.len(), 0); 
    // assert_eq!(tx.unwrap().nonce, 5); 
    // assert_eq!(q.len(), 1);
    // assert_eq!(q.next_fee().unwrap(), BigDecimal::from(40));
    // assert_eq!(q.pending_nonce(), 8);

    // let mut q = _q.clone();

    // assert_eq!(q.insert(test::tx(1, 5, 60)), false);
    // assert_eq!(q.get_fee(5).unwrap(), BigDecimal::from(60));

    // let mut q = _q.clone();
    // let (rejected, tx) = q.pop(6);
    // assert_eq!(rejected.len(), 2); 
    // assert_eq!(tx.is_none(), true);
    // assert_eq!(q.len(), 0);
    // assert_eq!(q.next_fee(), None);

    // let mut q = _q.clone();
    // let (rejected, tx) = q.pop(7);
    // assert_eq!(rejected.len(), 1); 
    // assert_eq!(tx, None);
    // assert_eq!(q.len(), 1);
    // assert_eq!(q.pending_nonce(), 8);

    // let mut q = _q.clone();
    // let (rejected, tx) = q.pop(8);
    // assert_eq!(rejected.len(), 2); 
    // assert_eq!(tx.is_none(), true);
    // assert_eq!(q.pending_nonce(), 0);

    // let mut q = _q.clone();
    // assert_eq!(q.insert(test::tx(1, 6, 40)), true);
    // let (rejected, tx) = q.pop(6);
    // assert_eq!(rejected.len(), 1); 
    // assert_eq!(tx, None);

    // let (rejected, tx) = q.pop(6);
    // assert_eq!(rejected.len(), 0); 
    // assert_eq!(tx.unwrap().nonce, 6);
    // assert_eq!(q.len(), 1);
    // assert_eq!(q.next_fee().unwrap(), BigDecimal::from(40));
    // assert_eq!(q.pending_nonce(), 8);

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