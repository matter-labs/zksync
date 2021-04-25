// Built-in uses
use std::collections::HashSet;
// Workspace uses
use zksync_types::event::{transaction::*, EventData, ZkSyncEvent};
// External uses
use serde::Deserialize;
// Local uses

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
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
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_types::event::test_data::get_transaction_event;

    #[test]
    fn test_transaction_filter() {
        // Match all events.
        let mut tx_filter = TransactionFilter {
            types: None,
            accounts: None,
            tokens: None,
            status: None,
        };

        let event =
            get_transaction_event(TransactionType::Deposit, 1, 0, TransactionStatus::Committed);
        assert!(tx_filter.matches(&event));
        // Add types filter.
        let tx_types = [
            TransactionType::Transfer,
            TransactionType::ChangePubKey,
            TransactionType::Withdraw,
        ];
        tx_filter.types = Some(tx_types.iter().copied().collect());
        // The deposit doesn't match.
        assert!(!tx_filter.matches(&event));
        // Change the type so it matches.
        for &tx_type in tx_types.iter() {
            let event = get_transaction_event(tx_type, 1, 0, TransactionStatus::Committed);
            assert!(tx_filter.matches(&event));
        }
        // Add status filter.
        tx_filter.status = Some(TransactionStatus::Rejected);
        // Committed transaction doesn't match.
        assert!(!tx_filter.matches(&event));
        // Change the status.
        let event = get_transaction_event(
            TransactionType::ChangePubKey,
            1,
            0,
            TransactionStatus::Rejected,
        );
        assert!(tx_filter.matches(&event));
        // Add accounts filter.
        let accounts = [12, 34, 56];
        tx_filter.accounts = Some(accounts.iter().copied().collect());
        assert!(!tx_filter.matches(&event));
        let tokens = [1, 2, 3, 4];
        for (&account_id, &token_id) in accounts.iter().zip(tokens.iter()) {
            // Filter by type, account and status. Token doesn't matter.
            let event = get_transaction_event(
                TransactionType::Transfer,
                account_id,
                token_id,
                TransactionStatus::Rejected,
            );
            assert!(tx_filter.matches(&event));
        }
        // Finally, add tokens filter.
        tx_filter.tokens = Some(tokens.iter().copied().collect());
        // No matching tokens.
        for &token_id in &[5, 11, 20, 25] {
            let event = get_transaction_event(
                TransactionType::Transfer,
                12,
                token_id,
                TransactionStatus::Rejected,
            );
            assert!(!tx_filter.matches(&event));
        }
        // All events should match.
        for &token_id in tokens.iter() {
            let event = get_transaction_event(
                TransactionType::Transfer,
                12,
                token_id,
                TransactionStatus::Rejected,
            );
            assert!(tx_filter.matches(&event));
        }
    }
}
