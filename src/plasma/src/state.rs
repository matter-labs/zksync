use bigdecimal::{BigDecimal, Zero};
use merkle_tree::AccountTree;
use models::plasma::account::Account;
use models::plasma::params;
use models::plasma::tx::{DepositTx, ExitTx, TransferTx};
use models::plasma::{AccountId, AccountMap, Fr, TransferApplicationError};

pub struct PlasmaState {
    /// Accounts stored in a sparse Merkle tree
    pub balance_tree: AccountTree,

    /// Current block number
    pub block_number: u32,
}

impl PlasmaState {
    pub fn empty() -> Self {
        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let balance_tree = AccountTree::new(tree_depth);
        Self {
            balance_tree,
            block_number: 0,
        }
    }

    pub fn new(accounts: AccountMap, current_block: u32) -> Self {
        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let mut balance_tree = AccountTree::new(tree_depth);
        for (id, account) in accounts {
            balance_tree.insert(id, account);
        }
        Self {
            balance_tree,
            block_number: current_block,
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
        self.balance_tree.items.get(&account_id).cloned()
    }

    pub fn apply_transfer(
        &mut self,
        tx: &TransferTx,
    ) -> Result<BigDecimal, TransferApplicationError> {
        if let Some(mut from) = self.balance_tree.items.get(&tx.from).cloned() {
            // TODO: take from `from` instead and uncomment below
            let pub_key = self
                .get_account(tx.from)
                .and_then(|a| a.get_pub_key())
                .ok_or(TransferApplicationError::UnknownSigner)?;
            if let Some(verified_against) = tx.cached_pub_key.as_ref() {
                if pub_key.0 != verified_against.0 {
                    return Err(TransferApplicationError::InvalidSigner);
                }
            } else {
                return Err(TransferApplicationError::InvalidSigner);
            }

            let mut transacted_amount = BigDecimal::zero();
            transacted_amount += &tx.amount;
            transacted_amount += &tx.fee;

            if tx.nonce > from.nonce {
                //println!("Nonce is too high");
                return Err(TransferApplicationError::NonceIsTooHigh);
            } else if tx.nonce < from.nonce {
                //println!("Nonce is too low");
                return Err(TransferApplicationError::NonceIsTooLow);
            }

            if from.balance < transacted_amount {
                //println!("Insufficient balance");
                return Err(TransferApplicationError::InsufficientBalance);
            }

            if tx.good_until_block < self.block_number {
                //println!("Transaction is outdated");
                return Err(TransferApplicationError::ExpiredTransaction);
            }

            // update state

            // allow to send to non-existing accounts
            // let mut to = self.balance_tree.items.get(&tx.to).ok_or(())?.clone();

            let mut to = Account::default();
            if let Some(existing_to) = self.balance_tree.items.get(&tx.to) {
                to = existing_to.clone();
            }

            from.balance -= transacted_amount;

            from.nonce += 1;
            if tx.to != 0 {
                to.balance += &tx.amount;
            }

            self.balance_tree.insert(tx.from, from);
            self.balance_tree.insert(tx.to, to);

            let collected_fee = tx.fee.clone();

            return Ok(collected_fee);
        }

        Err(TransferApplicationError::InvalidSigner)
    }

    pub fn apply_deposit(&mut self, tx: &DepositTx) -> Result<(), ()> {
        let existing_acc = self.balance_tree.items.get(&tx.account);

        if existing_acc.is_none() {
            let mut acc = Account::default();
            let tx = tx.clone();
            acc.public_key_x = tx.pub_x;
            acc.public_key_y = tx.pub_y;
            acc.balance = tx.amount;
            self.balance_tree.insert(tx.account, acc);
        } else {
            let mut acc = existing_acc.unwrap().clone();
            acc.balance += &tx.amount;
            self.balance_tree.insert(tx.account, acc);
        }
        Ok(())
    }

    pub fn apply_exit(&mut self, tx: &ExitTx) -> Result<ExitTx, ()> {
        let acc = self.balance_tree.items.get(&tx.account).ok_or(())?.clone();

        let mut agumented_tx = tx.clone();

        println!("Adding account balance to ExitTx, value = {}", acc.balance);

        agumented_tx.amount = acc.balance;

        self.balance_tree.delete(tx.account);

        Ok(agumented_tx)
    }
}
