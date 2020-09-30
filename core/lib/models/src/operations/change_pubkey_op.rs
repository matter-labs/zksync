use crate::tx::ChangePubKey;
use crate::AccountId;
use crate::PubKeyHash;
use failure::{ensure, format_err};
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, ADDRESS_WIDTH, CHUNK_BYTES, NEW_PUBKEY_HASH_WIDTH, NONCE_BIT_WIDTH,
};
use zksync_crypto::primitives::bytes_slice_to_uint32;

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

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        let mut offset = 1;

        let mut len = ACCOUNT_ID_BIT_WIDTH / 8;
        ensure!(
            bytes.len() >= offset + len,
            "Change pubkey offchain, pubdata too short"
        );
        let account_id = bytes_slice_to_uint32(&bytes[offset..offset + len])
            .ok_or_else(|| format_err!("Change pubkey offchain, fail to get account id"))?;
        offset += len;

        len = NEW_PUBKEY_HASH_WIDTH / 8;
        ensure!(
            bytes.len() >= offset + len,
            "Change pubkey offchain, pubdata too short"
        );
        let new_pk_hash = PubKeyHash::from_bytes(&bytes[offset..offset + len])?;
        offset += len;

        len = ADDRESS_WIDTH / 8;
        ensure!(
            bytes.len() >= offset + len,
            "Change pubkey offchain, pubdata too short"
        );
        let account = Address::from_slice(&bytes[offset..offset + len]);
        offset += len;

        len = NONCE_BIT_WIDTH / 8;
        ensure!(
            bytes.len() >= offset + len,
            "Change pubkey offchain, pubdata too short"
        );
        let nonce = bytes_slice_to_uint32(&bytes[offset..offset + len])
            .ok_or_else(|| format_err!("Change pubkey offchain, fail to get nonce"))?;

        Ok(ChangePubKeyOp {
            tx: ChangePubKey {
                account_id,
                account,
                new_pk_hash,
                nonce,
                eth_signature: None,
            },
            account_id,
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        let mut result = Vec::with_capacity(1);
        result.push(self.account_id);
        result
    }
}
