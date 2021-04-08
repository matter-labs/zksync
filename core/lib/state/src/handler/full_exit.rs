use num::BigUint;
use std::time::Instant;
use zksync_crypto::params;
use zksync_types::{AccountUpdate, AccountUpdates, FullExit, FullExitOp, ZkSyncOp};
use zksync_utils::BigUintSerdeWrapper;

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};
use std::convert::Infallible;

impl TxHandler<FullExit> for ZkSyncState {
    type Op = FullExitOp;

    type OpError = Infallible;

    fn create_op(&self, priority_op: FullExit) -> Result<Self::Op, Infallible> {
        // NOTE: Authorization of the FullExit is verified on the contract.
        assert!(
            priority_op.token <= params::max_token_id(),
            "Full exit token is out of range, this should be enforced by contract"
        );
        vlog::debug!("Processing {:?}", priority_op);
        let account_balance = self
            .get_account(priority_op.account_id)
            .filter(|account| account.address == priority_op.eth_address)
            .map(|acccount| acccount.get_balance(priority_op.token))
            .map(BigUintSerdeWrapper);

        vlog::debug!("Balance: {:?}", account_balance);
        let op = FullExitOp {
            priority_op,
            withdraw_amount: account_balance,
        };

        Ok(op)
    }

    fn apply_tx(&mut self, priority_op: FullExit) -> Result<OpSuccess, Infallible> {
        let op = self.create_op(priority_op)?;

        let (fee, updates) = <Self as TxHandler<FullExit>>::apply_op(self, &op)?;
        let result = OpSuccess {
            fee,
            updates,
            executed_op: ZkSyncOp::FullExit(Box::new(op)),
        };

        Ok(result)
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), Infallible> {
        let start = Instant::now();
        let mut updates = Vec::new();
        let amount = if let Some(amount) = &op.withdraw_amount {
            amount.clone()
        } else {
            return Ok((None, updates));
        };

        let account_id = op.priority_op.account_id;

        // expect is ok since account's existence was verified before
        let mut account = self
            .get_account(account_id)
            .expect("Full exit account not found");

        let old_balance = account.get_balance(op.priority_op.token);
        let old_nonce = account.nonce;

        account.sub_balance(op.priority_op.token, &amount.0);

        let new_balance = account.get_balance(op.priority_op.token);
        assert_eq!(
            new_balance,
            BigUint::from(0u32),
            "Full exit amount is incorrect"
        );
        let new_nonce = account.nonce;

        self.insert_account(account_id, account);
        updates.push((
            account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.priority_op.token, old_balance, new_balance),
                old_nonce,
                new_nonce,
            },
        ));

        let fee = None;

        metrics::histogram!("state.full_exit", start.elapsed());
        Ok((fee, updates))
    }
}
