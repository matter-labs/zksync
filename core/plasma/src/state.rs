use bigdecimal::BigDecimal;
use failure::{bail, ensure, format_err, Error};
use log::trace;
use models::node::operations::{
    CloseOp, DepositOp, FranklinOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp,
};
use models::node::{Account, AccountAddress, AccountTree, FranklinPriorityOp};
use models::node::{
    AccountId, AccountMap, AccountUpdate, AccountUpdates, BlockNumber, Fr, TokenId,
};
use models::node::{Close, Deposit, FranklinTx, FullExit, Transfer, Withdraw};
use models::params;
use std::collections::HashMap;

#[derive(Debug)]
pub struct OpSuccess {
    pub fee: Option<CollectedFee>,
    pub updates: AccountUpdates,
    pub executed_op: FranklinOp,
}

pub struct PlasmaState {
    /// Accounts stored in a sparse Merkle tree
    balance_tree: AccountTree,

    account_id_by_address: HashMap<AccountAddress, AccountId>,

    /// Current block number
    pub block_number: BlockNumber,
}

#[derive(Debug)]
pub struct CollectedFee {
    pub token: TokenId,
    pub amount: BigDecimal,
}

impl PlasmaState {
    pub fn empty() -> Self {
        let tree_depth = params::account_tree_depth() as u32;
        let balance_tree = AccountTree::new(tree_depth);
        Self {
            balance_tree,
            block_number: 0,
            account_id_by_address: HashMap::new(),
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

    pub fn chunks_for_tx(&self, franklin_tx: &FranklinTx) -> usize {
        match franklin_tx {
            FranklinTx::Transfer(tx) => {
                if self.get_account_by_address(&tx.to).is_some() {
                    TransferOp::CHUNKS
                } else {
                    TransferToNewOp::CHUNKS
                }
            }
            FranklinTx::Withdraw(_) => WithdrawOp::CHUNKS,
            FranklinTx::Close(_) => CloseOp::CHUNKS,
        }
    }

    /// Priority op execution should not fail.
    pub fn execute_priority_op(&mut self, op: FranklinPriorityOp) -> OpSuccess {
        match op {
            FranklinPriorityOp::Deposit(op) => self.apply_deposit(op),
            FranklinPriorityOp::FullExit(op) => self.apply_full_exit(op),
        }
    }

    pub fn execute_tx(&mut self, tx: FranklinTx) -> Result<OpSuccess, Error> {
        match tx {
            FranklinTx::Transfer(tx) => self.apply_transfer(tx),
            FranklinTx::Withdraw(tx) => self.apply_withdraw(tx),
            FranklinTx::Close(tx) => self.apply_close(tx),
        }
    }

    fn get_free_account_id(&self) -> AccountId {
        // TODO check for collisions.
        self.balance_tree.items.len() as u32
    }

    fn apply_deposit(&mut self, priority_op: Deposit) -> OpSuccess {
        let account_id =
            if let Some((account_id, _)) = self.get_account_by_address(&priority_op.account) {
                account_id
            } else {
                self.get_free_account_id()
            };
        let deposit_op = DepositOp {
            priority_op,
            account_id,
        };

        let updates = self.apply_deposit_op(&deposit_op);
        OpSuccess {
            fee: None,
            updates,
            executed_op: FranklinOp::Deposit(Box::new(deposit_op)),
        }
    }

    fn apply_full_exit(&mut self, priority_op: FullExit) -> OpSuccess {
        assert!(
            priority_op.token < params::TOTAL_TOKENS as TokenId,
            "Full exit token is out of range, this should be enforced by contract"
        );
        trace!("Processing {:?}", priority_op);
        let account_balance = priority_op
            // Check that operation was signed
            .verify_signature()
            .and_then(|signed_by| {
                trace!("Signature correct, by: {}", signed_by.to_hex());
                // Check if account exists and withdraw was authorized by this account
                self.get_account(priority_op.account_id).filter(|tree_acc| {
                    let withdraw_authorized = tree_acc.address == signed_by;
                    let nonce_correct = tree_acc.nonce == priority_op.nonce;
                    trace!(
                        "authorized: {}, nonce ok: {}",
                        withdraw_authorized,
                        nonce_correct
                    );
                    withdraw_authorized && nonce_correct
                })
            })
            .map(|account| account.get_balance(priority_op.token))
            .filter(|balance| balance != &BigDecimal::from(0));
        assert_ne!(
            account_balance,
            Some(BigDecimal::from(0)),
            "FullExit with zero balance should be failed"
        );
        trace!("Balance: {:?}", account_balance);
        let op = FullExitOp {
            priority_op,
            withdraw_amount: account_balance,
        };

        OpSuccess {
            fee: None,
            updates: self.apply_full_exit_op(&op),
            executed_op: FranklinOp::FullExit(Box::new(op)),
        }
    }

    fn apply_full_exit_op(&mut self, op: &FullExitOp) -> AccountUpdates {
        let mut updates = Vec::new();
        let amount = if let Some(amount) = &op.withdraw_amount {
            amount.clone()
        } else {
            return updates;
        };

        let account_id = op.priority_op.account_id;

        // expect is ok since account since existence was verified before
        let mut account = self
            .get_account(account_id)
            .expect("Full exit account not found");

        let old_balance = account.get_balance(op.priority_op.token);
        let old_nonce = account.nonce;

        // Nonce should be verified before.
        assert_eq!(old_nonce, op.priority_op.nonce, "Full exit nonce mismatch");
        account.sub_balance(op.priority_op.token, &amount);
        account.nonce += 1;
        let new_balance = account.get_balance(op.priority_op.token);
        assert_eq!(
            new_balance,
            BigDecimal::from(0),
            "Full exit amount is incorrect"
        );
        let new_nonce = account.nonce;

        self.insert_account(account_id, account);
        updates.push((
            account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.priority_op.token, old_balance, new_balance),
                old_nonce,
                new_nonce,
            },
        ));

        updates
    }

