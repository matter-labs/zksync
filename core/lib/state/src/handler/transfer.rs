use num::{BigUint, Zero};
use std::time::Instant;

use zksync_crypto::params::{self, max_account_id};
use zksync_types::{
    Account, AccountUpdate, AccountUpdates, Address, PubKeyHash, Transfer, TransferOp,
    TransferToNewOp,
};

use crate::{
    handler::{error::TransferOpError, TxHandler},
    state::{CollectedFee, OpSuccess, TransferOutcome, ZkSyncState},
};

impl TxHandler<Transfer> for ZkSyncState {
    type Op = TransferOutcome;

    type OpError = TransferOpError;

    fn create_op(&self, tx: Transfer) -> Result<Self::Op, TransferOpError> {
        invariant!(
            tx.token <= params::max_token_id(),
            TransferOpError::InvalidTokenId
        );
        if tx.fee != BigUint::zero() {
            // Fee can only be paid in processable tokens
            invariant!(
                tx.token <= params::max_processable_token(),
                TransferOpError::InvalidFeeTokenId
            );
        }
        invariant!(tx.to != Address::zero(), TransferOpError::TargetAccountZero);
        let (from, from_account) = self
            .get_account_by_address(&tx.from)
            .ok_or(TransferOpError::FromAccountNotFound)?;
        invariant!(
            from_account.pub_key_hash != PubKeyHash::default(),
            TransferOpError::FromAccountLocked
        );
        if let Some((pub_key_hash, _)) = tx.verify_signature() {
            if pub_key_hash != from_account.pub_key_hash {
                return Err(TransferOpError::InvalidSignature);
            }
        }
        invariant!(
            from == tx.account_id,
            TransferOpError::TransferAccountIncorrect
        );

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

    fn apply_tx(&mut self, tx: Transfer) -> Result<OpSuccess, TransferOpError> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<Transfer>>::apply_op(self, &op)?;
        Ok(OpSuccess {
            fee,
            updates,
            executed_op: op.into_franklin_op(),
        })
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), TransferOpError> {
        match op {
            TransferOutcome::Transfer(transfer_op) => self.apply_transfer_op(&transfer_op),
            TransferOutcome::TransferToNew(transfer_to_new_op) => {
                self.apply_transfer_to_new_op(&transfer_to_new_op)
            }
        }
    }
}

impl ZkSyncState {
    fn apply_transfer_op(
        &mut self,
        op: &TransferOp,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), TransferOpError> {
        let start = Instant::now();

        invariant!(
            op.from <= max_account_id(),
            TransferOpError::SourceAccountIncorrect
        );
        invariant!(
            op.to <= max_account_id(),
            TransferOpError::TargetAccountIncorrect
        );

        if op.from == op.to {
            return self.apply_transfer_op_to_self(op);
        }

        let mut updates = Vec::new();
        let mut from_account = self.get_account(op.from).unwrap();
        let mut to_account = self.get_account(op.to).unwrap();

        let from_old_balance = from_account.get_balance(op.tx.token);
        let from_old_nonce = from_account.nonce;

        invariant!(
            op.tx.nonce == from_old_nonce,
            TransferOpError::NonceMismatch
        );
        invariant!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            TransferOpError::InsufficientBalance
        );

        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        *from_account.nonce += 1;

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

        metrics::histogram!("state.transfer", start.elapsed());
        Ok((Some(fee), updates))
    }

    fn apply_transfer_op_to_self(
        &mut self,
        op: &TransferOp,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), TransferOpError> {
        let start = Instant::now();

        invariant!(
            op.from <= max_account_id(),
            TransferOpError::SourceAccountIncorrect
        );
        invariant!(op.from == op.to, TransferOpError::CannotTransferToSelf);

        let mut updates = Vec::new();
        let mut account = self.get_account(op.from).unwrap();

        let old_balance = account.get_balance(op.tx.token);
        let old_nonce = account.nonce;

        invariant!(op.tx.nonce == old_nonce, TransferOpError::NonceMismatch);
        invariant!(
            old_balance >= &op.tx.amount + &op.tx.fee,
            TransferOpError::InsufficientBalance
        );

        account.sub_balance(op.tx.token, &op.tx.fee);
        *account.nonce += 1;

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

        metrics::histogram!("state.transfer_to_self", start.elapsed());
        Ok((Some(fee), updates))
    }

    fn apply_transfer_to_new_op(
        &mut self,
        op: &TransferToNewOp,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), TransferOpError> {
        let start = Instant::now();
        let mut updates = Vec::new();
        invariant!(
            op.from <= max_account_id(),
            TransferOpError::SourceAccountIncorrect
        );
        invariant!(
            op.to <= max_account_id(),
            TransferOpError::TargetAccountIncorrect
        );

        if let Some(account) = self.get_account(op.to) {
            vlog::error!(
                "Attempt to execute transfer to new account for an existing account. Account: {:#?}; Transfer: {:#?}",
                account,
                op
            );
            panic!("Transfer to new account exists");
        }

        let mut to_account = {
            let (acc, upd) = Account::create_account(op.to, op.tx.to);
            updates.extend(upd.into_iter());
            acc
        };

        let mut from_account = self.get_account(op.from).unwrap();
        let from_old_balance = from_account.get_balance(op.tx.token);
        let from_old_nonce = from_account.nonce;
        invariant!(
            op.tx.nonce == from_old_nonce,
            TransferOpError::NonceMismatch
        );
        invariant!(
            from_old_balance >= &op.tx.amount + &op.tx.fee,
            TransferOpError::InsufficientBalance
        );
        from_account.sub_balance(op.tx.token, &(&op.tx.amount + &op.tx.fee));
        *from_account.nonce += 1;
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

        metrics::histogram!("state.transfer_to_new", start.elapsed());
        Ok((Some(fee), updates))
    }
}
