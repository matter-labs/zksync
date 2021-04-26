use crate::tx::WithdrawNFT;
use crate::{
    helpers::{pack_fee_amount, unpack_fee_amount},
    H256,
};
use crate::{AccountId, Address, Nonce, TokenId};
use anyhow::{ensure, format_err};

use serde::{Deserialize, Serialize};
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, ADDRESS_WIDTH, CHUNK_BYTES, CONTENT_HASH_WIDTH, ETH_ADDRESS_BIT_WIDTH,
    FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::FromBytes;

/// Withdraw operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawNFTOp {
    pub tx: WithdrawNFT,
    pub creator_id: AccountId,
    pub creator_address: Address,
    pub content_hash: H256,
    pub serial_id: u32,
}

impl WithdrawNFTOp {
    pub const CHUNKS: usize = 10;
    pub const OP_CODE: u8 = 0x0b;
    pub const WITHDRAW_DATA_PREFIX: [u8; 1] = [1];

    pub(crate) fn get_public_data(&self) -> Vec<u8> {
        let mut data = vec![Self::OP_CODE];
        data.extend_from_slice(&self.tx.account_id.to_be_bytes());
        data.extend_from_slice(&self.creator_address.as_bytes());
        data.extend_from_slice(&self.content_hash.as_bytes());
        data.extend_from_slice(self.tx.to.as_bytes());
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&self.tx.fee_token.to_be_bytes());
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub(crate) fn get_withdrawal_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&Self::WITHDRAW_DATA_PREFIX); // first byte is a bool variable 'addToPendingWithdrawalsQueue'
        data.extend_from_slice(self.tx.to.as_bytes());
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * CHUNK_BYTES,
            "Wrong bytes length for withdraw pubdata"
        );

        let account_offset = 1;
        let creator_account_offset = account_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let content_hash_offset = creator_account_offset + ADDRESS_WIDTH / 8;
        let eth_address_offset = content_hash_offset + CONTENT_HASH_WIDTH / 8;
        let token_id_offset = eth_address_offset + ADDRESS_WIDTH / 8;
        let token_fee_id_offset = token_id_offset + TOKEN_BIT_WIDTH / 8;
        let fee_offset = token_fee_id_offset + TOKEN_BIT_WIDTH / 8;

        let account_id =
            u32::from_bytes(&bytes[account_offset..account_offset + ACCOUNT_ID_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get account id from withdraw pubdata"))?;
        let creator_address = Address::from_slice(
            &bytes[creator_account_offset..creator_account_offset + ADDRESS_WIDTH / 8],
        );
        let content_hash = H256::from_slice(
            &bytes[content_hash_offset..content_hash_offset + CONTENT_HASH_WIDTH / 8],
        );
        let from = Address::zero(); // From pubdata it is unknown
        let token = u32::from_bytes(&bytes[token_id_offset..token_id_offset + TOKEN_BIT_WIDTH / 8])
            .ok_or_else(|| format_err!("Cant get token id from withdraw pubdata"))?;
        let token_fee =
            u32::from_bytes(&bytes[token_fee_id_offset..token_fee_id_offset + TOKEN_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get token id from withdraw pubdata"))?;
        let to = Address::from_slice(
            &bytes[eth_address_offset..eth_address_offset + ETH_ADDRESS_BIT_WIDTH / 8],
        );
        let fee = unpack_fee_amount(
            &bytes[fee_offset..fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or_else(|| format_err!("Cant get fee from withdraw pubdata"))?;
        let nonce = 0; // From pubdata it is unknown
        let time_range = Default::default();

        let creator_id = AccountId(0); //  From pubdata it is unknown
        Ok(Self {
            tx: WithdrawNFT::new(
                AccountId(account_id),
                from,
                to,
                TokenId(token),
                TokenId(token_fee),
                fee,
                Nonce(nonce),
                time_range,
                None,
            ),
            creator_id,
            creator_address,
            content_hash,
            serial_id: 0,
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.tx.account_id]
    }
}
#[cfg(test)]
mod tests {
    use crate::{AccountId, Address, Nonce, TokenId, WithdrawNFT, WithdrawNFTOp, H256};
    use num::BigUint;

    #[test]
    fn public_data() {
        let op = WithdrawNFTOp {
            tx: WithdrawNFT::new(
                AccountId(10),
                Address::random(),
                Address::random(),
                TokenId(10),
                TokenId(0),
                BigUint::from(10u32),
                Nonce(0),
                Default::default(),
                None,
            ),
            creator_id: AccountId(0),
            creator_address: Address::random(),
            content_hash: H256::random(),
            serial_id: 1,
        };
        let pub_data = op.get_public_data();
        let new_op = WithdrawNFTOp::from_public_data(&pub_data).unwrap();
        dbg!(&new_op);
        dbg!(&op);
        assert!(
            new_op.tx.account_id == op.tx.account_id
                && new_op.creator_address == op.creator_address
                && new_op.creator_id == AccountId(0)
                && new_op.content_hash == op.content_hash
                && new_op.tx.to == op.tx.to
                && new_op.tx.fee_token == op.tx.fee_token
                && new_op.tx.token == op.tx.token
                && new_op.tx.fee == op.tx.fee
        )
    }
}
