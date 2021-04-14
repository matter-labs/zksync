use std::collections::HashSet;
use zksync_storage::event::types::{transaction::*, EventData, ZkSyncEvent};

#[derive(Debug, Clone)]
pub struct TransactionFilter {
    pub tx_types: Option<HashSet<TransactionType>>,
    pub account_ids: Option<HashSet<i64>>,
    pub token_ids: Option<HashSet<i32>>,
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
        if let Some(tx_types) = &self.tx_types {
            let tx_type = tx_event.tx_type();
            if !tx_types.contains(&tx_type) {
                return false;
            }
        }
        if let Some(token_ids) = &self.token_ids {
            let token_id = tx_event.token_id;
            if !token_ids.contains(&token_id) {
                return false;
            }
        }
        if let Some(account_ids) = &self.account_ids {
            let account_id = tx_event.account_id;
            if !account_ids.contains(&account_id) {
                return false;
            }
        }
        return true;
    }
}
