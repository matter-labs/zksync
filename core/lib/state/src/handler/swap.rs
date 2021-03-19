use anyhow::{ensure, format_err};
use num::Zero;
use std::time::Instant;
use zksync_crypto::params;
use zksync_types::{AccountUpdate, AccountUpdates, PubKeyHash, Swap, SwapOp};

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<Swap> for ZkSyncState {
    type Op = SwapOp;

    fn create_op(&self, tx: Swap) -> Result<Self::Op, anyhow::Error> {
        ensure!(
            tx.submitter_id <= params::max_account_id(),
            "Account id is too big"
        );
        ensure!(
            tx.orders.0.account_id <= params::max_account_id(),
            "Account id is too big"
        );
        ensure!(
            tx.orders.1.account_id <= params::max_account_id(),
            "Account id is too big"
        );
        ensure!(
            tx.orders.0.recipient_id <= params::max_account_id(),
            "Recipient account id is too big"
        );
        ensure!(
            tx.orders.1.recipient_id <= params::max_account_id(),
            "Recipient account id is too big"
        );
        ensure!(
            tx.orders.0.token_buy <= params::max_token_id(),
            "Token is not supported"
        );
        ensure!(
            tx.orders.1.token_buy <= params::max_token_id(),
            "Token is not supported"
        );
        ensure!(
            tx.fee_token <= params::max_token_id(),
            "Token is not supported"
        );

        let (submitter, submitter_account) = self
            .get_account_by_address(&tx.submitter_address)
            .ok_or_else(|| format_err!("Submitter account does not exist"))?;
        let account_0 = self
            .get_account(tx.orders.0.account_id)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        let account_1 = self
            .get_account(tx.orders.1.account_id)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        let _recipient_0 = self
            .get_account(tx.orders.1.account_id)
            .ok_or_else(|| format_err!("Recipient account does not exist"))?;
        let _recipient_1 = self
            .get_account(tx.orders.1.account_id)
            .ok_or_else(|| format_err!("Recipient account does not exist"))?;

        ensure!(
            submitter_account.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            account_0.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            account_1.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            tx.verify_signature() == Some(submitter_account.pub_key_hash),
            "Swap signature is incorrect"
        );
        ensure!(
            tx.orders.0.verify_signature() == Some(account_0.pub_key_hash),
            "Order signature is incorrect"
        );
        ensure!(
            tx.orders.1.verify_signature() == Some(account_1.pub_key_hash),
            "Order signature is incorrect"
        );
        ensure!(
            submitter == tx.submitter_id,
            "Submitter account id is incorrect"
        );

        Ok(SwapOp {
            tx: tx.clone(),
            submitter,
            accounts: (tx.orders.0.account_id, tx.orders.1.account_id),
            recipients: (tx.orders.0.recipient_id, tx.orders.1.recipient_id),
        })
    }

    fn apply_tx(&mut self, tx: Swap) -> Result<OpSuccess, anyhow::Error> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<Swap>>::apply_op(self, &op)?;
        Ok(OpSuccess {
            fee,
            updates,
            executed_op: op.into(),
        })
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), anyhow::Error> {
        self.apply_swap_op(&op)
    }
}

