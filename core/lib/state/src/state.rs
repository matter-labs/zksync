use anyhow::Error;
use num::BigUint;
use std::collections::HashMap;
use zksync_crypto::{params, Fr};
use zksync_types::{
    helpers::reverse_updates,
    operations::{TransferOp, TransferToNewOp, ZkSyncOp},
    Account, AccountId, AccountMap, AccountTree, AccountUpdate, AccountUpdates, Address,
    BlockNumber, SignedZkSyncTx, TokenId, ZkSyncPriorityOp, ZkSyncTx,
};

use crate::handler::TxHandler;

#[derive(Debug)]
pub struct OpSuccess {
    pub fee: Option<CollectedFee>,
    pub updates: AccountUpdates,
    pub executed_op: ZkSyncOp,
}

#[derive(Debug, Clone)]
pub struct ZkSyncState {
    /// Accounts stored in a sparse Merkle tree
    balance_tree: AccountTree,

    account_id_by_address: HashMap<Address, AccountId>,

    /// Current block number
    pub block_number: BlockNumber,
}

#[derive(Debug, Clone)]
pub struct CollectedFee {
    pub token: TokenId,
    pub amount: BigUint,
}

/// Helper enum to unify Transfer / TransferToNew operations.
#[derive(Debug)]
pub enum TransferOutcome {
    Transfer(TransferOp),
    TransferToNew(TransferToNewOp),
}

impl TransferOutcome {
    pub fn into_franklin_op(self) -> ZkSyncOp {
        match self {
            Self::Transfer(transfer) => transfer.into(),
            Self::TransferToNew(transfer) => transfer.into(),
        }
    }
}

impl ZkSyncState {
    pub fn empty() -> Self {
        let tree_depth = params::account_tree_depth();
        let balance_tree = AccountTree::new(tree_depth);
        Self {
            balance_tree,
            block_number: 0,
            account_id_by_address: HashMap::new(),
        }
    }

    pub fn from_acc_map(accounts: AccountMap, current_block: BlockNumber) -> Self {
        let mut empty = Self::empty();
        empty.block_number = current_block;
        for (id, account) in accounts {
            empty.insert_account(id, account);
        }
        empty
    }

    pub fn new(
        balance_tree: AccountTree,
        account_id_by_address: HashMap<Address, AccountId>,
        current_block: BlockNumber,
    ) -> Self {
        Self {
            balance_tree,
            block_number: current_block,
            account_id_by_address,
        }
    }

    pub fn get_accounts(&self) -> Vec<(u32, Account)> {
        self.balance_tree
            .items
            .iter()
            .map(|a| (*a.0 as u32, a.1.clone()))
            .collect()
    }

    pub fn root_hash(&self) -> Fr {
        self.balance_tree.root_hash()
    }

    pub fn get_account(&self, account_id: AccountId) -> Option<Account> {
        let start = std::time::Instant::now();

        let account = self.balance_tree.get(account_id).cloned();

        log::trace!(
            "Get account (id {}) execution time: {}ms",
            account_id,
            start.elapsed().as_millis()
        );

        account
    }

    pub fn chunks_for_batch(&self, txs: &[SignedZkSyncTx]) -> usize {
        txs.iter().map(|tx| self.chunks_for_tx(tx)).sum()
    }

    pub fn chunks_for_tx(&self, franklin_tx: &ZkSyncTx) -> usize {
        match franklin_tx {
            ZkSyncTx::Transfer(tx) => {
                if self.get_account_by_address(&tx.to).is_some() {
                    TransferOp::CHUNKS
                } else {
                    TransferToNewOp::CHUNKS
                }
            }
            _ => franklin_tx.min_chunks(),
        }
    }

    /// Priority op execution should not fail.
    pub fn execute_priority_op(&mut self, op: ZkSyncPriorityOp) -> OpSuccess {
        match op {
            ZkSyncPriorityOp::Deposit(op) => self
                .apply_tx(op)
                .expect("Priority operation execution failed"),
            ZkSyncPriorityOp::FullExit(op) => self
                .apply_tx(op)
                .expect("Priority operation execution failed"),
        }
    }

