pub mod data_restore_driver;
pub mod database_storage_interactor;
pub mod inmemory_storage_interactor;
pub mod storage_interactor;
pub mod tree_state;

pub mod config;
#[cfg(test)]
mod tests;
pub mod utils;

use crate::storage_interactor::StorageInteractor;
use zksync_types::{tokens::get_genesis_token_list, TokenId};

// How many blocks we will process at once.
pub const ETH_BLOCKS_STEP: u64 = 10000;
pub const END_ETH_BLOCKS_OFFSET: u64 = 40;

pub async fn add_tokens_to_storage(interactor: &mut StorageInteractor<'_>, eth_network: &str) {
    let genesis_tokens = get_genesis_token_list(eth_network).expect("Initial token list not found");
    for (id, token) in (1..).zip(genesis_tokens) {
        let add_token_log = format!(
            "Adding token: {}, id:{}, address: {}, decimals: {}",
            &token.symbol, id, &token.address, &token.decimals
        );
        interactor.store_token(token, TokenId(id)).await;
        vlog::info!("{}", add_token_log);
    }
}
