use crate::helpers::{pack_fee_amount, unpack_fee_amount};
use crate::tx::ChangePubKey;
use crate::AccountId;
use crate::PubKeyHash;
use anyhow::{ensure, format_err};
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, ADDRESS_WIDTH, CHUNK_BYTES, FEE_EXPONENT_BIT_WIDTH,
    FEE_MANTISSA_BIT_WIDTH, NEW_PUBKEY_HASH_WIDTH, NONCE_BIT_WIDTH, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::FromBytes;

/// ChangePubKey operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePubKeyOp {
    pub tx: ChangePubKey,
    pub account_id: AccountId,
}

impl ChangePubKeyOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x07;

    pub fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.account_id.to_be_bytes());
        data.extend_from_slice(&self.tx.new_pk_hash.data);
        data.extend_from_slice(&self.tx.account.as_bytes());
        data.extend_from_slice(&self.tx.nonce.to_be_bytes());
        data.extend_from_slice(&self.tx.fee_token.to_be_bytes());
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub fn get_eth_witness(&self) -> Vec<u8> {
        if let Some(eth_signature) = &self.tx.eth_signature {
            eth_signature.serialize_packed().to_vec()
        } else {
            Vec::new()
        }
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        let account_id_offset = 1;
        let pk_hash_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let account_offset = pk_hash_offset + NEW_PUBKEY_HASH_WIDTH / 8;
        let nonce_offset = account_offset + ADDRESS_WIDTH / 8;
        let fee_token_offset = nonce_offset + NONCE_BIT_WIDTH / 8;
        let fee_offset = fee_token_offset + TOKEN_BIT_WIDTH / 8;
        let end = fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8;

        ensure!(
            bytes.len() >= end,
            "Change pubkey offchain, pubdata too short"
        );

        let account_id = u32::from_bytes(&bytes[account_id_offset..pk_hash_offset])
            .ok_or_else(|| format_err!("Change pubkey offchain, fail to get account id"))?;
        let new_pk_hash = PubKeyHash::from_bytes(&bytes[pk_hash_offset..account_offset])?;
        let account = Address::from_slice(&bytes[account_offset..nonce_offset]);
        let nonce = u32::from_bytes(&bytes[nonce_offset..fee_token_offset])
            .ok_or_else(|| format_err!("Change pubkey offchain, fail to get nonce"))?;
        let fee_token = u16::from_bytes(&bytes[fee_token_offset..fee_offset])
            .ok_or_else(|| format_err!("Change pubkey offchain, fail to get fee token ID"))?;
        let fee = unpack_fee_amount(&bytes[fee_offset..end])
            .ok_or_else(|| format_err!("Change pubkey offchain, fail to get fee"))?;

        Ok(ChangePubKeyOp {
            tx: ChangePubKey::new(
                account_id,
                account,
                new_pk_hash,
                fee_token,
                fee,
                nonce,
                None,
                None,
            ),
            account_id,
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.account_id]
    }
}
