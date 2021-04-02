use anyhow::{bail, format_err};
use std::time::Instant;
use zksync_crypto::params;
use zksync_types::{
    operations::MintNFTOp, Account, AccountUpdate, AccountUpdates, MintNFT, ZkSyncOp,
};

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};
use num::{BigUint, Zero};
use zksync_types::tokens::NFT;

impl TxHandler<MintNFT> for ZkSyncState {
    type Op = MintNFTOp;

    fn create_op(&self, tx: MintNFT) -> Result<Self::Op, anyhow::Error> {
        assert!(
            tx.id <= params::max_token_id(),
            "NFT is out of range, this should be enforced by contract"
        );
        let op = MintNFTOp {
            account_id: tx.creator_id,
            tx,
        };

        Ok(op)
    }

    fn apply_tx(&mut self, tx: MintNFT) -> Result<OpSuccess, anyhow::Error> {
        let op = self.create_op(tx)?;

        let (fee, updates) = <Self as TxHandler<MintNFT>>::apply_op(self, &op)?;
        let result = OpSuccess {
            fee,
            updates,
            executed_op: ZkSyncOp::MintNFTOp(Box::new(op)),
        };

        Ok(result)
    }

    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), anyhow::Error> {
        let start = Instant::now();
        let mut updates = Vec::new();

        let mut recipient_account = self
            .get_account(op.tx.recipient_account_id)
            .ok_or(format_err!("Recipient account not found"))?;

        let token_account = if self.get_account(op.tx.account_id).is_none() {
            let (account, upd) = Account::create_account(op.tx.account_id, op.tx.address);
            updates.extend(upd.into_iter());
            account
        } else {
            bail!("Token account is already exists");
        };
        updates.push((
            op.tx.account_id,
            AccountUpdate::MintNFT {
                token: NFT::new(
                    op.tx.id,
                    op.tx.account_id,
                    op.tx.serial_id,
                    op.tx.creator_id,
                    op.tx.address,
                    None,
                    op.tx.content_hash,
                )?,
            },
        ));

        let old_amount = recipient_account.get_balance(op.tx.id);
        if old_amount != BigUint::zero() {
            bail!("Token {} is already in account", op.tx.id)
        }
        let old_nonce = recipient_account.nonce;
        recipient_account.add_balance(op.tx.id, &BigUint::from(1u32));

        self.insert_account(op.tx.account_id, token_account);

        updates.push((
            op.account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (op.tx.id, BigUint::zero(), BigUint::from(1u32)),
                old_nonce,
                new_nonce: old_nonce,
            },
        ));

        let fee = None;

        metrics::histogram!("state.mint_nft", start.elapsed());
        Ok((fee, updates))
    }
}
