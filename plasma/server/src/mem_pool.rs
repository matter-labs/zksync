use std::sync::mpsc::{channel, Sender, Receiver};
use plasma::models::{TransferTx, TransferBlock, Block, AccountId, Nonce};
use fnv::FnvHashMap;
use super::models::StateProcessingRequest;
use super::config;
use priority_queue::PriorityQueue;
use bigdecimal::BigDecimal;
use im::ordmap::OrdMap;
use num_traits::Zero;

const MAX_TRANSACTIONS_PER_ACCOUNT: usize = 128;

#[derive(Default)]
struct AccountTxQueue {
    pub queue: OrdMap<Nonce, TransferTx>,
}

const MAX_NONCE: Nonce = Nonce::max_value(); // simple trick to order queue ascending

pub type TxResult<T> = std::result::Result<T, String>;

/// AccountTxQueue is expected to always contain at least one tx!
impl AccountTxQueue {

    pub fn min_nonce(&self) -> Nonce {
        self.queue.get_min().unwrap().0
    }

    pub fn pending_nonce(&self) -> Nonce {
        let next_nonce = 0;
        for nonce in self.queue.keys() {
            if next_nonce != *nonce { break }
            next_nonce = nonce + 1;
        }
        next_nonce
    }

    pub fn next_fee(&self) -> BigDecimal {
        self.queue.values().next().unwrap().fee
    }

    /// Returns true if new item added
    pub fn insert(&mut self, tx: TransferTx) -> bool {
        self.queue.insert(tx.nonce, tx).is_none()
    }

    pub fn pop_next(&mut self) -> TransferTx {
        let (tx, queue) = self.queue.without_min();
        self.queue = queue;
        tx.unwrap()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    } 
}

#[derive(Default)]
struct TxQueue {
    queues: FnvHashMap<AccountId, AccountTxQueue>,
    order:  PriorityQueue<AccountId, BigDecimal>,
    len:    usize,
}

impl TxQueue {

    fn ensure_queue(&mut self, accountId: AccountId)  {
        if self.queues.get(&accountId).is_none() {
            self.queues.insert(accountId, AccountTxQueue::default());
            self.order.push(accountId, BigDecimal::zero());
        }
    }

    pub fn insert(&mut self, tx: TransferTx) {
        self.ensure_queue(tx.from);
        let queue = self.queues.get(&tx.from).unwrap();
        if queue.insert(tx) {
            self.len += 1;
        }
        self.order.change_priority(&tx.from, queue.next_fee());
    }

    pub fn batch_insert(&self, list: Vec<TransferTx>) {
        // TODO: optimize
        for tx in list.into_iter() {
            self.insert(tx);
        }
    }

    pub fn pop_next(&self) -> Option<TransferTx> {
        let next_account = self.order.peek().map(|(&accountId, _)| accountId);
        next_account.map(|accountId| {
            let (tx, emptied) = {
                let queue = self.queues[&accountId];
                let tx = queue.pop_next();
                self.len -= 1;
                (tx, queue.len() == 0)
            };
            if emptied {
                self.order.pop();
                self.queues.remove(&accountId);
            }
            tx
        })
    }

    pub fn pending_nonce(&self, account_id: AccountId) -> Option<Nonce> {
        self.queues.get(&account_id).map(|queue| queue.pending_nonce())
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

#[derive(Default)]
pub struct MemPool {
    // Batch size
    batch_requested:    bool,
    queue:              TxQueue,
}

impl std::iter::Iterator for MemPool {
    
    type Item = TransferTx;
    fn next(&mut self) -> Option<TransferTx> {
        self.queue.pop_next()
    }
}

pub enum MempoolRequest {
    AddTransaction(TransferTx),
    GetPendingNonce(AccountId, Sender<Option<Nonce>>),
    ProcessBatch,
}

impl MemPool {

    fn run(&mut self, 
        tx_for_requests: Sender<MempoolRequest>,
        rx_for_requests: Receiver<MempoolRequest>, 
        tx_for_blocks: Sender<StateProcessingRequest>) 
    {
        for req in rx_for_requests {            
            match req {
                MempoolRequest::AddTransaction(tx) => {
                    let add_result = self.add_transaction(tx);
                    if let Err(err) = add_result {
                        println!("error adding transaction to mempool: {}", err);
                        // TODO: return error message to api server
                    } else {
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
                    self.process_batch(&tx_for_blocks);
                },
                MempoolRequest::GetPendingNonce(account_id, channel) => {
                    channel.send(self.queue.pending_nonce(account_id));
                },
            }
        }
    }

    fn add_transaction(&mut self, transaction: TransferTx) -> TxResult<()> {
        println!("adding tx to mem pool");

        if let Some(queue) = self.queue.queues.get(&transaction.from) {
            if queue.len() >= MAX_TRANSACTIONS_PER_ACCOUNT {
                return Err(format!("Too many transactions in the queue for this account"))
            }

            let pending_nonce = queue.pending_nonce();
            if transaction.nonce != pending_nonce {
                return Err(format!("Nonce is out of sequence: expected {}, got {}", pending_nonce, transaction.nonce))
            }
        }

        self.queue.insert(transaction);
        // TODO: commit to database
        Ok(())
    }

    fn process_batch(&mut self, tx_for_blocks: &Sender<StateProcessingRequest>) {

        // send request to state_keeper
        let (tx, rx) = channel();
        let request = StateProcessingRequest::ApplyTransferBlock(self);
        tx_for_blocks.send(request).expect("must send block processing request");

        // now wait for state_keeper to return a result
        let result = rx.recv().expect("must receive answer for block processing request");
        if let Err((valid, invalid)) = result {
            self.queue.batch_insert(valid)
            // TODO: remove invalid transactions from db
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


#[test] 
fn test_mempool() {

}