use num::{BigUint, FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, ADDRESS_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BYTES, CONTENT_HASH_WIDTH,
    ETH_ADDRESS_BIT_WIDTH, LEGACY_CHUNK_BYTES, LEGACY_TOKEN_BIT_WIDTH, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::FromBytes;
use zksync_utils::BigUintSerdeWrapper;

use crate::{operations::error::FullExitOpError, AccountId, Address, FullExit, TokenId, H256};

/// FullExit operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullExitOp {
    pub priority_op: FullExit,
    /// None if withdraw was unsuccessful
    pub withdraw_amount: Option<BigUintSerdeWrapper>,
    pub creator_account_id: Option<AccountId>,
    pub creator_address: Option<Address>,
    pub serial_id: Option<u32>,
    pub content_hash: Option<H256>,
}

impl FullExitOp {
    pub const LEGACY_CHUNKS: usize = 6;
    pub const CHUNKS: usize = 11;
    pub const OP_CODE: u8 = 0x06;
    pub const WITHDRAW_DATA_PREFIX: [u8; 1] = [0];

    pub(crate) fn get_public_data(&self) -> Vec<u8> {
        let mut data = vec![Self::OP_CODE];
        data.extend_from_slice(&self.priority_op.account_id.to_be_bytes());
        data.extend_from_slice(self.priority_op.eth_address.as_bytes());
        data.extend_from_slice(&self.priority_op.token.to_be_bytes());
        data.extend_from_slice(
            &self
                .withdraw_amount
                .clone()
                .unwrap_or_default()
                .0
                .to_u128()
                .unwrap()
                .to_be_bytes(),
        );
        data.extend_from_slice(
            &self
                .creator_account_id
                .clone()
                .unwrap_or_default()
                .to_be_bytes(),
        );
        data.extend_from_slice(&self.creator_address.clone().unwrap_or_default().as_bytes());
        data.extend_from_slice(&self.serial_id.clone().unwrap_or_default().to_be_bytes());
        data.extend_from_slice(&self.content_hash.clone().unwrap_or_default().as_bytes());
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub(crate) fn get_withdrawal_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&Self::WITHDRAW_DATA_PREFIX); // first byte is a bool variable 'addToPendingWithdrawalsQueue'
        data.extend_from_slice(self.priority_op.eth_address.as_bytes());
        data.extend_from_slice(&self.priority_op.token.to_be_bytes());
        data.extend_from_slice(
            &self
                .withdraw_amount
                .clone()
                .map(|a| a.0.to_u128().unwrap())
                .unwrap_or(0)
                .to_be_bytes(),
        );
        data.extend_from_slice(
            &self
                .creator_account_id
                .clone()
                .unwrap_or_default()
                .to_be_bytes(),
        );
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, FullExitOpError> {
        if bytes.len() != Self::CHUNKS * CHUNK_BYTES {
            return Err(FullExitOpError::PubdataSizeMismatch);
        }

        let account_id_offset = 1;
        let eth_address_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let token_offset = eth_address_offset + ETH_ADDRESS_BIT_WIDTH / 8;
        let amount_offset = token_offset + TOKEN_BIT_WIDTH / 8;
        let creator_address = amount_offset + BALANCE_BIT_WIDTH / 8;
        let content_hash_offset = creator_address + ADDRESS_WIDTH / 8;

        let account_id = u32::from_bytes(&bytes[account_id_offset..eth_address_offset])
            .ok_or(FullExitOpError::CannotGetAccountId)?;
        let eth_address = Address::from_slice(&bytes[eth_address_offset..token_offset]);
        let token = u32::from_bytes(&bytes[token_offset..amount_offset])
            .ok_or(FullExitOpError::CannotGetTokenId)?;
        let amount = BigUint::from_u128(
            u128::from_bytes(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or(FullExitOpError::CannotGetAmount)?,
        )
        .unwrap();

        let creator_address = Address::from_slice(&bytes[creator_address..content_hash_offset]);

        let content_hash = H256::from_slice(
            &bytes[content_hash_offset..content_hash_offset + CONTENT_HASH_WIDTH / 8],
        );

        Ok(Self {
            priority_op: FullExit {
                account_id: AccountId(account_id),
                eth_address,
                token: TokenId(token),
            },
            withdraw_amount: Some(amount.into()),
            creator_address: Some(creator_address),
            creator_account_id: None, // Unknown from pub data
            serial_id: None,          // Unknown from pub data
            content_hash: Some(content_hash),
        })
    }

    pub fn from_legacy_public_data(bytes: &[u8]) -> Result<Self, FullExitOpError> {
        if bytes.len() != Self::CHUNKS * LEGACY_CHUNK_BYTES {
            return Err(FullExitOpError::PubdataSizeMismatch);
        }

        let account_id_offset = 1;
        let eth_address_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let token_offset = eth_address_offset + ETH_ADDRESS_BIT_WIDTH / 8;
        let amount_offset = token_offset + LEGACY_TOKEN_BIT_WIDTH / 8;

        let account_id = u32::from_bytes(&bytes[account_id_offset..eth_address_offset])
            .ok_or(FullExitOpError::CannotGetAccountId)?;
        let eth_address = Address::from_slice(&bytes[eth_address_offset..token_offset]);
        let token = u32::from_bytes(&bytes[token_offset..amount_offset])
            .ok_or(FullExitOpError::CannotGetTokenId)?;
        let amount = BigUint::from_u128(
            u128::from_bytes(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or(FullExitOpError::CannotGetAmount)?,
        )
        .unwrap();

        Ok(Self {
            priority_op: FullExit {
                account_id: AccountId(account_id),
                eth_address,
                token: TokenId(token),
            },
            withdraw_amount: Some(amount.into()),
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.priority_op.account_id]
    }
}
