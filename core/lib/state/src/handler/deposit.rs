use std::time::Instant;
use zksync_crypto::params;
use zksync_types::{Account, AccountUpdate, AccountUpdates, Deposit, DepositOp, ZkSyncOp};

use crate::{
    handler::{error::DepositOpError, TxHandler},
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<Deposit> for ZkSyncState {
    type Op = DepositOp;

    type OpError = DepositOpError;

    fn create_op(&self, priority_op: Deposit) -> Result<Self::Op, DepositOpError> {
        invariant!(
            priority_op.token <= params::max_fungible_token_id(),
            DepositOpError::InvalidToken
        );
        let account_id = if let Some((account_id, _)) = self.get_account_by_address(&priority_op.to)
        {
            account_id
        } else {
            self.get_free_account_id()
        };

        let op = DepositOp {
            priority_op,
            account_id,
        };

        Ok(op)
    }

    fn apply_tx(&mut self, priority_op: Deposit) -> Result<OpSuccess, DepositOpError> {
        let op = self.create_op(priority_op)?;

        let (fee, updates) = <Self as TxHandler<Deposit>>::apply_op(self, &op)?;
        let result = OpSuccess {
            fee,
            updates,
            executed_op: ZkSyncOp::Deposit(Box::new(op)),
        };

        Ok(result)
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), DepositOpError> {
        let start = Instant::now();
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

        let fee = None;

        metrics::histogram!("state.deposit", start.elapsed());
        Ok((fee, updates))
    }
}
