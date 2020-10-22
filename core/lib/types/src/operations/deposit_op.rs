use crate::AccountId;
use crate::Deposit;
use anyhow::{ensure, format_err};
use num::{BigUint, ToPrimitive};
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BYTES, FR_ADDRESS_LEN, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::FromBytes;

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
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.account_id.to_be_bytes());
        data.extend_from_slice(&self.priority_op.token.to_be_bytes());
        data.extend_from_slice(&self.priority_op.amount.to_u128().unwrap().to_be_bytes());
        data.extend_from_slice(&self.priority_op.to.as_bytes());
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * CHUNK_BYTES,
            "Wrong bytes length for deposit pubdata"
        );

        let account_id_offset = 1;
        let token_id_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_offset = token_id_offset + TOKEN_BIT_WIDTH / 8;
        let account_address_offset = amount_offset + BALANCE_BIT_WIDTH / 8;

        let account_id = u32::from_bytes(
            &bytes[account_id_offset..account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or_else(|| format_err!("Cant get account id from deposit pubdata"))?;
        let token = u16::from_bytes(&bytes[token_id_offset..token_id_offset + TOKEN_BIT_WIDTH / 8])
            .ok_or_else(|| format_err!("Cant get token id from deposit pubdata"))?;
        let amount = BigUint::from(
            u128::from_bytes(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get amount from deposit pubdata"))?,
        );
        let to = Address::from_slice(
            &bytes[account_address_offset..account_address_offset + FR_ADDRESS_LEN],
        );

        let from = Address::default(); // unknown from pubdata.

        Ok(Self {
            priority_op: Deposit {
                from,
                token,
                amount,
                to,
            },
            account_id,
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.account_id]
    }
}
