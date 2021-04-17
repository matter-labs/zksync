use zksync_crypto::{
    circuit::account::{CircuitAccount, CircuitAccountTree, CircuitBalanceTree},
    merkle_tree::SparseMerkleTree,
    Fr,
};

use crate::account::FromAccount;
use once_cell::sync::Lazy;
use zksync_types::Account;

pub static BALANCE_TREE_32: Lazy<CircuitBalanceTree> = Lazy::new(|| SparseMerkleTree::new(32));
pub static BALANCE_TREE_11: Lazy<CircuitBalanceTree> = Lazy::new(|| SparseMerkleTree::new(11));

pub fn get_state_root_hash(accounts: &[(i64, Account)], balance_tree: &CircuitBalanceTree) -> Fr {
    let mut account_state_tree: CircuitAccountTree = SparseMerkleTree::new(32);

    for (id, account) in accounts {
        let circuit_account = CircuitAccount::from_account(account.clone(), &balance_tree);

        account_state_tree.insert(*id as u32, circuit_account);
    }

    account_state_tree.root_hash()
}
