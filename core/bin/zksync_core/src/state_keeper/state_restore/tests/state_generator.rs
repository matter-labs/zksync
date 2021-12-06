use zksync_types::{Account, AccountId, AccountTree, AccountUpdate};

use crate::state_keeper::state_restore::db::mock::{MockBlock, MockImpl};

#[derive(Debug)]
pub struct StateGenerator {
    tree: AccountTree,
    mock_db: MockImpl,
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
        let mut current_block = std::mem::take(&mut self.current_block);
        self.mock_db.add_block(current_block);
    }

    pub(crate) fn update_account(&mut self, account_id: AccountId, update: AccountUpdate) {
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
