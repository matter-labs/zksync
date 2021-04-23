// Built-in uses
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::event::{block::*, EventData, ZkSyncEvent};
// Local uses

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockFilter {
    pub block_status: Option<BlockStatus>,
}

impl BlockFilter {
    pub fn matches(&self, event: &ZkSyncEvent) -> bool {
        let block_event = match &event.data {
            EventData::Block(block_event) => block_event,
            _ => return false,
        };
        if let Some(block_status) = &self.block_status {
            if block_event.status != *block_status {
                return false;
            }
        }
        return true;
    }
}
