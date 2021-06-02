use crate::operations::error::NoopOpError;
use serde::{Deserialize, Serialize};
use zksync_basic_types::AccountId;
use zksync_crypto::params::{CHUNK_BYTES, LEGACY_CHUNK_BYTES};

/// Noop operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoopOp {}

impl NoopOp {
    pub const CHUNKS: usize = 1;
    pub const OP_CODE: u8 = 0x00;

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, NoopOpError> {
        Self::parse_pub_data::<CHUNK_BYTES>(bytes)
    }

    pub fn from_legacy_public_data(bytes: &[u8]) -> Result<Self, NoopOpError> {
        Self::parse_pub_data::<LEGACY_CHUNK_BYTES>(bytes)
    }

    fn parse_pub_data<const BYTES: usize>(bytes: &[u8]) -> Result<Self, NoopOpError> {
        if bytes != [0; BYTES] {
            return Err(NoopOpError::IncorrectPubdata);
        }
        Ok(Self {})
    }

    pub(crate) fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub(crate) fn get_updated_account_ids(&self) -> Vec<AccountId> {
        Vec::new()
    }
}
