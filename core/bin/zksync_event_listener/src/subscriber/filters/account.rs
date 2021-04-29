// Built-in uses
use std::collections::HashSet;
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::{
    event::{account::*, EventData, ZkSyncEvent},
    AccountId, TokenId,
};
// Local uses

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountFilter {
    pub accounts: Option<HashSet<AccountId>>,
    pub tokens: Option<HashSet<TokenId>>,
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
            let token_id = match account_event.update_details.token_id {
                Some(token_id) => token_id,
                None => return false,
            };
            if !token_ids.contains(&token_id) {
                return false;
            }
        }
        if let Some(account_ids) = &self.accounts {
            let account_id = account_event.update_details.account_id;
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
    use zksync_types::event::test_data::get_account_event;

    #[test]
    fn test_account_filter() {
        // Match all events.
        let mut account_filter = AccountFilter {
            accounts: None,
            tokens: None,
            status: None,
        };

        let event = get_account_event(
            AccountId(100),
            Some(TokenId(10)),
            AccountStateChangeStatus::Finalized,
        );
        assert!(account_filter.matches(&event));

        // Only match by account id.
        account_filter.accounts = Some([1000, 2000].iter().map(|id| AccountId(*id)).collect());
        assert!(!account_filter.matches(&event));
        // Both ids should match.
        let event = get_account_event(AccountId(1000), None, AccountStateChangeStatus::Finalized);
        assert!(account_filter.matches(&event));
        let event = get_account_event(
            AccountId(2000),
            Some(TokenId(10)),
            AccountStateChangeStatus::Finalized,
        );
        assert!(account_filter.matches(&event));
        // Regardless of status too.
        let event = get_account_event(
            AccountId(2000),
            Some(TokenId(15)),
            AccountStateChangeStatus::Committed,
        );
        assert!(account_filter.matches(&event));

        // Add token id filter.
        account_filter.tokens = Some([0, 20].iter().map(|id| TokenId(*id)).collect());
        // Previous event doesn't match.
        assert!(!account_filter.matches(&event));
        // Events without token ids are filtered out too.
        let event = get_account_event(AccountId(2000), None, AccountStateChangeStatus::Committed);
        assert!(!account_filter.matches(&event));
        // Try correct one with both statuses.
        let event = get_account_event(
            AccountId(2000),
            Some(TokenId(0)),
            AccountStateChangeStatus::Committed,
        );
        assert!(account_filter.matches(&event));
        let event = get_account_event(
            AccountId(2000),
            Some(TokenId(20)),
            AccountStateChangeStatus::Finalized,
        );
        assert!(account_filter.matches(&event));

        // Finally, add a status filter.
        account_filter.status = Some(AccountStateChangeStatus::Committed);
        // No match.
        assert!(!account_filter.matches(&event));
        // Correct status.
        let event = get_account_event(
            AccountId(1000),
            Some(TokenId(20)),
            AccountStateChangeStatus::Committed,
        );
        assert!(account_filter.matches(&event));
    }
}