    /// Applies account updates.
    /// Assumes that all updates are correct, panics otherwise.
    pub fn apply_account_updates(&mut self, updates: AccountUpdates) {
        for (account_id, account_update) in updates {
            match account_update {
                AccountUpdate::Create { address, nonce } => {
                    assert!(self.get_account_by_address(&address).is_none());

                    let mut account = Account::default();
                    account.address = address;
                    account.nonce = nonce;
                    self.insert_account(account_id, account);
                }
                AccountUpdate::Delete { address, nonce } => {
                    let account = self
                        .get_account(account_id)
                        .expect("account to delete must exist");
                    assert_eq!(account.address, address);
                    assert_eq!(account.nonce, nonce);

                    self.remove_account(account_id);
                }
                AccountUpdate::UpdateBalance {
                    old_nonce,
                    new_nonce,
                    balance_update: (token_id, old_balance, new_balance),
                } => {
                    let mut account = self
                        .get_account(account_id)
                        .expect("account to update balance must exist");
                    assert_eq!(account.get_balance(token_id), old_balance);
                    assert_eq!(account.nonce, old_nonce);

                    account.set_balance(token_id, new_balance.clone());
                    account.nonce = new_nonce;
                    self.insert_account(account_id, account);
                }
                AccountUpdate::ChangePubKeyHash {
                    old_pub_key_hash,
                    new_pub_key_hash,
                    old_nonce,
                    new_nonce,
                } => {
                    let mut account = self
                        .get_account(account_id)
                        .expect("account to change pubkey must exist");
                    assert_eq!(account.pub_key_hash, old_pub_key_hash);
                    assert_eq!(account.nonce, old_nonce);

                    account.pub_key_hash = new_pub_key_hash.clone();
                    account.nonce = new_nonce;
                    self.insert_account(account_id, account);
                }
            }
        }
    }

    pub fn execute_txs_batch(&mut self, txs: &[SignedZkSyncTx]) -> Vec<Result<OpSuccess, Error>> {
        let mut successes = Vec::new();

        for (id, tx) in txs.iter().enumerate() {
            match self.execute_tx(tx.tx.clone()) {
                Ok(success) => {
                    successes.push(Ok(success));
                }
                Err(error) => {
                    // Restore the state that was observed before the batch execution.
                    successes.reverse();
                    for success in successes {
                        let mut updates = success
                            .expect("successes should not contain an error")
                            .updates;
                        reverse_updates(&mut updates);
                        self.apply_account_updates(updates);
                    }

                    // Create message for an error.
                    let error_msg = format!(
                        "Batch execution failed, since tx #{} of batch failed with a reason: {}",
                        id + 1,
                        error
                    );

                    // Create the same error for each transaction.
                    let errors = (0..txs.len())
                        .map(|_| Err(anyhow::format_err!("{}", error_msg)))
                        .collect();

                    // Stop execution and return an error.
                    return errors;
                }
            }
        }

        successes
    }

    pub fn execute_tx(&mut self, tx: ZkSyncTx) -> Result<OpSuccess, Error> {
        match tx {
            ZkSyncTx::Transfer(tx) => self.apply_tx(*tx),
            ZkSyncTx::Withdraw(tx) => self.apply_tx(*tx),
            ZkSyncTx::Close(tx) => self.apply_tx(*tx),
            ZkSyncTx::ChangePubKey(tx) => self.apply_tx(*tx),
            ZkSyncTx::ForcedExit(tx) => self.apply_tx(*tx),
        }
    }

    pub(crate) fn get_free_account_id(&self) -> AccountId {
        // TODO check for collisions.
        self.balance_tree.items.len() as u32
    }

    pub fn collect_fee(&mut self, fees: &[CollectedFee], fee_account: AccountId) -> AccountUpdates {
        let mut updates = Vec::new();

        let mut account = self.get_account(fee_account).unwrap_or_else(|| {
            panic!(
                "Fee account should be present in the account tree: {}",
                fee_account
            )
        });

        for fee in fees {
            if fee.amount == BigUint::from(0u32) {
                continue;
            }

            let old_amount = account.get_balance(fee.token).clone();
            let nonce = account.nonce;
            account.add_balance(fee.token, &fee.amount);
            let new_amount = account.get_balance(fee.token).clone();

            updates.push((
                fee_account,
                AccountUpdate::UpdateBalance {
                    balance_update: (fee.token, old_amount, new_amount),
                    old_nonce: nonce,
                    new_nonce: nonce,
                },
            ));
        }

        self.insert_account(fee_account, account);

        updates
    }

    pub fn get_account_by_address(&self, address: &Address) -> Option<(AccountId, Account)> {
        let account_id = *self.account_id_by_address.get(address)?;
        Some((
            account_id,
            self.get_account(account_id)
                .expect("Failed to get account by cached pubkey"),
        ))
    }

    #[doc(hidden)] // Public for benches.
    pub fn insert_account(&mut self, id: AccountId, account: Account) {
        self.account_id_by_address.insert(account.address, id);
        self.balance_tree.insert(id, account);
    }

    #[allow(dead_code)]
    pub(crate) fn remove_account(&mut self, id: AccountId) {
        if let Some(account) = self.get_account(id) {
            self.account_id_by_address.remove(&account.address);
            self.balance_tree.remove(id);
        }
    }

