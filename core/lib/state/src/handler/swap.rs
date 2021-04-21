use anyhow::{ensure, format_err};
use num::{BigUint, Zero};
use std::time::Instant;
use zksync_crypto::params::{max_account_id, max_fungible_token_id, max_token_id};
use zksync_types::{AccountUpdates, Order, PubKeyHash, Swap, SwapOp};

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<Swap> for ZkSyncState {
    type Op = SwapOp;

    fn create_op(&self, tx: Swap) -> Result<Self::Op, anyhow::Error> {
        self.verify_order(&tx.orders.0)?;
        self.verify_order(&tx.orders.1)?;
        ensure!(tx.submitter_id <= max_account_id(), "Account id is too big");
        ensure!(
            tx.fee_token <= max_fungible_token_id(),
            "Token is not supported"
        );

        let (submitter, submitter_account) = self
            .get_account_by_address(&tx.submitter_address)
            .ok_or_else(|| format_err!("Submitter account does not exist"))?;

        ensure!(
            submitter_account.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            tx.verify_signature() == Some(submitter_account.pub_key_hash),
            "Swap signature is incorrect"
        );
        ensure!(
            submitter == tx.submitter_id,
            "Submitter account_id or address is incorrect"
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
    fn verify_order(&self, order: &Order) -> anyhow::Result<()> {
        ensure!(
            order.account_id <= max_account_id(),
            "Account id is too big"
        );
        ensure!(
            order.recipient_id <= max_account_id(),
            "Account id is too big"
        );
        ensure!(order.token_buy <= max_token_id(), "Token is not supported");
        ensure!(order.token_sell <= max_token_id(), "Token is not supported");

        let account = self
            .get_account(order.account_id)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        let _recipient = self
            .get_account(order.recipient_id)
            .ok_or_else(|| format_err!("Recipient account does not exist"))?;

        ensure!(
            account.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            order.verify_signature() == Some(account.pub_key_hash),
            "Order signature is incorrect"
        );
        Ok(())
    }

    fn verify_swap_accounts(&self, swap: &Swap) -> anyhow::Result<()> {
        let submitter = self.get_account(swap.submitter_id).unwrap();

        ensure!(swap.nonce == submitter.nonce, "Nonce mismatch");
        ensure!(
            submitter.get_balance(swap.fee_token) >= swap.fee,
            "Not enough balance"
        );

        let verify_account = |order: &Order, amount: &BigUint| {
            let account = self.get_account(order.account_id).unwrap();
            ensure!(order.nonce == account.nonce, "Nonce mismatch");
            let necessary_amount =
                if swap.submitter_id == order.account_id && swap.fee_token == order.token_sell {
                    amount + &swap.fee
                } else {
                    amount.clone()
                };
            ensure!(
                account.get_balance(order.token_sell) >= necessary_amount,
                "Not enough balance"
            );
            Ok(())
        };

        verify_account(&swap.orders.0, &swap.amounts.0)?;
        verify_account(&swap.orders.1, &swap.amounts.1)
    }

    fn verify_swap(&self, swap: &Swap) -> anyhow::Result<()> {
        ensure!(
            swap.orders.0.token_buy == swap.orders.1.token_sell,
            "Buy/Sell tokens do not match"
        );
        ensure!(
            swap.orders.1.token_buy == swap.orders.0.token_sell,
            "Buy/Sell tokens do not match"
        );
        ensure!(
            swap.orders.0.token_sell != swap.orders.1.token_sell,
            "Can't swap for the same token"
        );
        ensure!(
            swap.orders.0.amount.is_zero() || swap.orders.0.amount == swap.amounts.0,
            "Amounts do not match"
        );
        ensure!(
            swap.orders.1.amount.is_zero() || swap.orders.1.amount == swap.amounts.1,
            "Amounts do not match"
        );

        let sold = &swap.amounts.0 * &swap.orders.0.price.1;
        let bought = &swap.amounts.1 * &swap.orders.0.price.0;
        ensure!(sold <= bought, "Amounts are not compatible with prices");

        let sold = &swap.amounts.1 * &swap.orders.1.price.1;
        let bought = &swap.amounts.0 * &swap.orders.1.price.0;
        ensure!(sold <= bought, "Amounts are not compatible with prices");
        Ok(())
    }

    fn apply_swap_op(
        &mut self,
        op: &SwapOp,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), anyhow::Error> {
        let start = Instant::now();

        self.verify_swap(&op.tx)?;
        self.verify_swap_accounts(&op.tx)?;

        let increment_0 =
            (!op.tx.orders.0.amount.is_zero() && op.accounts.0 != op.submitter) as u32;
        let increment_1 =
            (!op.tx.orders.1.amount.is_zero() && op.accounts.1 != op.submitter) as u32;
        let token_0 = op.tx.orders.0.token_sell;
        let token_1 = op.tx.orders.1.token_sell;
        let amounts = op.tx.amounts.clone();

        use crate::state::BalanceUpdate::*;

        let updates = vec![
            self.update_account(op.submitter, op.tx.fee_token, Sub(op.tx.fee.clone()), 1),
            self.update_account(op.accounts.0, token_0, Sub(amounts.0.clone()), increment_0),
            self.update_account(op.accounts.1, token_1, Sub(amounts.1.clone()), increment_1),
            self.update_account(op.recipients.0, token_1, Add(amounts.1), 0),
            self.update_account(op.recipients.1, token_0, Add(amounts.0), 0),
        ];

        let fee = CollectedFee {
            token: op.tx.fee_token,
            amount: op.tx.fee.clone(),
        };

        metrics::histogram!("state.swap", start.elapsed());
        Ok((Some(fee), updates))
    }
}
