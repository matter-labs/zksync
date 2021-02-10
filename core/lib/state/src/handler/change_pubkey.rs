use anyhow::{ensure, format_err};
use std::time::Instant;
use zksync_crypto::params;
use zksync_types::{
    operations::{ChangePubKeyOp, ZkSyncOp},
    tx::ChangePubKey,
    AccountUpdate, AccountUpdates,
};

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<ChangePubKey> for ZkSyncState {
    type Op = ChangePubKeyOp;

    fn create_op(&self, tx: ChangePubKey) -> Result<Self::Op, anyhow::Error> {
        let (account_id, account) = self
            .get_account_by_address(&tx.account)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        ensure!(
            tx.account == account.address,
            "ChangePubKey account address is incorrect"
        );
        ensure!(
            tx.is_eth_auth_data_valid(),
            "ChangePubKey Ethereum auth data is incorrect"
        );
        ensure!(
            tx.verify_signature() == Some(tx.new_pk_hash),
            "ChangePubKey zkSync signature is incorrect"
        );
        ensure!(
            account_id == tx.account_id,
            "ChangePubKey account id is incorrect"
        );
        ensure!(
            account_id <= params::max_account_id(),
            "ChangePubKey account id is bigger than max supported"
        );
        let change_pk_op = ChangePubKeyOp { tx, account_id };

        Ok(change_pk_op)
    }

    fn apply_tx(&mut self, tx: ChangePubKey) -> Result<OpSuccess, anyhow::Error> {
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
    ) -> Result<(Option<CollectedFee>, AccountUpdates), anyhow::Error> {
        let start = Instant::now();
        let mut updates = Vec::new();
        let mut account = self.get_account(op.account_id).unwrap();

        let old_balance = account.get_balance(op.tx.fee_token);

        let old_pub_key_hash = account.pub_key_hash;
        let old_nonce = account.nonce;

        // Update nonce.
        ensure!(op.tx.nonce == account.nonce, "Nonce mismatch");
        *account.nonce += 1;

        // Update pubkey hash.
        account.pub_key_hash = op.tx.new_pk_hash;

        // Subract fees.
        ensure!(old_balance >= op.tx.fee, "Not enough balance");
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
