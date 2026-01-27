// Built-in uses
// External uses
// Workspace uses
use zksync_types::ZkSyncOp;
// Local uses
use super::{
    default::parse_pub_data, v4::rollup_ops_blocks_from_bytes_inner, version::ZkSyncContractVersion,
};
use crate::rollup_ops::RollupOpsBlock;

pub fn rollup_ops_blocks_from_bytes(data: Vec<u8>) -> anyhow::Result<Vec<RollupOpsBlock>> {
    rollup_ops_blocks_from_bytes_inner(data, ZkSyncContractVersion::V6)
}

pub fn get_rollup_ops_from_data(data: &[u8]) -> Result<Vec<ZkSyncOp>, anyhow::Error> {
    parse_pub_data(
        data,
        ZkSyncOp::from_public_data,
        ZkSyncOp::public_data_length,
    )
}