    fn apply_transfer(&mut self, tx: Transfer) -> Result<OpSuccess, Error> {
        ensure!(
            tx.token < (params::TOTAL_TOKENS as TokenId),
            "Token id is not supported"
        );
        let (from, _) = self
            .get_account_by_address(&tx.from)
            .ok_or_else(|| format_err!("From account does not exist"))?;

        if let Some((to, _)) = self.get_account_by_address(&tx.to) {
            let transfer_op = TransferOp { tx, from, to };

            let (fee, updates) = self.apply_transfer_op(&transfer_op)?;
            Ok(OpSuccess {
                fee: Some(fee),
                updates,
                executed_op: FranklinOp::Transfer(Box::new(transfer_op)),
            })
        } else {
            let to = self.get_free_account_id();
            let transfer_to_new_op = TransferToNewOp { tx, from, to };

            let (fee, updates) = self.apply_transfer_to_new_op(&transfer_to_new_op)?;
            Ok(OpSuccess {
                fee: Some(fee),
                updates,
                executed_op: FranklinOp::TransferToNew(Box::new(transfer_to_new_op)),
            })
        }
    }

    fn apply_withdraw(&mut self, tx: Withdraw) -> Result<OpSuccess, Error> {
        ensure!(
            tx.token < (params::TOTAL_TOKENS as TokenId),
            "Token id is not supported"
        );
        let (account_id, _) = self
            .get_account_by_address(&tx.account)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        let withdraw_op = WithdrawOp { tx, account_id };

        let (fee, updates) = self.apply_withdraw_op(&withdraw_op)?;
        Ok(OpSuccess {
            fee: Some(fee),
            updates,
            executed_op: FranklinOp::Withdraw(Box::new(withdraw_op)),
        })
    }

    fn apply_close(&mut self, _tx: Close) -> Result<OpSuccess, Error> {
        bail!("Account closing is disabled");
        // let (account_id, _) = self
        //     .get_account_by_address(&tx.account)
        //     .ok_or_else(|| format_err!("Account does not exist"))?;
        // let close_op = CloseOp { tx, account_id };

        // let (fee, updates) = self.apply_close_op(&close_op)?;
        // Ok(OpSuccess {
        //     fee: Some(fee),
        //     updates,
        //     executed_op: FranklinOp::Close(Box::new(close_op)),
        // })
    }

    pub fn collect_fee(
        &mut self,
        fees: &[CollectedFee],
        fee_account: &AccountAddress,
    ) -> (AccountId, AccountUpdates) {
        let mut updates = Vec::new();

        let (id, mut account) =
            if let Some((id, account)) = self.get_account_by_address(fee_account) {
                (id, account)
            } else {
                panic!(
                    "Fee account should be present in the account tree: {}",
                    fee_account.to_hex()
                );
            };

        for fee in fees {
            if fee.amount == BigDecimal::from(0) {
                continue;
            }

            let old_amount = account.get_balance(fee.token).clone();
            let nonce = account.nonce;
            account.add_balance(fee.token, &fee.amount);
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

    pub fn get_account_by_address(&self, address: &AccountAddress) -> Option<(AccountId, Account)> {
        let account_id = *self.account_id_by_address.get(address)?;
        Some((
            account_id,
            self.get_account(account_id)
                .expect("Failed to get account by cached pubkey"),
        ))
    }

    fn insert_account(&mut self, id: AccountId, account: Account) {
        self.account_id_by_address
            .insert(account.address.clone(), id);
        self.balance_tree.insert(id, account);
    }

    #[allow(dead_code)]
    fn remove_account(&mut self, id: AccountId) {
        if let Some(account) = self.get_account(id) {
            self.account_id_by_address.remove(&account.address);
            self.balance_tree.delete(id);
        }
    }

    fn apply_deposit_op(&mut self, op: &DepositOp) -> AccountUpdates {
        let mut updates = Vec::new();

        let mut account = self.get_account(op.account_id).unwrap_or_else(|| {
            let (account, upd) =
                Account::create_account(op.account_id, op.priority_op.account.clone());
            updates.extend(upd.into_iter());
            account
        });

        let old_amount = account.get_balance(op.priority_op.token).clone();
        let old_nonce = account.nonce;
        account.add_balance(op.priority_op.token, &op.priority_op.amount);
        let new_amount = account.get_balance(op.priority_op.token).clone();

        self.insert_account(op.account_id, account);

        updates.push((
            op.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.priority_op.token, old_amount, new_amount),
                old_nonce,
                new_nonce: old_nonce,
            },
        ));

        updates
    }

