use failure::{ensure, format_err};
use models::{
    node::{AccountUpdates, ForcedExit, ForcedExitOp, FranklinOp, PubKeyHash},
    params,
    primitives::BigUintSerdeWrapper,
};

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, PlasmaState},
};

impl TxHandler<ForcedExit> for PlasmaState {
    type Op = ForcedExitOp;

    fn create_op(&self, tx: ForcedExit) -> Result<Self::Op, failure::Error> {
        ensure!(
            tx.token <= params::max_token_id(),
            "Token id is not supported"
        );
        let (account_id, account) = self
            .get_account_by_address(&tx.from)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        ensure!(
            account.pub_key_hash == PubKeyHash::default(),
            "Account is not locked; forced exit is forbidden"
        );
        // ensure!(
        //     tx.verify_signature() == Some(account.pub_key_hash),
        //     "withdraw signature is incorrect"
        // );
        ensure!(
            account_id == tx.account_id,
            "Withdraw account id is incorrect"
        );

        let account_balance = self
            .get_account(tx.account_id)
            .filter(|account| account.address == tx.from)
            .map(|account| account.get_balance(tx.token))
            .map(BigUintSerdeWrapper);

        let forced_exit_op = ForcedExitOp {
            tx,
            account_id,
            withdraw_amount: account_balance,
        };

        Ok(forced_exit_op)
    }

    fn apply_tx(&mut self, tx: ForcedExit) -> Result<OpSuccess, failure::Error> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<ForcedExit>>::apply_op(self, &op)?;
        Ok(OpSuccess {
            fee,
            updates,
            executed_op: FranklinOp::ForcedExit(Box::new(op)),
        })
    }

    fn apply_op(
        &mut self,
        _op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), failure::Error> {
        todo!()

        // ensure!(
        //     op.account_id <= max_account_id(),
        //     "Withdraw account id is bigger than max supported"
        // );

        // let mut updates = Vec::new();
        // let mut from_account = self.get_account(op.account_id).unwrap();

        // let amount = if let Some(amount) = &op.withdraw_amount {
        //     amount.clone()
        // } else {
        //     0u64.into()
        // };

        // let from_old_balance = from_account.get_balance(op.tx.token);
        // let from_old_nonce = from_account.nonce;

        // ensure!(op.tx.nonce == from_old_nonce, "Nonce mismatch");
        // // ensure!(
        // //     from_old_balance >= &op.tx.amount + &op.tx.fee,
        // //     "Not enough balance"
        // // );

        // // TODO:
        // // We must subtract balances from two accounts here:
        // // - Fee from the tx initiator
        // // - Balance from the target account.
        // // from_account.sub_balance(op.tx.token, &(amount + &op.tx.fee));
        // // from_account.nonce += 1;

        // let from_new_balance = from_account.get_balance(op.tx.token);
        // let from_new_nonce = from_account.nonce;

        // self.insert_account(op.account_id, from_account);

        // updates.push((
        //     op.account_id,
        //     AccountUpdate::UpdateBalance {
        //         balance_update: (op.tx.token, from_old_balance, from_new_balance),
        //         old_nonce: from_old_nonce,
        //         new_nonce: from_new_nonce,
        //     },
        // ));

        // let fee = CollectedFee {
        //     token: op.tx.token,
        //     amount: op.tx.fee.clone(),
        // };

        // Ok((fee, updates))
    }
}