    /// Converts the `ZkSyncTx` object to a `ZkSyncOp`, without applying it.
    pub fn zksync_tx_to_zksync_op(&self, tx: ZkSyncTx) -> Result<ZkSyncOp, anyhow::Error> {
        match tx {
            ZkSyncTx::Transfer(tx) => self.create_op(*tx).map(TransferOutcome::into_franklin_op),
            ZkSyncTx::Withdraw(tx) => self.create_op(*tx).map(Into::into),
            ZkSyncTx::ChangePubKey(tx) => self.create_op(*tx).map(Into::into),
            ZkSyncTx::Close(_) => anyhow::bail!("Close op is disabled"),
            ZkSyncTx::ForcedExit(tx) => self.create_op(*tx).map(Into::into),
        }
    }

    /// Converts the `PriorityOp` object to a `ZkSyncOp`, without applying it.
    pub fn priority_op_to_zksync_op(&self, op: ZkSyncPriorityOp) -> ZkSyncOp {
        match op {
            ZkSyncPriorityOp::Deposit(op) => self.create_op(op).unwrap().into(),
            ZkSyncPriorityOp::FullExit(op) => self.create_op(op).unwrap().into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn apply_updates(&mut self, updates: &[(u32, AccountUpdate)]) {
        for (account_id, update) in updates {
            match update {
                AccountUpdate::Create { address, nonce } => {
                    let (mut account, _) = Account::create_account(*account_id, *address);
                    account.nonce = *nonce;
                    self.insert_account(*account_id, account);
                }
                AccountUpdate::Delete { address, nonce } => {
                    let account = self
                        .get_account(*account_id)
                        .expect("account doesn't exist");
                    assert_eq!(&account.address, address);
                    assert_eq!(&account.nonce, nonce);
                    self.remove_account(*account_id)
                }
                AccountUpdate::UpdateBalance {
                    old_nonce,
                    new_nonce,
                    balance_update,
                } => {
                    let mut account = self
                        .get_account(*account_id)
                        .expect("account doesn't exist");

                    let (token_id, old_amount, new_amount) = balance_update;

                    assert_eq!(account.nonce, *old_nonce, "nonce mismatch");
                    assert_eq!(
                        &account.get_balance(*token_id),
                        old_amount,
                        "balance mismatch"
                    );
                    account.nonce = *new_nonce;
                    account.set_balance(*token_id, new_amount.clone());

                    self.insert_account(*account_id, account);
                }
                AccountUpdate::ChangePubKeyHash {
                    old_pub_key_hash,
                    new_pub_key_hash,
                    old_nonce,
                    new_nonce,
                } => {
                    let mut account = self
                        .get_account(*account_id)
                        .expect("account doesn't exist");

                    assert_eq!(
                        &account.pub_key_hash, old_pub_key_hash,
                        "pub_key_hash mismatch"
                    );
                    assert_eq!(&account.nonce, old_nonce, "nonce mismatch");

                    account.pub_key_hash = new_pub_key_hash.clone();
                    account.nonce = *new_nonce;

                    self.insert_account(*account_id, account);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_crypto::rand::{Rng, SeedableRng, XorShiftRng};

    #[test]
    fn plasma_state_reversing_updates() {
        let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

        let token_id = 10;

        let mut random_addresses = Vec::new();
        for _ in 0..20 {
            random_addresses.push(Address::from(rng.gen::<[u8; 20]>()));
        }

        // Create two accounts: 0, 1
        // Delete 0, update balance of 1, create account 2
        // Reverse updates

        let initial_plasma_state = ZkSyncState::from_acc_map(AccountMap::default(), 0);

        let updates = {
            let mut updates = AccountUpdates::new();
            updates.push((
                0,
                AccountUpdate::Create {
                    address: random_addresses[0],
                    nonce: 0,
                },
            ));
            updates.push((
                1,
                AccountUpdate::Create {
                    address: random_addresses[1],
                    nonce: 0,
                },
            ));
            updates.push((
                0,
                AccountUpdate::Delete {
                    address: random_addresses[0],
                    nonce: 0,
                },
            ));
            updates.push((
                1,
                AccountUpdate::UpdateBalance {
                    old_nonce: 0,
                    new_nonce: 1,
                    balance_update: (token_id, 0u32.into(), 256u32.into()),
                },
            ));
            updates.push((
                2,
                AccountUpdate::Create {
                    address: random_addresses[2],
                    nonce: 0,
                },
            ));
            updates
        };

        let plasma_state_updated = {
            let mut plasma_state = initial_plasma_state.clone();
            plasma_state.apply_account_updates(updates.clone());
            plasma_state
        };
        assert_eq!(
            plasma_state_updated
                .get_account(1)
                .unwrap()
                .get_balance(token_id),
            256u32.into()
        );

        let plasma_state_updated_back = {
            let mut plasma_state = plasma_state_updated;
            let mut reversed_updates = updates;
            reverse_updates(&mut reversed_updates);
            plasma_state.apply_account_updates(reversed_updates);
            plasma_state
        };
        assert_eq!(
            plasma_state_updated_back.root_hash(),
            initial_plasma_state.root_hash()
        );
    }
}
