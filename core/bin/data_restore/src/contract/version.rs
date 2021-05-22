use std::convert::TryFrom;

use crate::{contract, rollup_ops::RollupOpsBlock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZkSyncContractVersion {
    V0,
    V1,
    V2,
    V3,
    V4,
}

impl TryFrom<u32> for ZkSyncContractVersion {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        use ZkSyncContractVersion::*;

        match value {
            0 => Ok(V0),
            1 => Ok(V1),
            2 => Ok(V2),
            3 => Ok(V3),
            4 => Ok(V4),
            _ => Err(anyhow::anyhow!("Unsupported contract version")),
        }
    }
}
impl From<ZkSyncContractVersion> for i32 {
    fn from(val: ZkSyncContractVersion) -> Self {
        match val {
            ZkSyncContractVersion::V0 => 0,
            ZkSyncContractVersion::V1 => 1,
            ZkSyncContractVersion::V2 => 2,
            ZkSyncContractVersion::V3 => 3,
            ZkSyncContractVersion::V4 => 4,
        }
    }
}

impl ZkSyncContractVersion {
    pub fn rollup_ops_blocks_from_bytes(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<RollupOpsBlock>> {
        use ZkSyncContractVersion::*;
        let mut blocks = match self {
            V0 | V1 | V2 | V3 => vec![contract::default::rollup_ops_blocks_from_bytes(data)?],
            V4 => contract::v4::rollup_ops_blocks_from_bytes(data)?,
        };
        // Set the contract version.
        for block in blocks.iter_mut() {
            block.contract_version = Some(*self);
        }
        Ok(blocks)
    }

    /// Returns the contract version incremented by `num`.
    ///
    /// # Arguments
    ///
    /// * `num` - how many times to upgrade.
    ///
    /// # Panics
    ///
    /// Panics if the the result is greater than the latest supported version.
    pub fn upgrade(&self, num: u32) -> Self {
        Self::try_from(i32::from(*self) as u32 + num)
            .expect("cannot upgrade past the latest contract version")
    }

    /// Returns supported block chunks sizes by the verifier contract
    /// with the given version.
    pub fn available_block_chunk_sizes(&self) -> &'static [usize] {
        use ZkSyncContractVersion::*;
        match self {
            V0 | V1 | V2 => &[6, 30, 74, 150, 334, 678],
            V3 => &[6, 30, 74, 150, 320, 630],
            V4 => &[10, 32, 72, 156, 322, 654],
        }
    }
}
