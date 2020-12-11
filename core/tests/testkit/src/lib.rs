use zksync_core::state_keeper::ZkSyncStateInitParams;
use zksync_types::{Account, Address};

pub use self::{
    account_set::AccountSet, state_keeper_utils::spawn_state_keeper, test_setup::TestSetup,
    types::*,
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

/// Constant for testkit
/// Real value is in `dev.env`
pub const MAX_WITHDRAWALS_PER_BLOCK: u32 = 10;

/// Initialize plasma state with one account - fee account.
pub fn genesis_state(fee_account_address: &Address) -> ZkSyncStateInitParams {
    let operator_account = Account::default_with_address(fee_account_address);
    let mut params = ZkSyncStateInitParams::new();
    params.insert_account(0, operator_account);
    params
}
