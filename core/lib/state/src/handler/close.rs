use num::BigUint;
use zksync_crypto::params::{self, max_account_id};
use zksync_types::{AccountUpdate, AccountUpdates, Close, CloseOp, TokenId};

use crate::{
    handler::{error::CloseOpError, TxHandler},
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<Close> for ZkSyncState {
    type Op = CloseOp;

    type OpError = CloseOpError;

    fn create_op(&self, _tx: Close) -> Result<Self::Op, CloseOpError> {
        panic!("Attempt to create disabled closed op");
    }

    fn apply_tx(&mut self, _tx: Close) -> Result<OpSuccess, CloseOpError> {
        Err(CloseOpError::CloseOperationsDisabled)
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), CloseOpError> {
        invariant!(
            op.account_id <= max_account_id(),
            CloseOpError::InvalidAccountId
        );

        let mut updates = Vec::new();
        let account = self.get_account(op.account_id).unwrap();

        for token in 0..params::total_tokens() {
            invariant!(
                account.get_balance(TokenId(token as u32)) == BigUint::from(0u32),
                CloseOpError::AccountNotEmpty(token)
            );
        }

        invariant!(op.tx.nonce == account.nonce, CloseOpError::NonceMismatch);

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

        Ok((Some(fee), updates))
    }
}
