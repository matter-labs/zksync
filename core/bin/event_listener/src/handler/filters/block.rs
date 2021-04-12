use zksync_storage::event::types::{block::*, ZkSyncEvent, EventData};

#[derive(Debug, Clone)]
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
