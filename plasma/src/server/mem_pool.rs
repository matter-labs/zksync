use std::sync::mpsc::{channel, Sender, Receiver};
use crate::models::{TransferTx, TransferBlock, Block};
use super::state_keeper::{StateProcessingRequest, BlockSource};
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
            batch_size : config::TX_BATCH_SIZE,
            current_block: TransferBlock::default(),
        }
    }

    pub fn run(&mut self, rx_for_tx: Receiver<TransferTx>, tx_for_blocks: Sender<StateProcessingRequest>) {
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
        let request = StateProcessingRequest::ApplyBlock(Block::Transfer(block), BlockSource::MemPool(tx));
        tx_for_blocks.send(request);

        // now wait for state_keeper to return a result
        let result = rx.recv().unwrap();

        if let Err(block_purged) = result {
            // out block is returned purged
            self.current_block = block_purged;
        };
    }
}