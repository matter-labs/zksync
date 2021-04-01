use std::todo;

use zksync_types::{AccountId, Address, H256};

use crate::{command::Command, config::LoadtestConfig, rng::LoadtestRNG};

#[derive(Debug, Copy, Clone)]
pub enum AccountState {
    Uninitialized,
    Initialized(AccountId),
}

#[derive(Debug, Clone)]
pub struct AccountLifespan {
    pub commands: Vec<Command>,
    pub eth_private_key: H256,
    pub address: Address,
    pub nonce: u64,
    pub state: AccountState,
}

impl AccountLifespan {
    pub fn new(config: &LoadtestConfig, rng: impl LoadtestRNG) -> Self {
        todo!()
    }
}
