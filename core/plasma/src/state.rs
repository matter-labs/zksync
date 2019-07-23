use bigdecimal::BigDecimal;
use failure::{bail, ensure, format_err, Error};
use ff::PrimeField;
use merkle_tree::AccountTree;
use models::plasma::account::Account;
use models::plasma::operations::{CloseOp, DepositOp, FranklinOp, PartialExitOp, TransferOp};
use models::plasma::tx::{Close, Deposit, FranklinTx, Transfer, Withdraw};
use models::plasma::{
    params, AccountUpdate, AccountUpdates, BlockNumber, FeeAmount, TokenAmount, TokenId,
    TransferToNewOp,
};
use models::plasma::{AccountId, AccountMap, Fr};
use std::collections::HashMap;

#[derive(Hash, PartialEq, Eq)]
struct PubkeyBytes {
    bytes: Vec<u64>,
}

#[derive(Debug)]
pub struct TxSuccess {
    pub fee: CollectedFee,
    pub updates: AccountUpdates,
    pub executed_op: FranklinOp,
}

impl From<(Fr, Fr)> for PubkeyBytes {
    fn from((pk_x, pk_y): (Fr, Fr)) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(pk_x.into_repr().as_ref());
        bytes.extend_from_slice(pk_y.into_repr().as_ref());
        Self { bytes }
    }
}

pub struct PlasmaState {
    /// Accounts stored in a sparse Merkle tree
    balance_tree: AccountTree,

    account_id_by_pubkey: HashMap<PubkeyBytes, AccountId>,

    /// Current block number
    pub block_number: BlockNumber,
}

#[derive(Debug)]
pub struct CollectedFee {
    pub token: TokenId,
    pub amount: FeeAmount,
}

impl PlasmaState {
    pub fn empty() -> Self {
        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let balance_tree = AccountTree::new(tree_depth);
        Self {
            balance_tree,
            block_number: 0,
            account_id_by_pubkey: HashMap::new(),
        }
    }

