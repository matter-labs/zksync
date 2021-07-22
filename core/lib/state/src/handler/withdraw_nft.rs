use std::time::Instant;

use num::BigUint;

use zksync_crypto::params::{self, max_account_id, max_processable_token};
use zksync_types::{
    AccountUpdate, AccountUpdates, PubKeyHash, TokenId, WithdrawNFT, WithdrawNFTOp, ZkSyncOp,
};

use crate::{
    handler::{error::WithdrawNFTOpError, TxHandler},
    state::{CollectedFee, OpSuccess, ZkSyncState},
};

impl TxHandler<WithdrawNFT> for ZkSyncState {
    type Op = WithdrawNFTOp;
    type OpError = WithdrawNFTOpError;

    fn create_op(&self, tx: WithdrawNFT) -> Result<Self::Op, Self::OpError> {
        invariant!(
            TokenId(params::MIN_NFT_TOKEN_ID) <= tx.token && tx.token <= params::max_token_id(),
            WithdrawNFTOpError::InvalidTokenId
        );
        let (account_id, account) = self
            .get_account_by_address(&tx.from)
            .ok_or(WithdrawNFTOpError::FromAccountIncorrect)?;

        invariant!(
            account.pub_key_hash != PubKeyHash::default(),
            WithdrawNFTOpError::FromAccountLocked
        );

        if let Some((pub_key_hash, _)) = tx.verify_signature() {
            if pub_key_hash != account.pub_key_hash {
                return Err(WithdrawNFTOpError::InvalidSignature);
            }
        }

        invariant!(
            account_id == tx.account_id,
            WithdrawNFTOpError::FromAccountIncorrect
        );
        invariant!(
            tx.fee_token <= max_processable_token(),
            WithdrawNFTOpError::InvalidFeeTokenId
        );
        if let Some(nft) = self.nfts.get(&tx.token) {
            let (creator_id, _creator_account) = self
                .get_account_by_address(&nft.creator_address)
                .ok_or(WithdrawNFTOpError::FromAccountNotFound)?;
            let withdraw_op = WithdrawNFTOp {
                tx,
                creator_id,
                creator_address: nft.creator_address,
                content_hash: nft.content_hash,
                serial_id: nft.serial_id,
            };

            Ok(withdraw_op)
        } else {
            Err(WithdrawNFTOpError::NFTNotFound)
        }
    }

    fn apply_tx(&mut self, tx: WithdrawNFT) -> Result<OpSuccess, Self::OpError> {
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
    ) -> Result<(Option<CollectedFee>, AccountUpdates), Self::OpError> {
        let start = Instant::now();
        invariant!(
            op.tx.account_id <= max_account_id(),
            WithdrawNFTOpError::FromAccountIncorrect
        );
        invariant!(
            op.creator_id <= max_account_id(),
            WithdrawNFTOpError::CreatorAccountIncorrect
        );

        let mut updates = Vec::new();
        let mut from_account = self.get_account(op.tx.account_id).unwrap();

        let from_old_balance = from_account.get_balance(op.tx.token);
        let from_old_nonce = from_account.nonce;

        invariant!(
            op.tx.nonce == from_old_nonce,
            WithdrawNFTOpError::NonceMismatch
        );
        invariant!(
            from_old_balance == BigUint::from(1u32),
            WithdrawNFTOpError::InsufficientNFTBalance
        );

        from_account.sub_balance(op.tx.token, &from_old_balance);
        *from_account.nonce += 1;

        let from_new_balance = from_account.get_balance(op.tx.token);
        let from_new_nonce = from_account.nonce;

        // Withdraw nft
        updates.push((
            op.tx.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.token, from_old_balance, from_new_balance),
                old_nonce: from_old_nonce,
                new_nonce: from_new_nonce,
            },
        ));

        let from_old_balance = from_account.get_balance(op.tx.fee_token);

        invariant!(
            from_old_balance >= op.tx.fee,
            WithdrawNFTOpError::InsufficientBalance
        );
        from_account.sub_balance(op.tx.fee_token, &op.tx.fee);
        let from_new_balance = from_account.get_balance(op.tx.fee_token);
        // Pay fee
        updates.push((
            op.tx.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.fee_token, from_old_balance, from_new_balance),
                old_nonce: from_new_nonce,
                new_nonce: from_new_nonce,
            },
        ));

        self.insert_account(op.tx.account_id, from_account);

        let fee = CollectedFee {
            token: op.tx.fee_token,
            amount: op.tx.fee.clone(),
        };

        metrics::histogram!("state.withdraw_nft", start.elapsed());
        Ok((Some(fee), updates))
    }
}
