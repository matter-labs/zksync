use crate::FullExit;
use anyhow::{ensure, format_err};
use num::{BigUint, FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use zksync_basic_types::{AccountId, Address};
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BYTES, ETH_ADDRESS_BIT_WIDTH, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::FromBytes;
use zksync_utils::BigUintSerdeWrapper;

/// FullExit operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullExitOp {
    pub priority_op: FullExit,
    /// None if withdraw was unsuccessful
    pub withdraw_amount: Option<BigUintSerdeWrapper>,
}

impl FullExitOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x06;
    pub const WITHDRAW_DATA_PREFIX: [u8; 1] = [0];

    pub(crate) fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
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
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * CHUNK_BYTES,
            "Wrong bytes length for full exit pubdata"
        );

        let account_id_offset = 1;
        let eth_address_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let token_offset = eth_address_offset + ETH_ADDRESS_BIT_WIDTH / 8;
        let amount_offset = token_offset + TOKEN_BIT_WIDTH / 8;

        let account_id = u32::from_bytes(&bytes[account_id_offset..eth_address_offset])
            .ok_or_else(|| format_err!("Cant get account id from full exit pubdata"))?;
        let eth_address = Address::from_slice(&bytes[eth_address_offset..token_offset]);
        let token = u16::from_bytes(&bytes[token_offset..amount_offset])
            .ok_or_else(|| format_err!("Cant get token id from full exit pubdata"))?;
        let amount = BigUint::from_u128(
            u128::from_bytes(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get amount from full exit pubdata"))?,
        )
        .unwrap();

        Ok(Self {
            priority_op: FullExit {
                account_id,
                eth_address,
                token,
            },
            withdraw_amount: Some(amount.into()),
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.priority_op.account_id]
    }
}
