use std::collections::HashSet;
use zksync_storage::event::types::{account::*, ZkSyncEvent, EventData};

#[derive(Debug, Clone)]
pub struct AccountFilter {
    pub account_ids: Option<HashSet<i64>>,
    pub token_ids: Option<HashSet<i32>>,
    pub status: Option<AccountStateChangeStatus>,
}

impl AccountFilter {
    pub fn matches(&self, event: &ZkSyncEvent) -> bool {
        let account_event = match &event.data {
            EventData::Account(account_event) => account_event,
            _ => return false,
        };
        if let Some(status) = &self.status {
            if account_event.status != *status {
                return false;
            }
        }
        if let Some(token_ids) = &self.token_ids {
            if let Some(token_id) = account_event.account_update_details.token_id {
                if !token_ids.contains(&token_id) {
                    return false;
                }
            }
        }
        if let Some(account_ids) = &self.account_ids {
            let account_id = account_event.account_update_details.account_id;
            if !account_ids.contains(&account_id) {
                return false;
            }
        }
        return true;
    }
}
