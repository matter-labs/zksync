use failure::{bail, ensure, format_err, Error};
use log::trace;
use models::node::operations::{
    ChangePubKeyOp, CloseOp, DepositOp, FranklinOp, FullExitOp, TransferFromOp, TransferOp,
    TransferToNewOp, WithdrawOp,
};
use models::node::tx::ChangePubKey;
use models::node::{Account, AccountTree, FranklinPriorityOp, PubKeyHash};
use models::node::{
    AccountId, AccountMap, AccountUpdate, AccountUpdates, BlockNumber, Fr, TokenId,
};
use models::node::{Address, TransferFrom};
use models::node::{Close, Deposit, FranklinTx, FullExit, Transfer, Withdraw};
use models::params;
use models::params::max_account_id;
use models::primitives::BigUintSerdeWrapper;
use num::BigUint;
use std::collections::HashMap;

#[derive(Debug)]
pub struct OpSuccess {
    pub fee: Option<CollectedFee>,
    pub updates: AccountUpdates,
    pub executed_op: FranklinOp,
}

#[derive(Debug, Clone)]
pub struct PlasmaState {
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
enum TransferOutcome {
    Transfer(TransferOp),
    TransferToNew(TransferToNewOp),
}

impl TransferOutcome {
    pub fn into_franklin_op(self) -> FranklinOp {
        match self {
            Self::Transfer(transfer) => transfer.into(),
            Self::TransferToNew(transfer) => transfer.into(),
        }
    }
}

impl PlasmaState {
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

