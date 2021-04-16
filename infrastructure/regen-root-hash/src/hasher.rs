use zksync_crypto::{
    circuit::account::{CircuitAccount, CircuitAccountTree, CircuitBalanceTree},
    merkle_tree::SparseMerkleTree,
    Fr,
};

use crate::account::FromAccount;
use lazy_static::lazy_static;
use zksync_types::Account;

lazy_static! {
    pub static ref BALANCE_TREE_32: CircuitBalanceTree = SparseMerkleTree::new(32);
    pub static ref BALANCE_TREE_11: CircuitBalanceTree = SparseMerkleTree::new(11);
}

pub fn get_state_root_hash(
    accounts: &Vec<(i64, Account)>,
    balance_tree: &CircuitBalanceTree,
) -> Fr {
    let mut account_state_tree: CircuitAccountTree = SparseMerkleTree::new(32);

    for (id, account) in accounts {
        let circuit_account = CircuitAccount::from_account(account.clone(), &balance_tree);

        account_state_tree.insert(*id as u32, circuit_account);
    }

    account_state_tree.root_hash()
}
