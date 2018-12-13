use std::sync::mpsc::{Sender, Receiver};
use crate::models::tx::TxUnpacked;
use crate::models::plasma_models::{Tx, TxBlock, Block};
use super::config;

pub struct MemPool {
    // Batch size
    pub batch_size: usize,

    // Accumulated transactions
    pub current_batch: Vec<Tx>,
}

impl MemPool {

    pub fn new() -> Self {
        Self{
            batch_size : config::TX_BATCH_SIZE,
            current_batch: vec![],
        }
    }

    pub fn run(&mut self, rx_for_tx: Receiver<TxUnpacked>, tx_for_blocks: Sender<Block>) {
        for tx in rx_for_tx {            
            if let Ok(tx) = Tx::try_from(&tx) {
                println!("adding tx to mem pool");
                self.current_batch.push(tx);
                if self.current_batch.len() == self.batch_size {
                    self.process_batch(&tx_for_blocks)
                }
            } else {
                println!("invalid transaction: {:?}", tx);
                // TODO: any error handling needed here?
            }
        }
    }

    fn process_batch(&mut self, tx_for_blocks: &Sender<Block>) {
        let transactions = std::mem::replace(&mut self.current_batch, Vec::with_capacity(self.batch_size));
        let block = TxBlock::with(transactions);
        tx_for_blocks.send(Block::Tx(block));
    }
}