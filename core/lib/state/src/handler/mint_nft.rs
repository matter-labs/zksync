use anyhow::{bail, ensure, format_err};
use std::time::Instant;
use zksync_crypto::params;
use zksync_types::{
    operations::MintNFTOp, Account, AccountUpdate, AccountUpdates, Address, MintNFT, Token,
    TokenId, ZkSyncOp,
};

use crate::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, ZkSyncState},
};
use num::{BigUint, ToPrimitive, Zero};
use zksync_types::tokens::NFT;

pub const NFT_TOKEN_ID: TokenId = TokenId(2 ^ 32 - 1);

impl TxHandler<MintNFT> for ZkSyncState {
    type Op = MintNFTOp;

    fn create_op(&self, tx: MintNFT) -> Result<Self::Op, anyhow::Error> {
        ensure!(
            tx.fee_token <= params::max_token_id(),
            "Token id is not supported"
        );
        ensure!(
            tx.recipient != Address::zero(),
            "Transfer to Account with address 0 is not allowed"
        );
        let (recipient, _) = self
            .get_account_by_address(&tx.recipient)
            .ok_or_else(|| format_err!("Recipient account does not exist"))?;
        // TODO support minting to new
        let token_account_id = self.get_free_account_id();
        let op = MintNFTOp {
            creator_account_id: tx.creator_id,
            recipient_account_id: recipient,
            token_account_id,
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

        let token_id = TokenId(op.token_account_id.0);

        let mut creator_account = self
            .get_account(op.creator_account_id)
            .ok_or(format_err!("Recipient account not found"))?;

        let mut recipient_account = self
            .get_account(op.recipient_account_id)
            .ok_or(format_err!("Recipient account not found"))?;

        let old_balance = creator_account.get_balance(NFT_TOKEN_ID);

        let old_nonce = creator_account.nonce;
        creator_account.add_balance(NFT_TOKEN_ID, &BigUint::from(1u32));
        *creator_account.nonce += 1;

        let new_balance = creator_account.get_balance(NFT_TOKEN_ID);

        updates.push((
            op.creator_account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (NFT_TOKEN_ID, old_balance, new_balance.clone()),
                old_nonce,
                new_nonce: creator_account.nonce,
            },
        ));

        let serial_id = new_balance.to_u32().unwrap_or_default();
        let token_address = op.tx.calculate_address(serial_id);

        let token_account = if self.get_account(op.token_account_id).is_none() {
            let (account, upd) = Account::create_account(op.token_account_id, token_address);
            updates.extend(upd.into_iter());
            account
        } else {
            bail!("Token account is already exists");
        };

        updates.push((
            op.token_account_id,
            AccountUpdate::MintNFT {
                token: NFT::new(
                    token_id,
                    op.token_account_id,
                    serial_id,
                    op.tx.creator_id,
                    token_address,
                    None,
                    op.tx.content_hash,
                )?,
            },
        ));

        let old_amount = recipient_account.get_balance(token_id);
        if old_amount != BigUint::zero() {
            bail!("Token {} is already in account", token_id)
        }
        let old_nonce = recipient_account.nonce;
        recipient_account.add_balance(token_id, &BigUint::from(1u32));

        self.insert_account(op.token_account_id, token_account);

        updates.push((
            op.recipient_account_id,
            AccountUpdate::UpdateBalance {
                balance_update: (token_id, BigUint::zero(), BigUint::from(1u32)),
                old_nonce,
                new_nonce: old_nonce,
            },
        ));

        let fee = CollectedFee {
            token: op.tx.fee_token,
            amount: op.tx.fee.clone(),
        };

        metrics::histogram!("state.mint_nft", start.elapsed());
        Ok((Some(fee), updates))
    }
}
