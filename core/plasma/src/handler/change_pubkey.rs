use failure::{ensure, format_err};
use models::{
    node::{
        operations::{ChangePubKeyOp, FranklinOp},
        tx::ChangePubKey,
        AccountUpdate, AccountUpdates,
    },
    params,
};
use num::BigUint;

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, PlasmaState},
};

impl TxHandler<ChangePubKey> for PlasmaState {
    type Op = ChangePubKeyOp;

    fn create_op(&self, tx: ChangePubKey) -> Result<Self::Op, failure::Error> {
        let (account_id, account) = self
            .get_account_by_address(&tx.account)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        ensure!(
            tx.eth_signature.is_none() || tx.verify_eth_signature() == Some(account.address),
            "ChangePubKey signature is incorrect"
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

    fn apply_tx(&mut self, tx: ChangePubKey) -> Result<OpSuccess, failure::Error> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<ChangePubKey>>::apply_op(self, &op)?;
        Ok(OpSuccess {
            fee,
            updates,
            executed_op: FranklinOp::ChangePubKeyOffchain(Box::new(op)),
        })
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), failure::Error> {
        let mut updates = Vec::new();
        let mut account = self.get_account(op.account_id).unwrap();

        let old_pub_key_hash = account.pub_key_hash.clone();
        let old_nonce = account.nonce;

        ensure!(op.tx.nonce == account.nonce, "Nonce mismatch");
        account.pub_key_hash = op.tx.new_pk_hash.clone();
        account.nonce += 1;

        let new_pub_key_hash = account.pub_key_hash.clone();
        let new_nonce = account.nonce;

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

        let fee = CollectedFee {
            token: params::ETH_TOKEN_ID,
            amount: BigUint::from(0u32),
        };

        Ok((Some(fee), updates))
    }
}