    pub fn new(accounts: AccountMap, current_block: u32) -> Self {
        let mut empty = Self::empty();
        empty.block_number = current_block;
        for (id, account) in accounts {
            empty.insert_account(id, account);
        }
        empty
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

    pub fn apply_tx(&mut self, tx: FranklinTx) -> Result<TxSuccess, Error> {
        match tx {
            FranklinTx::Transfer(tx) => self.apply_transfer(tx),
            FranklinTx::Deposit(tx) => self.apply_deposit(tx),
            FranklinTx::Withdraw(tx) => self.apply_withdraw(tx),
            FranklinTx::Close(tx) => self.apply_close(tx),
        }
    }

    fn get_free_account_id(&self) -> AccountId {
        // TODO check for collisions.
        self.balance_tree.items.len() as u32
    }

    fn apply_deposit(&mut self, tx: Deposit) -> Result<TxSuccess, Error> {
        let account_id =
            if let Some((account_id, _)) = self.get_account_by_pubkey(tx.to.pk_x, tx.to.pk_y) {
                account_id
            } else {
                self.get_free_account_id()
            };
        let deposit_op = DepositOp { tx, account_id };

        let (fee, updates) = self.apply_deposit_op(&deposit_op)?;
        Ok(TxSuccess {
            fee,
            updates,
            executed_op: FranklinOp::Deposit(deposit_op),
        })
    }

    fn apply_transfer(&mut self, tx: Transfer) -> Result<TxSuccess, Error> {
        let (from, _) = self
            .get_account_by_pubkey(tx.from.pk_x, tx.from.pk_y)
            .ok_or_else(|| format_err!("From account does not exist"))?;

        if let Some((to, _)) = self.get_account_by_pubkey(tx.to.pk_x, tx.to.pk_y) {
            let transfer_op = TransferOp { tx, from, to };

            let (fee, updates) = self.apply_transfer_op(&transfer_op)?;
            Ok(TxSuccess {
                fee,
                updates,
                executed_op: FranklinOp::Transfer(transfer_op),
            })
        } else {
            let to = self.get_free_account_id();
            let transfer_to_new_op = TransferToNewOp { tx, from, to };

            let (fee, updates) = self.apply_transfer_to_new_op(&transfer_to_new_op)?;
            Ok(TxSuccess {
                fee,
                updates,
                executed_op: FranklinOp::TransferToNew(transfer_to_new_op),
            })
        }
    }

    fn apply_withdraw(&mut self, tx: Withdraw) -> Result<TxSuccess, Error> {
        let (account_id, _) = self
            .get_account_by_pubkey(tx.account.pk_x, tx.account.pk_y)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        let partial_exit_op = PartialExitOp { tx, account_id };

        let (fee, updates) = self.apply_partial_exit_op(&partial_exit_op)?;
        Ok(TxSuccess {
            fee,
            updates,
            executed_op: FranklinOp::PartialExit(partial_exit_op),
        })
    }

    fn apply_close(&mut self, tx: Close) -> Result<TxSuccess, Error> {
        let (account_id, _) = self
            .get_account_by_pubkey(tx.account.pk_x, tx.account.pk_y)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        let close_op = CloseOp { tx, account_id };

        let (fee, updates) = self.apply_close_op(&close_op)?;
        Ok(TxSuccess {
            fee,
            updates,
            executed_op: FranklinOp::Close(close_op),
        })
    }

    pub fn collect_fee(
        &mut self,
        fees: &[CollectedFee],
        fee_account: &(Fr, Fr),
    ) -> (AccountId, AccountUpdates) {
        let mut updates = Vec::new();
        let (id, mut account, create_upd) =
            self.get_or_create_account_by_pubkey(fee_account.0, fee_account.1);
        updates.extend(create_upd.into_iter());

        for fee in fees {
            let old_amount = account.get_balance(fee.token).clone();
            let nonce = account.nonce;
            account.add_balance(fee.token, u32::from(fee.amount));
            let new_amount = account.get_balance(fee.token).clone();

            updates.push((
                id,
                AccountUpdate::UpdateBalance {
                    balance_update: (fee.token, old_amount, new_amount),
                    old_nonce: nonce,
                    new_nonce: nonce,
                },
            ));
        }

        self.insert_account(id, account);

        (id, updates)
    }

    fn get_or_create_account_by_id(
        &self,
        id: AccountId,
        pub_key_x: Fr,
        pub_key_y: Fr,
    ) -> (Account, AccountUpdates) {
        let mut updates = Vec::new();
        let account = self.get_account(id).unwrap_or_else(|| {
            let mut acc = Account::default();
            acc.public_key_x = pub_key_x;
            acc.public_key_y = pub_key_y;
            acc.nonce = 0;

            updates.push((
                id,
                AccountUpdate::Create {
                    public_key_x: acc.public_key_x,
                    public_key_y: acc.public_key_y,
                    nonce: acc.nonce,
                },
            ));
            acc
        });
        (account, updates)
    }

    fn get_or_create_account_by_pubkey(
        &self,
        pub_key_x: Fr,
        pub_key_y: Fr,
    ) -> (AccountId, Account, AccountUpdates) {
        let mut updates = Vec::new();
        let (id, account) = self
            .get_account_by_pubkey(pub_key_x, pub_key_y)
            .unwrap_or_else(|| {
                let mut acc = Account::default();
                acc.public_key_x = pub_key_x;
                acc.public_key_y = pub_key_y;
                acc.nonce = 0;

                let acc_id = self.total_accounts();

                updates.push((
                    acc_id,
                    AccountUpdate::Create {
                        public_key_x: acc.public_key_x,
                        public_key_y: acc.public_key_y,
                        nonce: acc.nonce,
                    },
                ));
                (acc_id, acc)
            });
        (id, account, updates)
    }

    fn get_account_by_pubkey(&self, pub_key_x: Fr, pub_key_y: Fr) -> Option<(AccountId, Account)> {
        let account_id = *self
            .account_id_by_pubkey
            .get(&(pub_key_x, pub_key_y).into())?;
        Some((
            account_id,
            self.get_account(account_id)
                .expect("Failed to get account by cached pubkey"),
        ))
    }

    fn insert_account(&mut self, id: AccountId, account: Account) {
        self.account_id_by_pubkey
            .insert((account.public_key_x, account.public_key_y).into(), id);
        self.balance_tree.insert(id, account);
    }

    fn remove_account(&mut self, id: AccountId) {
        if let Some(account) = self.get_account(id) {
            self.account_id_by_pubkey
                .remove(&(account.public_key_x, account.public_key_y).into());
            self.balance_tree.delete(id);
        }
    }

    fn total_accounts(&self) -> u32 {
        self.account_id_by_pubkey.len() as u32
    }

    fn apply_deposit_op(
        &mut self,
        op: &DepositOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        let tx = &op.tx;
        let mut updates = Vec::new();
        let (mut acc, create_upd) =
            self.get_or_create_account_by_id(op.account_id, tx.to.pk_x, tx.to.pk_y);
        updates.extend(create_upd.into_iter());

        let old_amount = acc.get_balance(tx.token).clone();
        let old_nonce = acc.nonce;
        ensure!(
            tx.nonce == old_nonce,
            "Nonce mismatch tx: {}, account: {}",
            tx.nonce,
            old_nonce
        );
        // TODO: (Drogan) check eth state balance.
        acc.add_balance(tx.token, tx.amount);
        acc.nonce += 1;
        let new_amount = acc.get_balance(tx.token).clone();
        let new_nonce = acc.nonce;

        self.insert_account(op.account_id, acc);

        updates.push((
            op.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (tx.token, old_amount, new_amount),
                old_nonce,
                new_nonce,
            },
        ));

        let fee = CollectedFee {
            token: tx.token,
            amount: tx.fee,
        };

        Ok((fee, updates))
    }

