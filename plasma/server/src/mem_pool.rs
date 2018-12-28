use std::sync::mpsc::{channel, Sender, Receiver};
use plasma::models::{TransferTx, TransferBlock, Block};
use super::models::StateProcessingRequest;
use super::config;

pub struct MemPool {
    // Batch size
    pub batch_size: usize,

    // Accumulated transactions
    pub current_block: TransferBlock,
}

impl MemPool {

    pub fn new() -> Self {
        Self{
            batch_size : config::TRANSFER_BATCH_SIZE,
            current_block: TransferBlock::default(),
        }
    }

    fn run(&mut self, rx_for_tx: Receiver<TransferTx>, tx_for_blocks: Sender<StateProcessingRequest>) {
        for tx in rx_for_tx {            
            println!("adding tx to mem pool");
            self.current_block.transactions.push(tx);
            if self.current_block.transactions.len() == self.batch_size {
                self.process_batch(&tx_for_blocks)
            }
        }
    }

    fn process_batch(&mut self, tx_for_blocks: &Sender<StateProcessingRequest>) {

        // send the current block to state_keeper
        let block = std::mem::replace(&mut self.current_block, TransferBlock::default());
        let (tx, rx) = channel();
        let request = StateProcessingRequest::ApplyBlock(Block::Transfer(block), Some(tx));
        tx_for_blocks.send(request).expect("queue must work");

        // now wait for state_keeper to return a result
        let result = rx.recv().unwrap();

        if let Err(block_purged) = result {
            // out block is returned purged
            if let Block::Transfer(block) = block_purged {
                self.current_block = block;
            }
        };
    }

    fn get_latest_nonce(&self, address: u32) -> Option<u32> {
        None
    }
}

pub fn start_mem_pool(mut mem_pool: MemPool, rx_for_tx: Receiver<TransferTx>, tx_for_blocks: Sender<StateProcessingRequest>) {
        std::thread::Builder::new().name("mem_pool".to_string()).spawn(move || {  
            mem_pool.run(rx_for_tx, tx_for_blocks);
        });
}
