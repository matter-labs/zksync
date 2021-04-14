use std::time::Instant;

use anyhow::{ensure, format_err};
use num::BigUint;

use zksync_crypto::params::{self, max_account_id};
use zksync_types::{
    AccountUpdate, AccountUpdates, PubKeyHash, TokenId, WithdrawNFT, WithdrawNFTOp, ZkSyncOp,
};

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<WithdrawNFT> for ZkSyncState {
    type Op = WithdrawNFTOp;

    fn create_op(&self, tx: WithdrawNFT) -> Result<Self::Op, anyhow::Error> {
        ensure!(
            tx.token <= params::max_token_id() && tx.token >= TokenId(params::MIN_NFT_TOKEN_ID),
            "Token id is not supported"
        );
        let (creator_id, _creator_account) = self
            .get_account_by_address(&tx.creator)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        let (account_id, account) = self
            .get_account_by_address(&tx.from)
            .ok_or_else(|| format_err!("Account does not exist"))?;
        ensure!(
            account.pub_key_hash != PubKeyHash::default(),
            "Account is locked"
        );
        ensure!(
            tx.verify_signature() == Some(account.pub_key_hash),
            "withdraw signature is incorrect"
        );
        ensure!(
            account_id == tx.account_id,
            "Withdraw account id is incorrect"
        );
        let withdraw_op = WithdrawNFTOp {
            tx,
            account_id,
            creator_id,
        };

        Ok(withdraw_op)
    }

    fn apply_tx(&mut self, tx: WithdrawNFT) -> Result<OpSuccess, anyhow::Error> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<WithdrawNFT>>::apply_op(self, &op)?;
        Ok(OpSuccess {
            fee,
            updates,
            executed_op: ZkSyncOp::WithdrawNFT(Box::new(op)),
        })
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), anyhow::Error> {
        let start = Instant::now();
        ensure!(
            op.account_id <= max_account_id(),
            "Withdraw account id is bigger than max supported"
        );
        ensure!(
            op.creator_id <= max_account_id(),
            "Withdraw creator id is bigger than max supported"
        );

        let mut updates = Vec::new();
        let mut from_account = self.get_account(op.account_id).unwrap();

        let from_old_balance = from_account.get_balance(op.tx.token);
        let from_old_nonce = from_account.nonce;

        ensure!(op.tx.nonce == from_old_nonce, "Nonce mismatch");
        ensure!(
            from_old_balance == BigUint::from(1u32),
            "NFT balance is not correct"
        );

        from_account.sub_balance(op.tx.token, &from_old_balance);
        *from_account.nonce += 1;

        let from_new_balance = from_account.get_balance(op.tx.token);
        let from_new_nonce = from_account.nonce;

        // Withdraw nft
        updates.push((
            op.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, from_old_balance, from_new_balance),
                old_nonce: from_old_nonce,
                new_nonce: from_new_nonce,
            },
        ));

        let from_old_balance = from_account.get_balance(op.tx.fee_token);
        from_account.sub_balance(op.tx.fee_token, &op.tx.fee);
        let from_new_balance = from_account.get_balance(op.tx.fee_token);
        ensure!(from_old_balance >= op.tx.fee, "Not enough balance");
        // Pay fee
        updates.push((
            op.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.fee_token, from_old_balance, from_new_balance),
                old_nonce: from_new_nonce,
                new_nonce: from_new_nonce,
            },
        ));

        self.insert_account(op.account_id, from_account);

        let fee = CollectedFee {
            token: op.tx.fee_token,
            amount: op.tx.fee.clone(),
        };

        metrics::histogram!("state.withdraw_nft", start.elapsed());
        Ok((Some(fee), updates))
    }
}
