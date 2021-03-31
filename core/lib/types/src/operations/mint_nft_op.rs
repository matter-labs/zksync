use crate::{AccountId, Address, TokenId};
use crate::{MintNFT, H256};
use num::{BigUint, ToPrimitive};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, ADDRESS_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BYTES, CONTENT_HASH_WIDTH,
    FR_ADDRESS_LEN, SERIAL_ID_BIT_WIDTH, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::FromBytes;

#[derive(Error, Debug)]
pub enum MintNFTParsingError {
    #[error("Wrong number of types")]
    WrongNumberOfBytes,
    #[error("Cannot parse creator account id")]
    CreatorAccountId,
    #[error("Cannot parse token id")]
    TokenId,
    #[error("Cannot parse token account id")]
    AccountId,
    #[error("Cannot parse serial id")]
    SerialId,
    #[error("Cannot parse recipient account id")]
    RecipientAccountId,
}

/// Deposit operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintNFTOp {
    pub priority_op: MintNFT,
    pub account_id: AccountId,
}

impl MintNFTOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x09;

    pub fn get_public_data(&self) -> Vec<u8> {
        let mut data = vec![Self::OP_CODE];
        data.extend_from_slice(&self.account_id.to_be_bytes()); // Creator account id
        data.extend_from_slice(&self.priority_op.id.to_be_bytes());
        data.extend_from_slice(&self.priority_op.account_id.to_be_bytes());
        data.extend_from_slice(&self.priority_op.serial_id.to_be_bytes());
        data.extend_from_slice(&self.priority_op.address.as_bytes());
        data.extend_from_slice(&self.priority_op.content_hash.as_bytes());
        data.extend_from_slice(&self.priority_op.recipient_account_id.to_be_bytes());
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, MintNFTParsingError> {
        if bytes.len() != Self::CHUNKS * CHUNK_BYTES {
            return Err(MintNFTParsingError::WrongNumberOfBytes);
        }

        let account_id_offset = 1;
        let token_id_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let token_account_id_offset = token_id_offset + TOKEN_BIT_WIDTH / 8;
        let serial_id_offset = token_account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let address_offset = serial_id_offset + SERIAL_ID_BIT_WIDTH / 8;
        let content_hash_offset = address_offset + ADDRESS_WIDTH / 8;
        let recipient_account_id_offset = content_hash_offset + CONTENT_HASH_WIDTH / 8;

        let creator_account_id = u32::from_bytes(
            &bytes[account_id_offset..account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or(MintNFTParsingError::CreatorAccountId)?;

        let token_id =
            u32::from_bytes(&bytes[token_id_offset..token_id_offset + TOKEN_BIT_WIDTH / 8])
                .ok_or(MintNFTParsingError::TokenId)?;
        let token_account_id = u32::from_bytes(
            &bytes[token_account_id_offset..token_account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or(MintNFTParsingError::AccountId)?;
        let serial_id =
            u32::from_bytes(&bytes[serial_id_offset..serial_id_offset + SERIAL_ID_BIT_WIDTH / 8])
                .ok_or(MintNFTParsingError::SerialId)?;

        let token_address =
            Address::from_slice(&bytes[address_offset..address_offset + ADDRESS_WIDTH / 8]);

        let content_hash = H256::from_slice(
            &bytes[content_hash_offset..content_hash_offset + CONTENT_HASH_WIDTH / 8],
        );
        let recipient_account_id = u32::from_bytes(
            &bytes[recipient_account_id_offset
                ..recipient_account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or(MintNFTParsingError::RecipientAccountId)?;

        Ok(Self {
            priority_op: MintNFT {
                id: TokenId(token_id),
                account_id: AccountId(token_account_id),
                serial_id,
                creator_id: AccountId(creator_account_id),
                address: token_address,
                content_hash,
                recipient_account_id: AccountId(recipient_account_id),
            },
            account_id: AccountId(creator_account_id),
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![
            self.account_id,
            self.priority_op.account_id,
            self.priority_op.recipient_account_id,
        ]
    }
}
