//! Common primitives used within testkit.
use num::BigUint;
use std::collections::HashMap;
use web3::types::TransactionReceipt;
use zksync_config::ConfigurationOptions;
use zksync_types::block::Block;
use zksync_types::TokenId;

#[derive(Debug)]
pub struct TestkitConfig {
    pub chain_id: u8,
    pub gas_price_factor: f64,
    pub web3_url: String,
}

impl TestkitConfig {
    pub fn from_env() -> Self {
        let env_config = ConfigurationOptions::from_env();
        TestkitConfig {
            chain_id: env_config.chain_id,
            gas_price_factor: env_config.gas_price_factor,
            web3_url: env_config.web3_url,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ETHAccountId(pub usize);

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ZKSyncAccountId(pub usize);

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Token(pub TokenId);

#[derive(Debug, Clone)]
pub struct BlockExecutionResult {
    pub block: Block,
    pub commit_result: TransactionReceipt,
    pub verify_result: TransactionReceipt,
    pub withdrawals_result: TransactionReceipt,
    pub block_size_chunks: usize,
}

impl BlockExecutionResult {
    pub fn new(
        block: Block,
        commit_result: TransactionReceipt,
        verify_result: TransactionReceipt,
        withdrawals_result: TransactionReceipt,
        block_size_chunks: usize,
    ) -> Self {
        Self {
            block,
            commit_result,
            verify_result,
            withdrawals_result,
            block_size_chunks,
        }
    }
}

// Struct used to keep expected balance changes after transactions execution.
#[derive(Default)]
pub struct ExpectedAccountState {
    pub eth_accounts_state: HashMap<(ETHAccountId, TokenId), BigUint>,
    pub sync_accounts_state: HashMap<(ZKSyncAccountId, TokenId), BigUint>,

    // Amount of withdraw operations performed in block.
    pub withdraw_ops: usize,
}
