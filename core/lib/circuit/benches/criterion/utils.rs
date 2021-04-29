// External deps
use num::BigUint;
// Workspace deps
use zksync_crypto::circuit::{account::CircuitAccount, CircuitAccountTree};
use zksync_state::state::ZkSyncState;
use zksync_test_account::ZkSyncAccount;
use zksync_types::{Account, AccountId, AccountMap, Address, BlockNumber, TokenId};

// Public re-exports
use std::str::FromStr;
pub use zksync_circuit::witness::utils::WitnessBuilder;

pub const FEE_ACCOUNT_ID: AccountId = AccountId(0);

/// Helper structure to generate `ZkSyncState` and `CircuitAccountTree`.
#[derive(Debug)]
pub struct ZkSyncStateGenerator;

impl ZkSyncStateGenerator {
    fn create_state(accounts: AccountMap) -> (ZkSyncState, CircuitAccountTree) {
        let plasma_state = ZkSyncState::from_acc_map(accounts, BlockNumber(1));

        let mut circuit_account_tree =
            CircuitAccountTree::new(zksync_crypto::params::account_tree_depth());
        for (id, account) in plasma_state.get_accounts() {
            circuit_account_tree.insert(id, CircuitAccount::from(account))
        }

        (plasma_state, circuit_account_tree)
    }

    pub fn generate(accounts: &[WitnessTestAccount]) -> (ZkSyncState, CircuitAccountTree) {
        let accounts: Vec<_> = accounts
            .iter()
            .map(|acc| (acc.id, acc.account.clone()))
            .collect();

        let accounts = if accounts.iter().any(|(id, _)| *id == FEE_ACCOUNT_ID) {
            accounts.into_iter().collect()
        } else {
            std::iter::once((
                FEE_ACCOUNT_ID,
                Account::default_with_address(
                    &Address::from_str("feeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap(),
                ),
            ))
            .chain(accounts)
            .collect()
        };

        Self::create_state(accounts)
    }
}

/// A helper structure for witness tests which contains both testkit
/// zkSync account and an actual zkSync account.
#[derive(Debug)]
pub struct WitnessTestAccount {
    pub zksync_account: ZkSyncAccount,
    pub id: AccountId,
    pub account: Account,
}

impl WitnessTestAccount {
    pub fn new(id: AccountId, balance: u64) -> Self {
        let zksync_account = ZkSyncAccount::rand();
        zksync_account.set_account_id(Some(id));

        let account = {
            let mut account = Account::default_with_address(&zksync_account.address);
            account.add_balance(TokenId(0), &BigUint::from(balance));
            account.pub_key_hash = zksync_account.pubkey_hash;
            account
        };

        Self {
            zksync_account,
            id,
            account,
        }
    }
}
