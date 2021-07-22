// Built-in uses
use std::collections::HashMap;
use std::fmt;
// Workspace uses
use zksync_storage::event::{get_event_type, EventType};
use zksync_types::event::ZkSyncEvent;
// External uses
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
// Local uses
use self::{account::AccountFilter, block::BlockFilter, transaction::TransactionFilter};

mod account;
mod block;
mod transaction;

#[cfg(test)]
mod tests;

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
        let event_type = get_event_type(event);
        match self.0.get(&event_type) {
            Some(filter) => filter.matches(event),
            None => self.0.is_empty(),
        }
    }
}

struct EventFiltersVisitor;

impl<'de> Visitor<'de> for EventFiltersVisitor {
    type Value = HashMap<EventType, EventFilter>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("map")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = HashMap::with_capacity(access.size_hint().unwrap_or(0));

        while let Some(key) = access.next_key()? {
            let value = match key {
                EventType::Account => EventFilter::Account(access.next_value::<AccountFilter>()?),
                EventType::Block => EventFilter::Block(access.next_value::<BlockFilter>()?),
                EventType::Transaction => {
                    EventFilter::Transaction(access.next_value::<TransactionFilter>()?)
                }
            };

            map.insert(key, value);
        }

        Ok(map)
    }
}

impl<'de> Deserialize<'de> for SubscriberFilters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(deserializer.deserialize_map(EventFiltersVisitor)?))
    }
}
