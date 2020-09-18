use failure::{bail, ensure};
use models::{
    node::{AccountUpdate, AccountUpdates, Close, CloseOp, TokenId},
    params::{self, max_account_id},
};
use num::BigUint;

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, PlasmaState},
};

impl TxHandler<Close> for PlasmaState {
    type Op = CloseOp;

    fn create_op(&self, _tx: Close) -> Result<Self::Op, failure::Error> {
        panic!("Attempt to create disabled closed op");
    }

    fn apply_tx(&mut self, _tx: Close) -> Result<OpSuccess, failure::Error> {
        bail!("Account closing is disabled");
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), failure::Error> {
        ensure!(
            op.account_id <= max_account_id(),
            "Close account id is bigger than max supported"
        );

        let mut updates = Vec::new();
        let account = self.get_account(op.account_id).unwrap();

        for token in 0..params::total_tokens() {
            if account.get_balance(token as TokenId) != BigUint::from(0u32) {
                bail!("Account is not empty, token id: {}", token);
            }
        }

        ensure!(op.tx.nonce == account.nonce, "Nonce mismatch");

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
