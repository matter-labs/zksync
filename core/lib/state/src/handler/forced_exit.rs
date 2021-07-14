use std::time::Instant;
use zksync_crypto::params;
use zksync_types::{AccountUpdate, AccountUpdates, ForcedExit, ForcedExitOp, PubKeyHash, ZkSyncOp};
use zksync_utils::BigUintSerdeWrapper;

use crate::{
    handler::{error::ForcedExitOpError, TxHandler},
    state::{CollectedFee, OpSuccess, ZkSyncState},
};
use num::{BigUint, Zero};

impl TxHandler<ForcedExit> for ZkSyncState {
    type Op = ForcedExitOp;

    type OpError = ForcedExitOpError;

    fn create_op(&self, tx: ForcedExit) -> Result<Self::Op, ForcedExitOpError> {
        // Check the tx signature.
        let initiator_account = self
            .get_account(tx.initiator_account_id)
            .ok_or(ForcedExitOpError::InitiatorAccountNotFound)?;

        if let Some((pub_key_hash, _)) = tx.verify_signature() {
            if pub_key_hash != initiator_account.pub_key_hash {
                return Err(ForcedExitOpError::InvalidSignature);
            }
        }

        // Check the token ID correctness.
        invariant!(
            tx.token <= params::max_fungible_token_id(),
            ForcedExitOpError::InvalidTokenId
        );

        if tx.fee != BigUint::zero() {
            // Fee can only be paid in processable tokens
            invariant!(
                tx.token <= params::max_processable_token(),
                ForcedExitOpError::InvalidFeeTokenId
            );
        }

        // Check that target account does not have an account ID set.
        let (target_account_id, account) = self
            .get_account_by_address(&tx.target)
            .ok_or(ForcedExitOpError::TargetAccountNotFound)?;

        invariant!(
            target_account_id != params::NFT_STORAGE_ACCOUNT_ID,
            ForcedExitOpError::TargetAccountNotFound
        );

        invariant!(
            account.pub_key_hash == PubKeyHash::default(),
            ForcedExitOpError::TargetAccountNotLocked
        );

        // Obtain the token balance to be withdrawn.
        let account_balance = self
            .get_account(target_account_id)
            .filter(|account| account.address == tx.target)
            .map(|account| account.get_balance(tx.token))
            .map(BigUintSerdeWrapper);

        let forced_exit_op = ForcedExitOp {
            tx,
            target_account_id,
            withdraw_amount: account_balance,
        };

        Ok(forced_exit_op)
    }

    fn apply_tx(&mut self, tx: ForcedExit) -> Result<OpSuccess, ForcedExitOpError> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<ForcedExit>>::apply_op(self, &op)?;
        Ok(OpSuccess {
            fee,
            updates,
            executed_op: ZkSyncOp::ForcedExit(Box::new(op)),
        })
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), ForcedExitOpError> {
        let start = Instant::now();
        invariant!(
            op.tx.initiator_account_id <= params::max_account_id(),
            ForcedExitOpError::IncorrectInitiatorAccount
        );

        let initiator_account_id = op.tx.initiator_account_id;
        let target_account_id = op.target_account_id;

        let mut updates = Vec::new();
        let mut initiator_account = self.get_account(initiator_account_id).unwrap();
        let mut target_account = self.get_account(target_account_id).unwrap();

        // Obtain the amount of tokens to withdraw from the target account.
        let amount = if let Some(amount) = &op.withdraw_amount {
            amount.clone().0
        } else {
            0u64.into()
        };

        // Check that initiator account has enough balance to cover fees.
        let initiator_old_balance = initiator_account.get_balance(op.tx.token);
        let initiator_old_nonce = initiator_account.nonce;

        invariant!(
            op.tx.nonce == initiator_old_nonce,
            ForcedExitOpError::NonceMismatch
        );
        invariant!(
            initiator_old_balance >= op.tx.fee,
            ForcedExitOpError::InitiatorInsufficientBalance
        );

        // Check that target account has required amount of tokens to withdraw.
        // (normally, it should, since we're declaring this amount ourselves, but
        // this check is added for additional safety).
        let target_old_balance = target_account.get_balance(op.tx.token);
        invariant!(
            target_old_balance == amount,
            ForcedExitOpError::TargetAccountBalanceMismatch
        );

        // Take fees from the initiator account (and update initiator account nonce).
        initiator_account.sub_balance(op.tx.token, &op.tx.fee);
        *initiator_account.nonce += 1;

        // Withdraw funds from the target account (note that target account nonce is not affected).
        target_account.sub_balance(op.tx.token, &amount);

        // Store required data to generate account updates later.
        let initiator_new_balance = initiator_account.get_balance(op.tx.token);
        let initiator_new_nonce = initiator_account.nonce;

        let target_new_balance = target_account.get_balance(op.tx.token);
        let target_nonce = target_account.nonce;

        // Update accounts in the tree.
        self.insert_account(op.tx.initiator_account_id, initiator_account);
        self.insert_account(op.target_account_id, target_account);

        updates.push((
            initiator_account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, initiator_old_balance, initiator_new_balance),
                old_nonce: initiator_old_nonce,
                new_nonce: initiator_new_nonce,
            },
        ));

        updates.push((
            target_account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, target_old_balance, target_new_balance),
                old_nonce: target_nonce,
                new_nonce: target_nonce,
            },
        ));

        let fee = CollectedFee {
            token: op.tx.token,
            amount: op.tx.fee.clone(),
        };

        metrics::histogram!("state.forced_exit", start.elapsed());
        Ok((Some(fee), updates))
    }
}
