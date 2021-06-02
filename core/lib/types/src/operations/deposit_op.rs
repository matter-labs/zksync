use crate::{operations::error::DepositOpError, AccountId, Address, Deposit, TokenId};
use num::{BigUint, ToPrimitive};
use serde::{Deserialize, Serialize};
use zksync_crypto::{
    params::{
        ACCOUNT_ID_BIT_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BYTES, FR_ADDRESS_LEN, LEGACY_CHUNK_BYTES,
        LEGACY_TOKEN_BIT_WIDTH, TOKEN_BIT_WIDTH,
    },
    primitives::FromBytes,
};

/// Deposit operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositOp {
    pub priority_op: Deposit,
    pub account_id: AccountId,
}

impl DepositOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x01;

    pub fn get_public_data(&self) -> Vec<u8> {
        let mut data = vec![Self::OP_CODE];
        data.extend_from_slice(&self.account_id.to_be_bytes());
        data.extend_from_slice(&self.priority_op.token.to_be_bytes());
        data.extend_from_slice(&self.priority_op.amount.to_u128().unwrap().to_be_bytes());
        data.extend_from_slice(&self.priority_op.to.as_bytes());
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, DepositOpError> {
        Self::parse_pub_data(bytes, TOKEN_BIT_WIDTH, CHUNK_BYTES)
    }

    pub fn from_legacy_public_data(bytes: &[u8]) -> Result<Self, DepositOpError> {
        Self::parse_pub_data(bytes, LEGACY_TOKEN_BIT_WIDTH, LEGACY_CHUNK_BYTES)
    }

    fn parse_pub_data(
        bytes: &[u8],
        token_bit_width: usize,
        chunk_bytes: usize,
    ) -> Result<Self, DepositOpError> {
        if bytes.len() != Self::CHUNKS * chunk_bytes {
            return Err(DepositOpError::PubdataSizeMismatch);
        }

        let account_id_offset = 1;
        let token_id_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_offset = token_id_offset + token_bit_width / 8;
        let account_address_offset = amount_offset + BALANCE_BIT_WIDTH / 8;

        let account_id = u32::from_bytes(
            &bytes[account_id_offset..account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or(DepositOpError::CannotGetAccountId)?;
        let token = u32::from_bytes(&bytes[token_id_offset..token_id_offset + token_bit_width / 8])
            .ok_or(DepositOpError::CannotGetTokenId)?;
        let amount = BigUint::from(
            u128::from_bytes(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or(DepositOpError::CannotGetAmount)?,
        );
        let to = Address::from_slice(
            &bytes[account_address_offset..account_address_offset + FR_ADDRESS_LEN],
        );

        let from = Address::default(); // unknown from pubdata.

        Ok(Self {
            priority_op: Deposit {
                from,
                token: TokenId(token),
                amount,
                to,
            },
            account_id: AccountId(account_id),
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.account_id]
    }
}
