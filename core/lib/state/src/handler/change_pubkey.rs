use std::time::Instant;
use zksync_crypto::params;
use zksync_types::{
    operations::{ChangePubKeyOp, ZkSyncOp},
    tx::ChangePubKey,
    AccountUpdate, AccountUpdates,
};

use crate::{
    handler::{error::ChangePubKeyOpError, TxHandler},
    state::{CollectedFee, OpSuccess, ZkSyncState},
};
use zksync_crypto::params::max_processable_token;

impl TxHandler<ChangePubKey> for ZkSyncState {
    type Op = ChangePubKeyOp;
    type OpError = ChangePubKeyOpError;

    fn create_op(&self, tx: ChangePubKey) -> Result<Self::Op, ChangePubKeyOpError> {
        let (account_id, account) = self
            .get_account_by_address(&tx.account)
            .ok_or(ChangePubKeyOpError::AccountNotFound)?;
        invariant!(
            tx.account == account.address,
            ChangePubKeyOpError::InvalidAccountAddress
        );
        invariant!(
            tx.fee_token <= max_processable_token(),
            ChangePubKeyOpError::InvalidFeeTokenId
        );
        invariant!(
            tx.is_eth_auth_data_valid(),
            ChangePubKeyOpError::InvalidAuthData
        );

        if let Some((pub_key_hash, _)) = tx.verify_signature() {
            if pub_key_hash != tx.new_pk_hash {
                return Err(ChangePubKeyOpError::InvalidZksyncSignature);
            }
        }
        invariant!(
            account_id == tx.account_id,
            ChangePubKeyOpError::InvalidAccountId
        );
        invariant!(
            account_id <= params::max_account_id(),
            ChangePubKeyOpError::AccountIdTooBig
        );
        let change_pk_op = ChangePubKeyOp { tx, account_id };

        Ok(change_pk_op)
    }

    fn apply_tx(&mut self, tx: ChangePubKey) -> Result<OpSuccess, ChangePubKeyOpError> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<ChangePubKey>>::apply_op(self, &op)?;
        Ok(OpSuccess {
            fee,
            updates,
            executed_op: ZkSyncOp::ChangePubKeyOffchain(Box::new(op)),
        })
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), ChangePubKeyOpError> {
        let start = Instant::now();
        let mut updates = Vec::new();
        let mut account = self.get_account(op.account_id).unwrap();

        let old_balance = account.get_balance(op.tx.fee_token);

        let old_pub_key_hash = account.pub_key_hash;
        let old_nonce = account.nonce;

        // Update nonce.
        invariant!(
            op.tx.nonce == account.nonce,
            ChangePubKeyOpError::NonceMismatch
        );
        *account.nonce += 1;

        // Update pubkey hash.
        account.pub_key_hash = op.tx.new_pk_hash;

        // Subract fees.
        invariant!(
            old_balance >= op.tx.fee,
            ChangePubKeyOpError::InsufficientBalance
        );
        account.sub_balance(op.tx.fee_token, &op.tx.fee);

        let new_pub_key_hash = account.pub_key_hash;
        let new_nonce = account.nonce;
        let new_balance = account.get_balance(op.tx.fee_token);

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

        updates.push((
            op.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.fee_token, old_balance, new_balance),
                old_nonce: new_nonce,
                new_nonce,
            },
        ));

        let fee = CollectedFee {
            token: op.tx.fee_token,
            amount: op.tx.fee.clone(),
        };

        metrics::histogram!("state.change_pubkey", start.elapsed());
        Ok((Some(fee), updates))
    }
}
