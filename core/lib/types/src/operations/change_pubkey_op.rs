use crate::{
    helpers::{pack_fee_amount, unpack_fee_amount},
    operations::error::ChangePubkeyOpError,
    tx::ChangePubKey,
    AccountId, Address, Nonce, PubKeyHash, TokenId,
};
use serde::{Deserialize, Serialize};
use zksync_crypto::{
    params::{
        ACCOUNT_ID_BIT_WIDTH, ADDRESS_WIDTH, CHUNK_BYTES, FEE_EXPONENT_BIT_WIDTH,
        FEE_MANTISSA_BIT_WIDTH, LEGACY_TOKEN_BIT_WIDTH, NEW_PUBKEY_HASH_WIDTH, NONCE_BIT_WIDTH,
        TOKEN_BIT_WIDTH,
    },
    primitives::FromBytes,
};

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
        let mut data = vec![Self::OP_CODE];
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
        if let Some(eth_auth_data) = &self.tx.eth_auth_data {
            eth_auth_data.get_eth_witness()
        } else if let Some(eth_signature) = &self.tx.eth_signature {
            let mut bytes = vec![0x02];
            bytes.extend_from_slice(&eth_signature.serialize_packed());
            bytes
        } else {
            Vec::new()
        }
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, ChangePubkeyOpError> {
        Self::parse_pub_data(bytes, TOKEN_BIT_WIDTH)
    }

    pub fn from_legacy_public_data(bytes: &[u8]) -> Result<Self, ChangePubkeyOpError> {
        Self::parse_pub_data(bytes, LEGACY_TOKEN_BIT_WIDTH)
    }

    fn parse_pub_data(bytes: &[u8], token_bit_width: usize) -> Result<Self, ChangePubkeyOpError> {
        let account_id_offset = 1;
        let pk_hash_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let account_offset = pk_hash_offset + NEW_PUBKEY_HASH_WIDTH / 8;
        let nonce_offset = account_offset + ADDRESS_WIDTH / 8;
        let fee_token_offset = nonce_offset + NONCE_BIT_WIDTH / 8;
        let fee_offset = fee_token_offset + token_bit_width / 8;
        let end = fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8;

        if bytes.len() < end {
            return Err(ChangePubkeyOpError::PubdataSizeMismatch);
        }

        let account_id = u32::from_bytes(&bytes[account_id_offset..pk_hash_offset])
            .ok_or(ChangePubkeyOpError::CannotGetAccountId)?;
        let new_pk_hash = PubKeyHash::from_bytes(&bytes[pk_hash_offset..account_offset])?;
        let account = Address::from_slice(&bytes[account_offset..nonce_offset]);
        let nonce = u32::from_bytes(&bytes[nonce_offset..fee_token_offset])
            .ok_or(ChangePubkeyOpError::CannotGetNonce)?;
        let fee_token = u32::from_bytes(&bytes[fee_token_offset..fee_offset])
            .ok_or(ChangePubkeyOpError::CannotGetFeeTokenId)?;
        let fee =
            unpack_fee_amount(&bytes[fee_offset..end]).ok_or(ChangePubkeyOpError::CannotGetFee)?;

        Ok(ChangePubKeyOp {
            tx: ChangePubKey::new(
                AccountId(account_id),
                account,
                new_pk_hash,
                TokenId(fee_token),
                fee,
                Nonce(nonce),
                Default::default(),
                None,
                None,
            ),
            account_id: AccountId(account_id),
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![self.account_id]
    }
}
