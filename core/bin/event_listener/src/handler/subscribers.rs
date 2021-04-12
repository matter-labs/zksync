use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use zksync_storage::event::types::{EventType, ZkSyncEvent};
use super::filters::EventFilter;

#[derive(Debug, Clone)]
pub struct Subscriber {
    pub id: i64,
    pub filters: HashMap<EventType, EventFilter>,
}

impl Subscriber {
    pub fn matches(&self, event: &ZkSyncEvent) -> bool {
        let event_type = event.get_type();
        match self.filters.get(&event_type) {
            Some(filter) => filter.matches(event),
            None => self.filters.is_empty(),
        }
    }
}

impl PartialEq for Subscriber {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Subscriber {}

impl Hash for Subscriber {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