    fn apply_transfer_to_new_op(
        &mut self,
        op: &TransferToNewOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        let mut updates = Vec::new();

        assert!(
            self.get_account(op.to).is_none(),
            "Transfer to new account exists"
        );
        let mut to_account = {
            let (acc, upd) = Account::create_account(op.to, op.tx.to.clone());
            updates.extend(upd.into_iter());
            acc
        };

        let mut from_account = self.get_account(op.from).unwrap();
        let from_old_balance = from_account.get_balance(op.tx.token).clone();
        let from_old_nonce = from_account.nonce;
        ensure!(op.tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            "Not enough balance"
        );
        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        from_account.nonce += 1;
        let from_new_balance = from_account.get_balance(op.tx.token).clone();
        let from_new_nonce = from_account.nonce;

        let to_old_balance = to_account.get_balance(op.tx.token).clone();
        let to_account_nonce = to_account.nonce;
        to_account.add_balance(op.tx.token, &op.tx.amount);
        let to_new_balance = to_account.get_balance(op.tx.token).clone();

        self.insert_account(op.from, from_account);
        self.insert_account(op.to, to_account);

        updates.push((
            op.from,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, from_old_balance, from_new_balance),
                old_nonce: from_old_nonce,
                new_nonce: from_new_nonce,
            },
        ));
        updates.push((
            op.to,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, to_old_balance, to_new_balance),
                old_nonce: to_account_nonce,
                new_nonce: to_account_nonce,
            },
        ));

        let fee = CollectedFee {
            token: op.tx.token,
            amount: op.tx.fee.clone(),
        };

        Ok((fee, updates))
    }

    fn apply_withdraw_op(
        &mut self,
        op: &WithdrawOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        let mut updates = Vec::new();
        let mut from_account = self.get_account(op.account_id).unwrap();

        let from_old_balance = from_account.get_balance(op.tx.token).clone();
        let from_old_nonce = from_account.nonce;

        ensure!(op.tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            "Not enough balance"
        );

        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        from_account.nonce += 1;

        let from_new_balance = from_account.get_balance(op.tx.token).clone();
        let from_new_nonce = from_account.nonce;

        self.insert_account(op.account_id, from_account);

        updates.push((
            op.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, from_old_balance, from_new_balance),
                old_nonce: from_old_nonce,
                new_nonce: from_new_nonce,
            },
        ));

        let fee = CollectedFee {
            token: op.tx.token,
            amount: op.tx.fee.clone(),
        };

        Ok((fee, updates))
    }

    #[allow(dead_code)]
    fn apply_close_op(&mut self, op: &CloseOp) -> Result<(CollectedFee, AccountUpdates), Error> {
        let mut updates = Vec::new();
        let account = self.get_account(op.account_id).unwrap();

        for token in 0..params::TOTAL_TOKENS {
            if account.get_balance(token as TokenId) != BigDecimal::from(0) {
                bail!("Account is not empty, token id: {}", token);
            }
        }

        ensure!(op.tx.nonce == account.nonce, "Nonce mismatch");

        self.remove_account(op.account_id);

        updates.push((
            op.account_id,
            AccountUpdate::Delete {
                address: account.address,
                nonce: account.nonce,
            },
        ));

        let fee = CollectedFee {
            token: params::ETH_TOKEN_ID,
            amount: BigDecimal::from(0),
        };

        Ok((fee, updates))
    }

    fn apply_transfer_op(
        &mut self,
        op: &TransferOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        let mut updates = Vec::new();

        let mut from_account = self.get_account(op.from).unwrap();
        let mut to_account = self.get_account(op.to).unwrap();

        let from_old_balance = from_account.get_balance(op.tx.token).clone();
        let from_old_nonce = from_account.nonce;

        ensure!(op.tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            "Not enough balance"
        );

        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        from_account.nonce += 1;

        let from_new_balance = from_account.get_balance(op.tx.token).clone();
        let from_new_nonce = from_account.nonce;

        let to_old_balance = to_account.get_balance(op.tx.token).clone();
        let to_account_nonce = to_account.nonce;

        to_account.add_balance(op.tx.token, &op.tx.amount);

        let to_new_balance = to_account.get_balance(op.tx.token).clone();

        self.insert_account(op.from, from_account);
        self.insert_account(op.to, to_account);

        updates.push((
            op.from,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, from_old_balance, from_new_balance),
                old_nonce: from_old_nonce,
                new_nonce: from_new_nonce,
            },
        ));

        updates.push((
            op.to,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, to_old_balance, to_new_balance),
                old_nonce: to_account_nonce,
                new_nonce: to_account_nonce,
            },
        ));

        let fee = CollectedFee {
            token: op.tx.token,
            amount: op.tx.fee.clone(),
        };

        Ok((fee, updates))
    }
}
