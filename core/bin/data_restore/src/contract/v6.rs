use super::{v4::rollup_ops_blocks_from_bytes_inner, version::ZkSyncContractVersion};
use crate::rollup_ops::RollupOpsBlock;

pub fn rollup_ops_blocks_from_bytes(data: Vec<u8>) -> anyhow::Result<Vec<RollupOpsBlock>> {
    rollup_ops_blocks_from_bytes_inner(data, ZkSyncContractVersion::V6)
}