impl ZkSyncState {
    fn apply_swap_op(
        &mut self,
        op: &SwapOp,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), anyhow::Error> {
        let start = Instant::now();

        let mut updates = Vec::new();
        let submitter = self.get_account(op.submitter).unwrap();
        let account_0 = self.get_account(op.accounts.0).unwrap();
        let account_1 = self.get_account(op.accounts.1).unwrap();
        let token_0 = op.tx.orders.0.token_sell;
        let token_1 = op.tx.orders.1.token_sell;

        ensure!(op.tx.nonce == submitter.nonce, "Nonce mismatch");
        ensure!(op.tx.orders.0.nonce == account_0.nonce, "Nonce mismatch");
        ensure!(op.tx.orders.1.nonce == account_1.nonce, "Nonce mismatch");
        ensure!(token_0 != token_1, "Can't swap for the same token");
        ensure!(
            op.tx.orders.0.token_buy == op.tx.orders.1.token_sell,
            "Buy/Sell tokens do not match"
        );
        ensure!(
            op.tx.orders.1.token_buy == op.tx.orders.0.token_sell,
            "Buy/Sell tokens do not match"
        );
        ensure!(
            &op.tx.orders.0.price.0 * &op.tx.orders.1.price.0
                >= &op.tx.orders.0.price.1 * &op.tx.orders.1.price.1,
            "Prices are not compatible"
        );
        ensure!(
            &op.tx.amounts.0 * &op.tx.orders.0.price.1
                <= &op.tx.amounts.1 * &op.tx.orders.0.price.0,
            "Amounts not compatible with prices"
        );
        ensure!(
            &op.tx.amounts.1 * &op.tx.orders.1.price.1
                <= &op.tx.amounts.0 * &op.tx.orders.1.price.0,
            "Amounts not compatible with prices"
        );
        ensure!(
            op.tx.orders.0.amount.is_zero() || op.tx.orders.0.amount == op.tx.amounts.0,
            "Amounts do not match"
        );
        ensure!(
            op.tx.orders.1.amount.is_zero() || op.tx.orders.1.amount == op.tx.amounts.1,
            "Amounts do not match"
        );
        ensure!(
            submitter.get_balance(op.tx.fee_token) >= op.tx.fee,
            "Not enough balance"
        );

        if op.tx.submitter_id == op.accounts.0 && op.tx.fee_token == token_0 {
            ensure!(
                submitter.get_balance(op.tx.fee_token) >= &op.tx.amounts.0 + &op.tx.fee,
                "Not enough balance"
            );
        } else {
            ensure!(
                account_0.get_balance(token_0) >= op.tx.amounts.0,
                "Not enough balance"
            );
        }

        if op.tx.submitter_id == op.accounts.1 && op.tx.fee_token == token_1 {
            ensure!(
                submitter.get_balance(op.tx.fee_token) >= &op.tx.amounts.1 + &op.tx.fee,
                "Not enough balance"
            );
        } else {
            ensure!(
                account_1.get_balance(token_1) >= op.tx.amounts.1,
                "Not enough balance"
            );
        }

        let mut update_account = |account_id, token, amount, nonce_inc, add| {
            let mut account = self.get_account(account_id).unwrap();
            let old_balance = account.get_balance(token);

            if add {
                account.add_balance(token, amount);
            } else {
                account.sub_balance(token, amount);
            }

            let new_balance = account.get_balance(token);
            let old_nonce = account.nonce;
            *account.nonce += nonce_inc;
            self.insert_account(account_id, account);

            updates.push((
                account_id,
                AccountUpdate::UpdateBalance {
                    balance_update: (token, old_balance, new_balance),
                    old_nonce,
                    new_nonce: old_nonce + nonce_inc,
                },
            ));
        };

        let increment_0 =
            (!op.tx.orders.0.amount.is_zero() && op.accounts.0 != op.submitter) as u32;
        let increment_1 =
            (!op.tx.orders.1.amount.is_zero() && op.accounts.1 != op.submitter) as u32;

        update_account(op.submitter, op.tx.fee_token, &op.tx.fee, 1, false);
        update_account(op.accounts.0, token_0, &op.tx.amounts.0, increment_0, false);
        update_account(op.accounts.1, token_1, &op.tx.amounts.1, increment_1, false);
        update_account(op.recipients.0, token_1, &op.tx.amounts.1, 0, true);
        update_account(op.recipients.1, token_0, &op.tx.amounts.0, 0, true);

        let fee = CollectedFee {
            token: op.tx.fee_token,
            amount: op.tx.fee.clone(),
        };

        metrics::histogram!("state.swap", start.elapsed());
        Ok((Some(fee), updates))
    }
}
