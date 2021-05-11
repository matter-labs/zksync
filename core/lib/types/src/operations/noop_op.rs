use crate::operations::error::NoopOpError;
use serde::{Deserialize, Serialize};
use zksync_basic_types::AccountId;
use zksync_crypto::params::CHUNK_BYTES;

/// Noop operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoopOp {}

impl NoopOp {
    pub const CHUNKS: usize = 1;
    pub const OP_CODE: u8 = 0x00;

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, NoopOpError> {
        if bytes != [0; CHUNK_BYTES] {
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
