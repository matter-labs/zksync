// Built-in uses
use std::collections::HashSet;
// Workspace uses
use zksync_storage::event::types::{transaction::*, EventData, ZkSyncEvent};
// External uses
use serde::Deserialize;
// Local uses

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionFilter {
    pub types: Option<HashSet<TransactionType>>,
    pub accounts: Option<HashSet<i64>>,
    pub tokens: Option<HashSet<i32>>,
    pub status: Option<TransactionStatus>,
}

impl TransactionFilter {
    pub fn matches(&self, event: &ZkSyncEvent) -> bool {
        let tx_event = match &event.data {
            EventData::Transaction(tx_event) => tx_event,
            _ => return false,
        };
        if let Some(status) = &self.status {
            if tx_event.status != *status {
                return false;
            }
        }
        if let Some(tx_types) = &self.types {
            let tx_type = tx_event.tx_type();
            if !tx_types.contains(&tx_type) {
                return false;
            }
        }
        if let Some(token_ids) = &self.tokens {
            let token_id = tx_event.token_id;
            if !token_ids.contains(&token_id) {
                return false;
            }
        }
        if let Some(account_ids) = &self.accounts {
            let account_id = tx_event.account_id;
            if !account_ids.contains(&account_id) {
                return false;
            }
        }
        return true;
    }
}
