pub mod contract;
pub mod data_restore_driver;
pub mod database_storage_interactor;
pub mod eth_tx_helpers;
pub mod events;
pub mod events_state;
pub mod inmemory_storage_interactor;
pub mod rollup_ops;
pub mod storage_interactor;
pub mod tree_state;

#[cfg(test)]
mod tests;

use crate::storage_interactor::StorageInteractor;
use zksync_types::{tokens::get_genesis_token_list, TokenId};

// How many blocks we will process at once.
pub const ETH_BLOCKS_STEP: u64 = 10_000;
pub const END_ETH_BLOCKS_OFFSET: u64 = 40;

pub async fn add_tokens_to_storage<I: StorageInteractor>(interactor: &mut I, eth_network: &str) {
    let genesis_tokens =
        get_genesis_token_list(&eth_network).expect("Initial token list not found");
    for (id, token) in (1..).zip(genesis_tokens) {
        let add_token_log = format!(
            "Adding token: {}, id:{}, address: {}, decimals: {}",
            &token.symbol, id, &token.address, &token.decimals
        );
        interactor.store_token(token, TokenId(id)).await;
        vlog::info!("{}", add_token_log);
    }
}
