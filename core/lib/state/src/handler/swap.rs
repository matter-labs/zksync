use num::{BigUint, Zero};
use std::time::Instant;
use zksync_crypto::params::{max_account_id, max_processable_token, max_token_id};
use zksync_types::{AccountUpdates, Order, PubKeyHash, Swap, SwapOp};

use crate::handler::error::SwapOpError;
use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<Swap> for ZkSyncState {
    type Op = SwapOp;
    type OpError = SwapOpError;

    fn create_op(&self, tx: Swap) -> Result<Self::Op, Self::OpError> {
        self.verify_order(&tx.orders.0)?;
        self.verify_order(&tx.orders.1)?;
        invariant!(
            tx.submitter_id <= max_account_id(),
            SwapOpError::AccountIncorrect
        );
        invariant!(
            tx.fee_token <= max_processable_token(),
            SwapOpError::InvalidTokenId
        );

        let (submitter, submitter_account) = self
            .get_account_by_address(&tx.submitter_address)
            .ok_or(SwapOpError::SubmitterAccountNotFound)?;

        invariant!(
            submitter_account.pub_key_hash != PubKeyHash::default(),
            SwapOpError::AccountLocked
        );

        if let Some((pub_key_hash, _)) = tx.verify_signature() {
            if pub_key_hash != submitter_account.pub_key_hash {
                return Err(SwapOpError::SwapInvalidSignature);
            }
        }
        invariant!(
            submitter == tx.submitter_id,
            SwapOpError::SubmitterAccountIncorrect
        );

        let (recipient_0, _) = self
            .get_account_by_address(&tx.orders.0.recipient_address)
            .ok_or(SwapOpError::RecipientAccountNotFound)?;
        let (recipient_1, _) = self
            .get_account_by_address(&tx.orders.1.recipient_address)
            .ok_or(SwapOpError::RecipientAccountNotFound)?;

        Ok(SwapOp {
            tx: tx.clone(),
            submitter,
            accounts: (tx.orders.0.account_id, tx.orders.1.account_id),
            recipients: (recipient_0, recipient_1),
        })
    }

    fn apply_tx(&mut self, tx: Swap) -> Result<OpSuccess, SwapOpError> {
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
    ) -> Result<(Option<CollectedFee>, AccountUpdates), SwapOpError> {
        self.apply_swap_op(&op)
    }
}

impl ZkSyncState {
    fn verify_order(&self, order: &Order) -> Result<(), SwapOpError> {
        invariant!(
            order.token_buy <= max_token_id(),
            SwapOpError::InvalidTokenId
        );
        invariant!(
            order.token_sell <= max_token_id(),
            SwapOpError::InvalidTokenId
        );
        invariant!(
            order.account_id <= max_account_id(),
            SwapOpError::AccountIncorrect
        );

        let account = self
            .get_account(order.account_id)
            .ok_or(SwapOpError::AccountIncorrect)?;
        let _recipient = self
            .get_account_by_address(&order.recipient_address)
            .ok_or(SwapOpError::RecipientAccountNotFound)?;

        invariant!(
            account.pub_key_hash != PubKeyHash::default(),
            SwapOpError::AccountLocked
        );
        invariant!(
            order.verify_signature() == Some(account.pub_key_hash),
            SwapOpError::OrderInvalidSignature
        );
        Ok(())
    }

    fn verify_swap_accounts(&self, op: &SwapOp) -> Result<(), SwapOpError> {
        let tx = &op.tx;
        let submitter = self.get_account(tx.submitter_id).unwrap();

        invariant!(tx.nonce == submitter.nonce, SwapOpError::NonceMismatch);
        let balance = submitter.get_balance(tx.fee_token);
        let future_balance =
            if tx.submitter_id == op.recipients.0 && tx.fee_token == tx.orders.0.token_buy {
                balance + &tx.amounts.1
            } else if tx.submitter_id == op.recipients.1 && tx.fee_token == tx.orders.1.token_buy {
                balance + &tx.amounts.0
            } else {
                balance
            };
        invariant!(future_balance >= tx.fee, SwapOpError::InsufficientBalance);

        let verify_account = |order: &Order, amount: &BigUint| {
            let account = self.get_account(order.account_id).unwrap();
            invariant!(order.nonce == account.nonce, SwapOpError::NonceMismatch);
            let necessary_amount =
                if tx.submitter_id == order.account_id && tx.fee_token == order.token_sell {
                    &tx.fee + amount
                } else {
                    amount.clone()
                };
            invariant!(
                account.get_balance(order.token_sell) >= necessary_amount,
                SwapOpError::InsufficientBalance
            );
            Ok(())
        };

        verify_account(&tx.orders.0, &tx.amounts.0)?;
        verify_account(&tx.orders.1, &tx.amounts.1)
    }

    fn verify_swap(&self, swap: &Swap) -> Result<(), SwapOpError> {
        invariant!(
            swap.orders.0.token_buy == swap.orders.1.token_sell,
            SwapOpError::BuySellNotMatched
        );
        invariant!(
            swap.orders.1.token_buy == swap.orders.0.token_sell,
            SwapOpError::BuySellNotMatched
        );
        invariant!(
            swap.orders.0.token_sell != swap.orders.1.token_sell,
            SwapOpError::SwapSameToken
        );
        invariant!(
            swap.orders.0.amount.is_zero() || swap.orders.0.amount == swap.amounts.0,
            SwapOpError::AmountsNotMatched
        );
        invariant!(
            swap.orders.1.amount.is_zero() || swap.orders.1.amount == swap.amounts.1,
            SwapOpError::AmountsNotMatched
        );
        invariant!(
            swap.orders.0.account_id != swap.orders.1.account_id,
            SwapOpError::SelfSwap
        );

        let sold = &swap.amounts.0 * &swap.orders.0.price.1;
        let bought = &swap.amounts.1 * &swap.orders.0.price.0;
        invariant!(sold <= bought, SwapOpError::AmountsNotCompatible);

        let sold = &swap.amounts.1 * &swap.orders.1.price.1;
        let bought = &swap.amounts.0 * &swap.orders.1.price.0;
        invariant!(sold <= bought, SwapOpError::AmountsNotCompatible);
        Ok(())
    }

    fn apply_swap_op(
        &mut self,
        op: &SwapOp,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), SwapOpError> {
        let start = Instant::now();

        self.verify_swap(&op.tx)?;
        self.verify_swap_accounts(op)?;

        let increment_0 =
            (!op.tx.orders.0.amount.is_zero() && op.accounts.0 != op.submitter) as u32;
        let increment_1 =
            (!op.tx.orders.1.amount.is_zero() && op.accounts.1 != op.submitter) as u32;
        let token_0 = op.tx.orders.0.token_sell;
        let token_1 = op.tx.orders.1.token_sell;
        let amounts = op.tx.amounts.clone();

        use crate::state::BalanceUpdate::*;

        let updates = vec![
            self.update_account(op.accounts.0, token_0, Sub(amounts.0.clone()), increment_0),
            self.update_account(op.recipients.1, token_0, Add(amounts.0), 0),
            self.update_account(op.accounts.1, token_1, Sub(amounts.1.clone()), increment_1),
            self.update_account(op.recipients.0, token_1, Add(amounts.1), 0),
            self.update_account(op.submitter, op.tx.fee_token, Sub(op.tx.fee.clone()), 1),
        ];

        let fee = CollectedFee {
            token: op.tx.fee_token,
            amount: op.tx.fee.clone(),
        };

        metrics::histogram!("state.swap", start.elapsed());
        Ok((Some(fee), updates))
    }
}
