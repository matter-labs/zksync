// Built-in uses
use std::convert::TryFrom;
// External uses
// Workspace uses
use zksync_types::operations::ZkSyncOp;
// Local uses
use super::default;
use crate::{contract, rollup_ops::RollupOpsBlock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZkSyncContractVersion {
    V0,
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
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
            5 => Ok(V5),
            6 => Ok(V6),
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
            ZkSyncContractVersion::V5 => 5,
            ZkSyncContractVersion::V6 => 6,
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
            V4 | V5 => contract::v4::rollup_ops_blocks_from_bytes(data)?,
            V6 => contract::v6::rollup_ops_blocks_from_bytes(data)?,
        };
        // Set the contract version.
        for block in blocks.iter_mut() {
            block.contract_version = Some(*self);
        }
        Ok(blocks)
    }

    /// Attempts to restore block operations from the public data
    /// committed on the Ethereum smart contract.
    ///
    /// # Arguments
    ///
    /// * `data` - public data for block operations
    ///
    pub fn get_rollup_ops_from_data(&self, data: &[u8]) -> Result<Vec<ZkSyncOp>, anyhow::Error> {
        use ZkSyncContractVersion::*;
        match self {
            V0 | V1 | V2 | V3 | V4 | V5 => default::get_rollup_ops_from_data(data),
            V6 => contract::v6::get_rollup_ops_from_data(data),
        }
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
            V5 => &[18, 58, 136, 296, 612],
            V6 => &[26, 78, 182, 390],
        }
    }
}