    pub fn chunks_for_tx(&self, franklin_tx: &FranklinTx) -> usize {
        match franklin_tx {
            FranklinTx::Transfer(tx) => {
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
    pub fn execute_priority_op(&mut self, op: FranklinPriorityOp) -> OpSuccess {
        match op {
            FranklinPriorityOp::Deposit(op) => self.apply_deposit(op),
            FranklinPriorityOp::FullExit(op) => self.apply_full_exit(op),
        }
    }

    pub fn execute_tx(&mut self, tx: FranklinTx) -> Result<OpSuccess, Error> {
        match tx {
            FranklinTx::Transfer(tx) => self.apply_transfer(*tx),
            FranklinTx::TransferFrom(tx) => self.apply_transfer_from(*tx),
            FranklinTx::Withdraw(tx) => self.apply_withdraw(*tx),
            FranklinTx::Close(tx) => self.apply_close(*tx),
            FranklinTx::ChangePubKey(tx) => self.apply_change_pubkey(*tx),
        }
    }

    fn get_free_account_id(&self) -> AccountId {
        // TODO check for collisions.
        self.balance_tree.items.len() as u32
    }

    fn create_deposit_op(&self, priority_op: Deposit) -> DepositOp {
        assert!(
            priority_op.token <= params::max_token_id(),
            "Deposit token is out of range, this should be enforced by contract"
        );
        let account_id = if let Some((account_id, _)) = self.get_account_by_address(&priority_op.to)
        {
            account_id
        } else {
            self.get_free_account_id()
        };
        DepositOp {
            priority_op,
            account_id,
        }
    }

    fn apply_deposit(&mut self, priority_op: Deposit) -> OpSuccess {
        let deposit_op = self.create_deposit_op(priority_op);

        let updates = self.apply_deposit_op(&deposit_op);
        OpSuccess {
            fee: None,
            updates,
            executed_op: FranklinOp::Deposit(Box::new(deposit_op)),
        }
    }

    fn create_full_exit_op(&self, priority_op: FullExit) -> FullExitOp {
        // NOTE: Authroization of the FullExit is verified on the contract.
        assert!(
            priority_op.token <= params::max_token_id(),
            "Full exit token is out of range, this should be enforced by contract"
        );
        trace!("Processing {:?}", priority_op);
        let account_balance = self
            .get_account(priority_op.account_id)
            .filter(|account| account.address == priority_op.eth_address)
            .map(|acccount| acccount.get_balance(priority_op.token))
            .map(BigUintSerdeWrapper);

        trace!("Balance: {:?}", account_balance);
        FullExitOp {
            priority_op,
            withdraw_amount: account_balance,
        }
    }

    fn apply_full_exit(&mut self, priority_op: FullExit) -> OpSuccess {
        let op = self.create_full_exit_op(priority_op);

        OpSuccess {
            fee: None,
            updates: self.apply_full_exit_op(&op),
            executed_op: FranklinOp::FullExit(Box::new(op)),
        }
    }

    pub fn apply_full_exit_op(&mut self, op: &FullExitOp) -> AccountUpdates {
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

        account.sub_balance(op.priority_op.token, &amount.0);

        let new_balance = account.get_balance(op.priority_op.token);
        assert_eq!(
            new_balance,
            BigUint::from(0u32),
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

    fn create_transfer_op(&self, tx: Transfer) -> Result<TransferOutcome, Error> {
        ensure!(
            tx.token <= params::max_token_id(),
            "Token id is not supported"
        );
        ensure!(
            tx.to != Address::zero(),
            "Transfer to Account with address 0 is not allowed"
        );
        let (from, from_account) = self
            .get_account_by_address(&tx.from)
            .ok_or_else(|| format_err!("From account does not exist"))?;
        ensure!(
            from_account.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            tx.verify_signature() == Some(from_account.pub_key_hash),
            "Transfer signature is incorrect"
        );
        ensure!(from == tx.account_id, "Transfer account id is incorrect");

        let outcome = if let Some((to, _)) = self.get_account_by_address(&tx.to) {
            let transfer_op = TransferOp { tx, from, to };

            TransferOutcome::Transfer(transfer_op)
        } else {
            let to = self.get_free_account_id();
            let transfer_to_new_op = TransferToNewOp { tx, from, to };

            TransferOutcome::TransferToNew(transfer_to_new_op)
        };

        Ok(outcome)
    }

    fn apply_transfer(&mut self, tx: Transfer) -> Result<OpSuccess, Error> {
        let transfer = self.create_transfer_op(tx)?;

        match transfer {
            TransferOutcome::Transfer(transfer_op) => {
                let (fee, updates) = self.apply_transfer_op(&transfer_op)?;
                Ok(OpSuccess {
                    fee: Some(fee),
                    updates,
                    executed_op: FranklinOp::Transfer(Box::new(transfer_op)),
                })
            }
            TransferOutcome::TransferToNew(transfer_to_new_op) => {
                let (fee, updates) = self.apply_transfer_to_new_op(&transfer_to_new_op)?;
                Ok(OpSuccess {
                    fee: Some(fee),
                    updates,
                    executed_op: FranklinOp::TransferToNew(Box::new(transfer_to_new_op)),
                })
            }
        }
    }

    fn create_transfer_from_op(&self, tx: TransferFrom) -> Result<TransferFromOp, Error> {
        ensure!(
            tx.token <= params::max_token_id(),
            "Token id is not supported"
        );

        let (to, to_account) = self
            .get_account_by_address(&tx.to)
            .ok_or_else(|| format_err!("`To` account does not exist"))?;
        ensure!(
            to_account.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            tx.verify_sender_signature() == Some(to_account.pub_key_hash),
            "TransferFrom `to` signature is incorrect"
        );

        let (from, from_account) = self
            .get_account_by_address(&tx.from)
            .ok_or_else(|| format_err!("`From` account does not exist"))?;
        ensure!(
            from_account.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            tx.verify_from_signature() == Some(from_account.pub_key_hash),
            "TransferFrom `from` signature is incorrect"
        );

        ensure!(to == tx.account_id, "Transfer account id is incorrect");

        Ok(TransferFromOp { tx, from, to })
    }

    pub fn apply_transfer_from_op(
        &mut self,
        op: &TransferFromOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        ensure!(
            op.from <= max_account_id(),
            "Transfer from account id is bigger than max supported"
        );
        ensure!(
            op.to <= max_account_id(),
            "Transfer to account id is bigger than max supported"
        );

        ensure!(
            op.from != op.to,
            "TransferFrom `from` and `to` accounts are same"
        );

        let mut updates = Vec::new();
        let mut from_account = self.get_account(op.from).unwrap();
        let mut to_account = self.get_account(op.to).unwrap();

        // Update `from` account
        let from_old_balance = from_account.get_balance(op.tx.token);
        ensure!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            "Not enough balance"
        );

        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        let from_new_balance = from_account.get_balance(op.tx.token);
        let from_account_nonce = from_account.nonce;

        // Update `to` account
        let to_old_nonce = to_account.nonce;
        ensure!(op.tx.nonce == to_old_nonce, "Nonce mismatch");

        let to_old_balance = to_account.get_balance(op.tx.token);
        to_account.add_balance(op.tx.token, &op.tx.amount);
        to_account.nonce += 1;
        let to_new_balance = to_account.get_balance(op.tx.token);
        let to_new_nonce = to_account.nonce;

        self.insert_account(op.from, from_account);
        self.insert_account(op.to, to_account);

        updates.push((
            op.from,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, from_old_balance, from_new_balance),
                old_nonce: from_account_nonce,
                new_nonce: from_account_nonce,
            },
        ));

        updates.push((
            op.to,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, to_old_balance, to_new_balance),
                old_nonce: to_old_nonce,
                new_nonce: to_new_nonce,
            },
        ));

        let fee = CollectedFee {
            token: op.tx.token,
            amount: op.tx.fee.clone(),
        };

        Ok((fee, updates))
    }

