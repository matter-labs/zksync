use anyhow::{ensure, format_err};
use zksync_crypto::params::{self, max_account_id};
use zksync_types::{AccountUpdate, AccountUpdates, PubKeyHash, Withdraw, WithdrawOp, ZkSyncOp};

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<Withdraw> for ZkSyncState {
    type Op = WithdrawOp;

    fn create_op(&self, tx: Withdraw) -> Result<Self::Op, anyhow::Error> {
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

    fn apply_tx(&mut self, tx: Withdraw) -> Result<OpSuccess, anyhow::Error> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<Withdraw>>::apply_op(self, &op)?;
        Ok(OpSuccess {
            fee,
            updates,
            executed_op: ZkSyncOp::Withdraw(Box::new(op)),
        })
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), anyhow::Error> {
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

        Ok((Some(fee), updates))
    }
}
