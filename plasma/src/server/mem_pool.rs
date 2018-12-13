use std::sync::mpsc::{channel, Sender, Receiver};
use crate::models::tx::TxUnpacked;
use crate::models::plasma_models::{Tx, TxBlock, Block};
use super::state_keeper::{BlockProcessingRequest, BlockSource};
use super::config;

pub struct MemPool {
    // Batch size
    pub batch_size: usize,

    // Accumulated transactions
    pub current_block: TxBlock,
}

impl MemPool {

    pub fn new() -> Self {
        Self{
            batch_size : config::TX_BATCH_SIZE,
            current_block: TxBlock::empty(),
        }
    }

    pub fn run(&mut self, rx_for_tx: Receiver<TxUnpacked>, tx_for_blocks: Sender<BlockProcessingRequest>) {
        for tx in rx_for_tx {            
            if let Ok(tx) = Tx::try_from(&tx) {
                println!("adding tx to mem pool");
                self.current_block.transactions.push(tx);
                if self.current_block.transactions.len() == self.batch_size {
                    self.process_batch(&tx_for_blocks)
                }
            } else {
                println!("invalid transaction: {:?}", tx);
                // TODO: any error handling needed here?
            }
        }
    }

    fn process_batch(&mut self, tx_for_blocks: &Sender<BlockProcessingRequest>) {

        // send the current block to state_keeper
        let block = std::mem::replace(&mut self.current_block, TxBlock::empty());
        let (tx, rx) = channel();
        let request = BlockProcessingRequest(Block::Tx(block), BlockSource::MemPool(tx));
        tx_for_blocks.send(request);

        // now wait for state_keeper to return a result
        let result = rx.recv().unwrap();

        if let Err(block_purged) = result {
            // out block is returned purged
            self.current_block = block_purged;
        };
    }
}