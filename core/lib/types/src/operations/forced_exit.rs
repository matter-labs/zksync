use crate::AccountId;
use crate::{
    helpers::{pack_fee_amount, unpack_fee_amount},
    ForcedExit,
};
use anyhow::{ensure, format_err};
use num::{BigUint, FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BYTES, ETH_ADDRESS_BIT_WIDTH,
    FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::FromBytes;
use zksync_utils::BigUintSerdeWrapper;

/// ForcedExit operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForcedExitOp {
    pub tx: ForcedExit,
    /// Account ID of the account to which ForcedExit is applied.
    pub target_account_id: AccountId,
    /// None if withdraw was unsuccessful
    pub withdraw_amount: Option<BigUintSerdeWrapper>,
}

impl ForcedExitOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x08;
    pub const WITHDRAW_DATA_PREFIX: [u8; 1] = [1];

    pub fn amount(&self) -> u128 {
        self.withdraw_amount
            .clone()
            .map(|a| a.0.to_u128().unwrap())
            .unwrap_or(0)
    }

    pub fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.tx.initiator_account_id.to_be_bytes());
        data.extend_from_slice(&self.target_account_id.to_be_bytes());
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&self.amount().to_be_bytes());
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.extend_from_slice(self.tx.target.as_bytes());
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub fn get_withdrawal_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&Self::WITHDRAW_DATA_PREFIX); // first byte is a bool variable 'addToPendingWithdrawalsQueue'
        data.extend_from_slice(self.tx.target.as_bytes());
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&self.amount().to_be_bytes());
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * CHUNK_BYTES,
            "Wrong bytes length for forced exit pubdata"
        );

        let initiator_account_id_offset = 1;
        let target_account_id_offset = initiator_account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let token_id_offset = target_account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_offset = token_id_offset + TOKEN_BIT_WIDTH / 8;
        let fee_offset = amount_offset + BALANCE_BIT_WIDTH / 8;
        let eth_address_offset = fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8;
        let eth_address_end = eth_address_offset + ETH_ADDRESS_BIT_WIDTH / 8;

        let initiator_account_id =
            u32::from_bytes(&bytes[initiator_account_id_offset..target_account_id_offset])
                .ok_or_else(|| {
                    format_err!("Cant get initiator account id from forced exit pubdata")
                })?;
        let target_account_id = u32::from_bytes(&bytes[target_account_id_offset..token_id_offset])
            .ok_or_else(|| format_err!("Cant get target account id from forced exit pubdata"))?;
        let token = u16::from_bytes(&bytes[token_id_offset..amount_offset])
            .ok_or_else(|| format_err!("Cant get token id from forced exit pubdata"))?;
        let amount = BigUint::from_u128(
            u128::from_bytes(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get amount from forced exit pubdata"))?,
        )
        .unwrap();
        let fee = unpack_fee_amount(&bytes[fee_offset..eth_address_offset])
            .ok_or_else(|| format_err!("Cant get fee from withdraw pubdata"))?;
        let target = Address::from_slice(&bytes[eth_address_offset..eth_address_end]);

        let nonce = 0; // From pubdata it is unknown

        Ok(Self {
            tx: ForcedExit::new(initiator_account_id, target, token, fee, nonce, None),
            target_account_id,
            withdraw_amount: Some(amount.into()),
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.target_account_id, self.tx.initiator_account_id]
    }
}