    fn apply_transfer_from(&mut self, tx: TransferFrom) -> Result<OpSuccess, Error> {
        let transfer_from_op = self.create_transfer_from_op(tx)?;

        let (fee, updates) = self.apply_transfer_from_op(&transfer_from_op)?;
        Ok(OpSuccess {
            fee: Some(fee),
            updates,
            executed_op: FranklinOp::TransferFrom(Box::new(transfer_from_op)),
        })
    }

    fn create_withdraw_op(&self, tx: Withdraw) -> Result<WithdrawOp, Error> {
        ensure!(
            tx.token <= params::max_token_id(),
            "Token id is not supported"
        );
        let (account_id, account) = self
            .get_account_by_address(&tx.from)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        ensure!(
            account.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            tx.verify_signature() == Some(account.pub_key_hash),
            "withdraw signature is incorrect"
        );
        ensure!(
            account_id == tx.account_id,
            "Withdraw account id is incorrect"
        );
        let withdraw_op = WithdrawOp { tx, account_id };

        Ok(withdraw_op)
    }

    fn apply_withdraw(&mut self, tx: Withdraw) -> Result<OpSuccess, Error> {
        let withdraw_op = self.create_withdraw_op(tx)?;

        let (fee, updates) = self.apply_withdraw_op(&withdraw_op)?;
        Ok(OpSuccess {
            fee: Some(fee),
            updates,
            executed_op: FranklinOp::Withdraw(Box::new(withdraw_op)),
        })
    }

    fn apply_close(&mut self, _tx: Close) -> Result<OpSuccess, Error> {
        bail!("Account closing is disabled");
        // let (account_id, account) = self
        //     .get_account_by_address(&tx.account)
        //     .ok_or_else(|| format_err!("Account does not exist"))?;
        // let close_op = CloseOp { tx, account_id };
        //        ensure!(account.pub_key_hash != PubKeyHash::default(), "Account is locked");
        // ensure!(
        //     tx.verify_signature() == Some(account.pub_key_hash),
        //     "withdraw signature is incorrect"
        // );

        // let (fee, updates) = self.apply_close_op(&close_op)?;
        // Ok(OpSuccess {
        //     fee: Some(fee),
        //     updates,
        //     executed_op: FranklinOp::Close(Box::new(close_op)),
        // })
    }

    fn create_change_pubkey_op(&self, tx: ChangePubKey) -> Result<ChangePubKeyOp, Error> {
        let (account_id, account) = self
            .get_account_by_address(&tx.account)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        ensure!(
            tx.eth_signature.is_none() || tx.verify_eth_signature() == Some(account.address),
            "ChangePubKey signature is incorrect"
        );
        ensure!(
            account_id == tx.account_id,
            "ChangePubKey account id is incorrect"
        );
        ensure!(
            account_id <= params::max_account_id(),
            "ChangePubKey account id is bigger than max supported"
        );
        let change_pk_op = ChangePubKeyOp { tx, account_id };

        Ok(change_pk_op)
    }