    fn apply_transfer_to_new_op(
        &mut self,
        op: &TransferToNewOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        let tx = &op.tx;
        if self.get_account_by_pubkey(tx.to.pk_x, tx.to.pk_y).is_some() {
            bail!("Transfer to new account exists");
        }

        let mut updates = Vec::new();

        let (mut to_account, create_upd) =
            self.get_or_create_account_by_id(op.to, tx.to.pk_x, tx.to.pk_y);
        updates.extend(create_upd.into_iter());

        let mut from_account = self.get_account(op.from).unwrap();
        let from_old_balance = from_account.get_balance(tx.token).clone();
        let from_old_nonce = from_account.nonce;
        ensure!(tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance >= tx.amount + u32::from(tx.fee),
            "Not enough balance"
        );
        from_account.sub_balance(tx.token, tx.amount + u32::from(tx.fee));
        from_account.nonce += 1;
        let from_new_balance = from_account.get_balance(tx.token).clone();
        let from_new_nonce = from_account.nonce;

        let to_old_balance = to_account.get_balance(tx.token).clone();
        let to_account_nonce = to_account.nonce;
        to_account.add_balance(tx.token, tx.amount);
        let to_new_balance = to_account.get_balance(tx.token).clone();

        self.insert_account(op.from, from_account);
        self.insert_account(op.to, to_account);

        updates.push((
            op.from,
            AccountUpdate::UpdateBalance {
                balance_update: (tx.token, from_old_balance, from_new_balance),
                old_nonce: from_old_nonce,
                new_nonce: from_new_nonce,
            },
        ));
        updates.push((
            op.to,
            AccountUpdate::UpdateBalance {
                balance_update: (tx.token, to_old_balance, to_new_balance),
                old_nonce: to_account_nonce,
                new_nonce: to_account_nonce,
            },
        ));

        let fee = CollectedFee {
            token: tx.token,
            amount: tx.fee,
        };

        Ok((fee, updates))
    }

