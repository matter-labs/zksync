use num::BigUint;
use zksync_types::{
    Account, AccountId, AccountTree, AccountUpdate, Address, BlockNumber, Nonce, TokenId,
};

use crate::state_keeper::state_restore::db::mock::{MockBlock, MockImpl};

#[derive(Debug)]
pub struct StateGenerator {
    pub(super) tree: AccountTree,
    pub(super) mock_db: MockImpl,
    current_block: MockBlock,
    account_id: u32,
}

impl StateGenerator {
    pub(crate) fn new() -> Self {
        Self {
            tree: AccountTree::new(zksync_crypto::params::account_tree_depth()),
            mock_db: MockImpl::new(),
            current_block: MockBlock::default(),
            account_id: 0,
        }
    }

    pub(crate) fn create_db(&self) -> MockImpl {
        self.mock_db.clone()
    }

    pub(crate) fn seal_block(&mut self) {
        let current_block = std::mem::take(&mut self.current_block);
        self.mock_db.add_block(current_block);
    }

    pub(crate) fn save_cache(&mut self, block_number: BlockNumber) {
        let cache = self.tree.get_internals();
        self.mock_db.save_cache(block_number, cache);
    }

    pub(crate) fn create_account(&mut self) -> AccountId {
        let update = AccountUpdate::Create {
            address: Address::repeat_byte(self.account_id as u8),
            nonce: Nonce(0),
        };
        let id = AccountId(self.account_id);
        self.update_account(id, update);
        id
    }

    pub(crate) fn change_account_balance(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        balance: impl Into<BigUint>,
    ) {
        let account = self.tree.get(account_id.0).expect("Non-existing account");
        let update = AccountUpdate::UpdateBalance {
            old_nonce: account.nonce,
            new_nonce: account.nonce + 1,
            balance_update: (token_id, account.get_balance(token_id), balance.into()),
        };
        self.update_account(account_id, update);
    }

    fn update_account(&mut self, account_id: AccountId, update: AccountUpdate) {
        let account = match update.clone() {
            AccountUpdate::Create { address, .. } => {
                let account = Account::default_with_address(&address);
                self.tree.insert(self.account_id, account.clone());
                self.account_id += 1;
                account
            }
            AccountUpdate::UpdateBalance {
                new_nonce,
                balance_update,
                ..
            } => {
                let mut account = self
                    .tree
                    .get(account_id.0)
                    .expect("Non-existing account")
                    .clone();
                account.nonce = new_nonce;
                account.set_balance(balance_update.0, balance_update.2);
                self.tree.insert(account_id.0, account.clone());
                account
            }
            AccountUpdate::ChangePubKeyHash {
                new_pub_key_hash,
                new_nonce,
                ..
            } => {
                let mut account = self
                    .tree
                    .get(account_id.0)
                    .expect("Non-existing account")
                    .clone();
                account.nonce = new_nonce;
                account.pub_key_hash = new_pub_key_hash;
                self.tree.insert(account_id.0, account.clone());
                account
            }
            AccountUpdate::Delete { .. } => unimplemented!("Unsupported operation"),
            AccountUpdate::MintNFT { .. } => unimplemented!("Unsupported operation"),
            AccountUpdate::RemoveNFT { .. } => unimplemented!("Unsupported operation"),
        };

        self.current_block.updates.push((account_id, update));
        self.current_block.accounts.insert(account_id, account);
        self.current_block.hash = self.tree.root_hash();
    }
}
