use zksync_types::{Account, AccountId, Address};

pub use self::{
    account_set::AccountSet, state_keeper_utils::spawn_state_keeper, test_setup::TestSetup,
    types::*,
};

use num::BigUint;
use zksync_core::state_keeper::ZkSyncStateInitParams;
use zksync_crypto::params::{
    MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ADDRESS, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID,
};
pub use zksync_test_account as zksync_account;

pub mod account_set;
pub mod data_restore;
pub mod eth_account;
pub mod external_commands;
pub mod scenarios;
pub mod state_keeper_utils;
pub mod test_setup;
pub mod types;

/// Initialize plasma state with one account - fee account.
pub fn genesis_state(fee_account_address: &Address) -> ZkSyncStateInitParams {
    let operator_account = Account::default_with_address(fee_account_address);
    let mut params = ZkSyncStateInitParams::new();
    params.insert_account(AccountId(0), operator_account);
    let mut nft_storage = Account::default_with_address(&NFT_STORAGE_ACCOUNT_ADDRESS);
    nft_storage.set_balance(NFT_TOKEN_ID, BigUint::from(MIN_NFT_TOKEN_ID));
    params.insert_account(NFT_STORAGE_ACCOUNT_ID, nft_storage);
    params
}
