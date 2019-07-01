use bigdecimal::{BigDecimal, Zero};
use merkle_tree::AccountTree;
use models::plasma::account::{Account, ETH_TOKEN_ID};
use models::plasma::tx::{DepositTx, ExitTx, TransferTx};
use models::plasma::{params, AccountUpdate};
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

    // TODO (Drogan) Maybe simplify signature checking?
    pub fn apply_transfer(
        &mut self,
        tx: &TransferTx,
    ) -> Result<(BigDecimal, Vec<AccountUpdate>), TransferApplicationError> {
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
                //debug!("Nonce is too high");
                return Err(TransferApplicationError::NonceIsTooHigh);
            } else if tx.nonce < from.nonce {
                //debug!("Nonce is too low");
                return Err(TransferApplicationError::NonceIsTooLow);
            }

            if *from.get_balance(ETH_TOKEN_ID) < transacted_amount {
                //debug!("Insufficient balance");
                return Err(TransferApplicationError::InsufficientBalance);
            }

            if tx.good_until_block < self.block_number {
                //debug!("Transaction is outdated");
                return Err(TransferApplicationError::ExpiredTransaction);
            }

            // update state

            // allow to send to non-existing accounts
            // let mut to = self.balance_tree.items.get(&tx.to).ok_or(())?.clone();

            let from_account_update = {
                let from_old_balance = from.get_balance(ETH_TOKEN_ID).clone();
                let old_nonce = from.nonce;
                from.sub_balance(ETH_TOKEN_ID, &transacted_amount);
                let from_new_balance = from.get_balance(ETH_TOKEN_ID).clone();
                from.nonce += 1;

                self.balance_tree.insert(tx.from, from);

                AccountUpdate::UpdateBalance {
                    id: tx.from,
                    balance_update: (ETH_TOKEN_ID, from_old_balance, from_new_balance),
                    nonce: old_nonce,
                }
            };

            let to_account_updates = {
                let mut to_account_updates = Vec::new();
                let mut to = self.balance_tree.items.remove(&tx.to).unwrap_or_else(|| {
                    let new_acc = Account::default();

                    let create_acc_update = AccountUpdate::Create {
                        id: tx.to,
                        public_key_x: new_acc.public_key_x.clone(),
                        public_key_y: new_acc.public_key_y.clone(),
                        nonce: new_acc.nonce,
                    };
                    to_account_updates.push(create_acc_update);

                    new_acc
                });

                // TODO (Drogan) Why? Document somewhere.
                if tx.to != 0 {
                    let to_old_balance = to.get_balance(ETH_TOKEN_ID).clone();
                    to.add_balance(ETH_TOKEN_ID, &tx.amount);
                    let to_new_balance = to.get_balance(ETH_TOKEN_ID).clone();

                    let balance_update = AccountUpdate::UpdateBalance {
                        id: tx.to,
                        balance_update: (ETH_TOKEN_ID, to_old_balance, to_new_balance),
                        nonce: to.nonce,
                    };
                    to_account_updates.push(balance_update);
                }

                self.balance_tree.insert(tx.to, to);
                to_account_updates
            };

            let collected_fee = tx.fee.clone();

            let mut account_updates = vec![from_account_update];
            account_updates.extend(to_account_updates.into_iter());

            return Ok((collected_fee, account_updates));
        }

        Err(TransferApplicationError::InvalidSigner)
    }

    pub fn apply_deposit(&mut self, tx: &DepositTx) -> Result<Vec<AccountUpdate>, ()> {
        let mut updates = Vec::new();

        let mut acc = self
            .balance_tree
            .items
            .remove(&tx.account)
            .unwrap_or_else(|| {
                let mut acc = Account::default();
                acc.public_key_x = tx.pub_x.clone();
                acc.public_key_y = tx.pub_y.clone();

                updates.push(AccountUpdate::Create {
                    id: tx.account,
                    public_key_x: acc.public_key_x.clone(),
                    public_key_y: acc.public_key_y.clone(),
                    nonce: acc.nonce,
                });

                acc
            });

        let old_amount = acc.get_balance(ETH_TOKEN_ID).clone();
        let old_nonce = acc.nonce;
        acc.add_balance(ETH_TOKEN_ID, &tx.amount);
        let new_amount = acc.get_balance(ETH_TOKEN_ID).clone();

        self.balance_tree.insert(tx.account, acc);

        updates.push(AccountUpdate::UpdateBalance {
            id: tx.account,
            balance_update: (ETH_TOKEN_ID, old_amount, new_amount),
            nonce: old_nonce,
        });

        Ok(updates)
    }

    pub fn apply_exit(&mut self, tx: &mut ExitTx) -> Result<Vec<AccountUpdate>, ()> {
        let acc = self.balance_tree.items.remove(&tx.account).ok_or(())?;

        debug!(
            "Adding account balance to ExitTx, value = {}",
            acc.get_balance(ETH_TOKEN_ID)
        );

        let old_amount = acc.get_balance(ETH_TOKEN_ID).clone();
        tx.amount = old_amount.clone();

        let mut updates = Vec::new();
        updates.push(AccountUpdate::UpdateBalance {
            id: tx.account,
            balance_update: (ETH_TOKEN_ID, old_amount, BigDecimal::zero()),
            nonce: acc.nonce,
        });
        updates.push(AccountUpdate::Delete {
            id: tx.account,
            public_key_x: acc.public_key_x,
            public_key_y: acc.public_key_y,
            nonce: acc.nonce,
        });
        Ok(updates)
    }
}
