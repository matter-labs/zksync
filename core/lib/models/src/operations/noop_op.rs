use failure::ensure;
use serde::{Deserialize, Serialize};
use zksync_crypto::params::CHUNK_BYTES;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoopOp {}

impl NoopOp {
    pub const CHUNKS: usize = 1;
    pub const OP_CODE: u8 = 0x00;

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(
            bytes == [0; CHUNK_BYTES],
            format!("Wrong pubdata for noop operation {:?}", bytes)
        );
        Ok(Self {})
    }

    pub(crate) fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }
}
