// Built-in uses
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::event::{block::*, EventData, ZkSyncEvent};
// Local uses

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockFilter {
    pub status: Option<BlockStatus>,
}

impl BlockFilter {
    pub fn matches(&self, event: &ZkSyncEvent) -> bool {
        let block_event = match &event.data {
            EventData::Block(block_event) => block_event,
            _ => return false,
        };
        if let Some(block_status) = &self.status {
            if block_event.status != *block_status {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_types::event::test_data::get_block_event;

    #[test]
    fn test_block_filter() {
        // Match all block events.
        let block_filter = BlockFilter { status: None };
        for block_status in &[
            BlockStatus::Committed,
            BlockStatus::Finalized,
            BlockStatus::Reverted,
        ] {
            let block_event = get_block_event(*block_status);
            assert!(block_filter.matches(&block_event));
        }
        // Only match committed blocks.
        let block_filter = BlockFilter {
            status: Some(BlockStatus::Committed),
        };
        let block_event = get_block_event(BlockStatus::Committed);
        assert!(block_filter.matches(&block_event));
        // Should be filtered out.
        let block_event = get_block_event(BlockStatus::Finalized);
        assert!(!block_filter.matches(&block_event));
    }
}