    fn apply_partial_exit_op(
        &mut self,
        op: &PartialExitOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        let tx = &op.tx;

        let mut updates = Vec::new();
        let mut from_account = self
            .get_account(op.account_id)
            .ok_or_else(|| format_err!("From account does not exist id: {}", op.account_id))?;
        let from_old_balance = from_account.get_balance(tx.token).clone();
        let from_old_nonce = from_account.nonce;

        ensure!(tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance >= tx.amount + u32::from(tx.fee),
            "Not enough balance"
        );

        from_account.sub_balance(tx.token, tx.amount + u32::from(tx.fee));
        from_account.nonce += 1;

        let from_new_balance = from_account.get_balance(tx.token).clone();
        let from_new_nonce = from_account.nonce;

        self.insert_account(op.account_id, from_account);

        updates.push((
            op.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (tx.token, from_old_balance, from_new_balance),
                old_nonce: from_old_nonce,
                new_nonce: from_new_nonce,
            },
        ));

        let fee = CollectedFee {
            token: tx.token,
            amount: tx.fee,
        };

        Ok((fee, updates))
    }

    fn apply_close_op(&mut self, op: &CloseOp) -> Result<(CollectedFee, AccountUpdates), Error> {
        let mut updates = Vec::new();
        let account = self
            .get_account(op.account_id)
            .ok_or_else(|| format_err!("Account does not exist id: {}", op.account_id))?;

        for token in 0..params::TOTAL_TOKENS {
            if account.get_balance(token as TokenId) != 0 {
                bail!("Account is not empty, token id: {}", token);
            }
        }

        ensure!(op.tx.nonce == account.nonce, "Nonce mismatch");

        self.remove_account(op.account_id);

        updates.push((
            op.account_id,
            AccountUpdate::Delete {
                public_key_x: account.public_key_x,
                public_key_y: account.public_key_y,
                nonce: account.nonce,
            },
        ));

        let fee = CollectedFee {
            token: params::ETH_TOKEN_ID,
            amount: 0,
        };

        Ok((fee, updates))
    }

    fn apply_transfer_op(
        &mut self,
        op: &TransferOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        let mut updates = Vec::new();
        let tx = &op.tx;
        let mut from_account = self.get_account(op.from).unwrap();
        let mut to_account = self.get_account(op.to).unwrap();

        let from_old_balance = from_account.get_balance(tx.token).clone();
        let from_old_nonce = from_account.nonce;

        ensure!(tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance >= tx.amount + u32::from(tx.fee),
            "Not enough balance"
        );

        from_account.sub_balance(tx.token, tx.amount + u32::from(tx.fee));
        from_account.nonce += 1;

        let from_new_balance = from_account.get_balance(tx.token).clone();
        let from_new_nonce = from_account.nonce;

        let to_old_balance = to_account.get_balance(tx.token).clone();
        let to_account_nonce = to_account.nonce;

        to_account.add_balance(tx.token, tx.amount);

        let to_new_balance = to_account.get_balance(tx.token).clone();

        self.insert_account(op.from, from_account);
        self.insert_account(op.to, to_account);

        updates.push((
            op.from,
            AccountUpdate::UpdateBalance {
                balance_update: (tx.token, from_old_balance, from_new_balance),
                old_nonce: from_old_nonce,
                new_nonce: from_new_nonce,
            },
        ));

        updates.push((
            op.to,
            AccountUpdate::UpdateBalance {
                balance_update: (tx.token, to_old_balance, to_new_balance),
                old_nonce: to_account_nonce,
                new_nonce: to_account_nonce,
            },
        ));

        let fee = CollectedFee {
            token: tx.token,
            amount: tx.fee,
        };

        Ok((fee, updates))
    }
}
