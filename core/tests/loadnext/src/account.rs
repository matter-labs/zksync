use std::todo;

use zksync::{RpcProvider, Wallet};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{AccountId, Address, H256};

use crate::{command::Command, config::LoadtestConfig};

#[derive(Debug)]
pub struct AccountLifespan {
    pub commands: Vec<Command>,
    pub wallet: Wallet<PrivateKeySigner, RpcProvider>,
}

impl AccountLifespan {
    pub fn new(config: &LoadtestConfig, wallet: Wallet<PrivateKeySigner, RpcProvider>) -> Self {
        Self {
            commands: vec![],
            wallet,
        }
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        Ok(())
    }
}
