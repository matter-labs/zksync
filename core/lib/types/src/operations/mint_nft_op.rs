use serde::{Deserialize, Serialize};

use zksync_crypto::{
    params::{
        ACCOUNT_ID_BIT_WIDTH, CHUNK_BYTES, CONTENT_HASH_WIDTH, FEE_EXPONENT_BIT_WIDTH,
        FEE_MANTISSA_BIT_WIDTH, NFT_STORAGE_ACCOUNT_ID, TOKEN_BIT_WIDTH,
    },
    primitives::FromBytes,
};

use crate::helpers::{pack_fee_amount, unpack_fee_amount};
use crate::operations::error::MintNFTOpError;
use crate::{AccountId, Address, MintNFT, Nonce, TokenId, H256};

/// Deposit operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintNFTOp {
    pub tx: MintNFT,
    pub creator_account_id: AccountId,
    pub recipient_account_id: AccountId,
}

impl MintNFTOp {
    pub const CHUNKS: usize = 5;
    pub const OP_CODE: u8 = 0x09;

    pub fn get_public_data(&self) -> Vec<u8> {
        let mut data = vec![Self::OP_CODE];
        data.extend_from_slice(&self.creator_account_id.to_be_bytes());
        data.extend_from_slice(&self.recipient_account_id.to_be_bytes());
        data.extend_from_slice(&self.tx.content_hash.as_bytes());
        data.extend_from_slice(&self.tx.fee_token.to_be_bytes());
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, MintNFTOpError> {
        if bytes.len() != Self::CHUNKS * CHUNK_BYTES {
            return Err(MintNFTOpError::WrongNumberOfBytes);
        }

        let creator_account_id_offset = 1;
        let recipient_account_id_offset = creator_account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let content_hash_offset = recipient_account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let fee_token_offset = content_hash_offset + CONTENT_HASH_WIDTH / 8;
        let fee_offset = fee_token_offset + TOKEN_BIT_WIDTH / 8;

        let creator_account_id = u32::from_bytes(
            &bytes[creator_account_id_offset..creator_account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or(MintNFTOpError::CreatorAccountId)?;

        let recipient_account_id = u32::from_bytes(
            &bytes[recipient_account_id_offset
                ..recipient_account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or(MintNFTOpError::RecipientAccountId)?;

        let creator_address = Address::default(); // Unknown from pubdata

        let content_hash = H256::from_slice(
            &bytes[content_hash_offset..content_hash_offset + CONTENT_HASH_WIDTH / 8],
        );

        let recipient_address = Address::default(); // Unknown from pubdata

        let fee_token_id =
            u32::from_bytes(&bytes[fee_token_offset..fee_token_offset + TOKEN_BIT_WIDTH / 8])
                .ok_or(MintNFTOpError::FeeTokenId)?;

        let fee = unpack_fee_amount(
            &bytes[fee_offset..fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or(MintNFTOpError::Fee)?;

        let nonce = 0; // It is unknown from pubdata

        Ok(Self {
            tx: MintNFT::new(
                AccountId(creator_account_id),
                creator_address,
                content_hash,
                recipient_address,
                fee,
                TokenId(fee_token_id),
                Nonce(nonce),
                None,
            ),
            creator_account_id: AccountId(creator_account_id),
            recipient_account_id: AccountId(recipient_account_id),
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![
            self.recipient_account_id,
            self.creator_account_id,
            NFT_STORAGE_ACCOUNT_ID,
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::{AccountId, Address, MintNFT, MintNFTOp, Nonce, TokenId, H256};
    use num::BigUint;

    #[test]
    fn public_data() {
        let op = MintNFTOp {
            tx: MintNFT::new(
                AccountId(10),
                Address::random(),
                H256::random(),
                Address::random(),
                BigUint::from(10u32),
                TokenId(0),
                Nonce(0),
                None,
            ),
            creator_account_id: AccountId(10),
            recipient_account_id: AccountId(11),
        };
        let pub_data = op.get_public_data();
        let new_op = MintNFTOp::from_public_data(&pub_data).unwrap();
        assert!(
            new_op.creator_account_id == op.creator_account_id
                && new_op.recipient_account_id == op.recipient_account_id
                && new_op.tx.content_hash == op.tx.content_hash
                && new_op.tx.fee == op.tx.fee
                && new_op.tx.fee_token == op.tx.fee_token
                && new_op.tx.creator_address == Default::default()
                && new_op.tx.recipient == Default::default()
                && new_op.tx.creator_id == op.tx.creator_id
        )
    }
}
