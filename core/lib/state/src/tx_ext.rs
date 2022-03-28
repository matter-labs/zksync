use crate::error::OpError;
use zksync_types::ZkSyncTx;

pub(super) trait TxCheck {
    fn check_timestamp(&self, block_timestamp: u64) -> Result<(), OpError>;
}

impl TxCheck for ZkSyncTx {
    fn check_timestamp(&self, block_timestamp: u64) -> Result<(), OpError> {
        if !self.time_range().is_valid(block_timestamp) {
            return Err(OpError::TimestampError);
        }
        Ok(())
    }
}