    fn apply_change_pubkey(&mut self, tx: ChangePubKey) -> Result<OpSuccess, Error> {
        let change_pk_op = self.create_change_pubkey_op(tx)?;

        let (fee, updates) = self.apply_change_pubkey_op(&change_pk_op)?;
        Ok(OpSuccess {
            fee: Some(fee),
            updates,
            executed_op: FranklinOp::ChangePubKeyOffchain(Box::new(change_pk_op)),
        })
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
        self.account_id_by_address
            .insert(account.address.clone(), id);
        self.balance_tree.insert(id, account);
    }

    #[allow(dead_code)]
    fn remove_account(&mut self, id: AccountId) {
        if let Some(account) = self.get_account(id) {
            self.account_id_by_address.remove(&account.address);
            self.balance_tree.remove(id);
        }
    }

    pub fn apply_deposit_op(&mut self, op: &DepositOp) -> AccountUpdates {
        let mut updates = Vec::new();

        let mut account = self.get_account(op.account_id).unwrap_or_else(|| {
            let (account, upd) = Account::create_account(op.account_id, op.priority_op.to);
            updates.extend(upd.into_iter());
            account
        });

        let old_amount = account.get_balance(op.priority_op.token);
        let old_nonce = account.nonce;
        account.add_balance(op.priority_op.token, &op.priority_op.amount);
        let new_amount = account.get_balance(op.priority_op.token);

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

    pub fn apply_transfer_to_new_op(
        &mut self,
        op: &TransferToNewOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        let mut updates = Vec::new();

        ensure!(
            op.from <= max_account_id(),
            "TransferToNew from account id is bigger than max supported"
        );
        ensure!(
            op.to <= max_account_id(),
            "TransferToNew to account id is bigger than max supported"
        );

        assert!(
            self.get_account(op.to).is_none(),
            "Transfer to new account exists"
        );
        let mut to_account = {
            let (acc, upd) = Account::create_account(op.to, op.tx.to);
            updates.extend(upd.into_iter());
            acc
        };

        let mut from_account = self.get_account(op.from).unwrap();
        let from_old_balance = from_account.get_balance(op.tx.token);
        let from_old_nonce = from_account.nonce;
        ensure!(op.tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            "Not enough balance"
        );
        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        from_account.nonce += 1;
        let from_new_balance = from_account.get_balance(op.tx.token);
        let from_new_nonce = from_account.nonce;

        let to_old_balance = to_account.get_balance(op.tx.token);
        let to_account_nonce = to_account.nonce;
        to_account.add_balance(op.tx.token, &op.tx.amount);
        let to_new_balance = to_account.get_balance(op.tx.token);

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

    pub fn apply_withdraw_op(
        &mut self,
        op: &WithdrawOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        ensure!(
            op.account_id <= max_account_id(),
            "Withdraw account id is bigger than max supported"
        );

        let mut updates = Vec::new();
        let mut from_account = self.get_account(op.account_id).unwrap();

        let from_old_balance = from_account.get_balance(op.tx.token);
        let from_old_nonce = from_account.nonce;

        ensure!(op.tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            "Not enough balance"
        );

        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        from_account.nonce += 1;

        let from_new_balance = from_account.get_balance(op.tx.token);
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

    pub fn apply_close_op(
        &mut self,
        op: &CloseOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        ensure!(
            op.account_id <= max_account_id(),
            "Close account id is bigger than max supported"
        );

        let mut updates = Vec::new();
        let account = self.get_account(op.account_id).unwrap();

        for token in 0..params::total_tokens() {
            if account.get_balance(token as TokenId) != BigUint::from(0u32) {
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
            amount: BigUint::from(0u32),
        };

        Ok((fee, updates))
    }

    pub fn apply_change_pubkey_op(
        &mut self,
        op: &ChangePubKeyOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        let mut updates = Vec::new();
        let mut account = self.get_account(op.account_id).unwrap();

        let old_pub_key_hash = account.pub_key_hash.clone();
        let old_nonce = account.nonce;

        ensure!(op.tx.nonce == account.nonce, "Nonce mismatch");
        account.pub_key_hash = op.tx.new_pk_hash.clone();
        account.nonce += 1;

        let new_pub_key_hash = account.pub_key_hash.clone();
        let new_nonce = account.nonce;

        self.insert_account(op.account_id, account);

        updates.push((
            op.account_id,
            AccountUpdate::ChangePubKeyHash {
                old_pub_key_hash,
                old_nonce,
                new_pub_key_hash,
                new_nonce,
            },
        ));

        let fee = CollectedFee {
            token: params::ETH_TOKEN_ID,
            amount: BigUint::from(0u32),
        };

        Ok((fee, updates))
    }

    pub fn apply_transfer_op(
        &mut self,
        op: &TransferOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        ensure!(
            op.from <= max_account_id(),
            "Transfer from account id is bigger than max supported"
        );
        ensure!(
            op.to <= max_account_id(),
            "Transfer to account id is bigger than max supported"
        );

        if op.from == op.to {
            return self.apply_transfer_op_to_self(op);
        }

        let mut updates = Vec::new();
        let mut from_account = self.get_account(op.from).unwrap();
        let mut to_account = self.get_account(op.to).unwrap();

        let from_old_balance = from_account.get_balance(op.tx.token);
        let from_old_nonce = from_account.nonce;

        ensure!(op.tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            "Not enough balance"
        );

        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        from_account.nonce += 1;

        let from_new_balance = from_account.get_balance(op.tx.token);
        let from_new_nonce = from_account.nonce;

        let to_old_balance = to_account.get_balance(op.tx.token);
        let to_account_nonce = to_account.nonce;

        to_account.add_balance(op.tx.token, &op.tx.amount);

        let to_new_balance = to_account.get_balance(op.tx.token);

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

    fn apply_transfer_op_to_self(
        &mut self,
        op: &TransferOp,
    ) -> Result<(CollectedFee, AccountUpdates), Error> {
        ensure!(
            op.from <= max_account_id(),
            "Transfer to self from account id is bigger than max supported"
        );
        ensure!(
            op.from == op.to,
            "Bug: transfer to self should not be called."
        );

        let mut updates = Vec::new();
        let mut account = self.get_account(op.from).unwrap();

        let old_balance = account.get_balance(op.tx.token);
        let old_nonce = account.nonce;

        ensure!(op.tx.nonce == old_nonce, "Nonce mismatch");
        ensure!(
            old_balance >= &op.tx.amount + &op.tx.fee,
            "Not enough balance"
        );

        account.sub_balance(op.tx.token, &op.tx.fee);
        account.nonce += 1;

        let new_balance = account.get_balance(op.tx.token);
        let new_nonce = account.nonce;

        self.insert_account(op.from, account);

        updates.push((
            op.from,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, old_balance, new_balance),
                old_nonce,
                new_nonce,
            },
        ));

        let fee = CollectedFee {
            token: op.tx.token,
            amount: op.tx.fee.clone(),
        };

        Ok((fee, updates))
    }

    /// Converts the `FranklinTx` object to a `FranklinOp`, without applying it.
    pub fn franklin_tx_to_franklin_op(&self, tx: FranklinTx) -> Result<FranklinOp, Error> {
        match tx {
            FranklinTx::Transfer(tx) => self
                .create_transfer_op(*tx)
                .map(TransferOutcome::into_franklin_op),
            FranklinTx::TransferFrom(tx) => self.create_transfer_from_op(*tx).map(Into::into),
            FranklinTx::Withdraw(tx) => self.create_withdraw_op(*tx).map(Into::into),
            FranklinTx::ChangePubKey(tx) => self.create_change_pubkey_op(*tx).map(Into::into),
            FranklinTx::Close(_) => failure::bail!("Close op is disabled"),
        }
    }

    /// Converts the `PriorityOp` object to a `FranklinOp`, without applying it.
    pub fn priority_op_to_franklin_op(&self, op: FranklinPriorityOp) -> FranklinOp {
        match op {
            FranklinPriorityOp::Deposit(op) => self.create_deposit_op(op).into(),
            FranklinPriorityOp::FullExit(op) => self.create_full_exit_op(op).into(),
        }
    }
}
