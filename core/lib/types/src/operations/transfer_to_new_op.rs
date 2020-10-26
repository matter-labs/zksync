use crate::AccountId;
use crate::{
    helpers::{pack_fee_amount, pack_token_amount, unpack_fee_amount, unpack_token_amount},
    Transfer,
};
use anyhow::{ensure, format_err};
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, CHUNK_BYTES,
    FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, FR_ADDRESS_LEN, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::FromBytes;

/// TransferToNew operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferToNewOp {
    pub tx: Transfer,
    pub from: AccountId,
    pub to: AccountId,
}

impl TransferToNewOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x02;

    pub(crate) fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.from.to_be_bytes());
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&pack_token_amount(&self.tx.amount));
        data.extend_from_slice(&self.tx.to.as_bytes());
        data.extend_from_slice(&self.to.to_be_bytes());
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * CHUNK_BYTES,
            "Wrong bytes length for transfer to new pubdata"
        );

        let from_offset = 1;
        let token_id_offset = from_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_offset = token_id_offset + TOKEN_BIT_WIDTH / 8;
        let to_address_offset =
            amount_offset + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8;
        let to_id_offset = to_address_offset + FR_ADDRESS_LEN;
        let fee_offset = to_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;

        let from_id = u32::from_bytes(&bytes[from_offset..from_offset + ACCOUNT_ID_BIT_WIDTH / 8])
            .ok_or_else(|| format_err!("Cant get from account id from transfer to new pubdata"))?;
        let to_id = u32::from_bytes(&bytes[to_id_offset..to_id_offset + ACCOUNT_ID_BIT_WIDTH / 8])
            .ok_or_else(|| format_err!("Cant get to account id from transfer to new pubdata"))?;
        let from = Address::zero(); // It is unknown from pubdata;
        let to = Address::from_slice(&bytes[to_address_offset..to_address_offset + FR_ADDRESS_LEN]);
        let token = u16::from_bytes(&bytes[token_id_offset..token_id_offset + TOKEN_BIT_WIDTH / 8])
            .ok_or_else(|| format_err!("Cant get token id from transfer to new pubdata"))?;
        let amount = unpack_token_amount(
            &bytes[amount_offset
                ..amount_offset + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or_else(|| format_err!("Cant get amount from transfer to new pubdata"))?;
        let fee = unpack_fee_amount(
            &bytes[fee_offset..fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or_else(|| format_err!("Cant get fee from transfer to new pubdata"))?;
        let nonce = 0; // It is unknown from pubdata

        Ok(Self {
            tx: Transfer::new(from_id, from, to, token, amount, fee, nonce, None),
            from: from_id,
            to: to_id,
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.from, self.to]
    }
}
