// Built-in uses
use std::collections::HashSet;
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::event::{account::*, EventData, ZkSyncEvent};
// Local uses

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountFilter {
    pub accounts: Option<HashSet<i64>>,
    pub tokens: Option<HashSet<i32>>,
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
        // If there's a filter for tokens, deny events that do not feature them.
        // (Essentially, `CreateAccount` and `DeleteAccount`).
        if let Some(token_ids) = &self.tokens {
            let token_id = match account_event.account_update_details.token_id {
                Some(token_id) => token_id,
                None => return false,
            };
            if !token_ids.contains(&token_id) {
                return false;
            }
        }
        if let Some(account_ids) = &self.accounts {
            let account_id = account_event.account_update_details.account_id;
            if !account_ids.contains(&account_id) {
                return false;
            }
        }
        return true;
    }
}
