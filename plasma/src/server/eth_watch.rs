use std::sync::mpsc::Sender;
use crate::models::plasma_models::{Block};

pub struct EthWatch {

}

impl EthWatch {

    pub fn new() -> Self {
        Self{}
    }

    pub fn run(&mut self, tx_for_blocks: Sender<Block>) {

    }

}