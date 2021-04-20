// Built-in uses
use std::collections::HashMap;
use std::convert::TryFrom;
// Workspace uses
use zksync_storage::event::{records::EventType, types::ZkSyncEvent};
// External uses
// Local uses
use account::AccountFilter;
use block::BlockFilter;
use transaction::TransactionFilter;

mod account;
mod block;
mod transaction;

#[derive(Debug, Clone)]
pub enum EventFilter {
    Account(AccountFilter),
    Block(BlockFilter),
    Transaction(TransactionFilter),
}

impl EventFilter {
    pub fn matches(&self, event: &ZkSyncEvent) -> bool {
        match self {
            EventFilter::Account(account_filter) => account_filter.matches(event),
            EventFilter::Block(block_filter) => block_filter.matches(event),
            EventFilter::Transaction(tx_filter) => tx_filter.matches(event),
        }
    }
}

#[derive(Debug)]
pub struct SubscriberFilters(HashMap<EventType, EventFilter>);

impl SubscriberFilters {
    pub fn matches(&self, event: &ZkSyncEvent) -> bool {
        let event_type = event.get_type();
        match self.0.get(&event_type) {
            Some(filter) => filter.matches(event),
            None => self.0.is_empty(),
        }
    }
}

impl TryFrom<String> for SubscriberFilters {
    type Error = serde_json::Error;

    fn try_from(input: String) -> Result<Self, Self::Error> {
        let value_map: HashMap<EventType, serde_json::Value> = serde_json::from_str(&input)?;
        let mut event_map = HashMap::new();
        for (event_type, value) in value_map.into_iter() {
            let filter = match event_type {
                EventType::Account => EventFilter::Account(serde_json::from_value(value)?),
                EventType::Block => EventFilter::Block(serde_json::from_value(value)?),
                EventType::Transaction => EventFilter::Transaction(serde_json::from_value(value)?),
            };
            event_map.insert(event_type, filter);
        }
        Ok(Self(event_map))
    }
}
