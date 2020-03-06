use models::node::{Account, AccountId};

pub struct StoredAccountState {
    pub committed: Option<(AccountId, Account)>,
    pub verified: Option<(AccountId, Account)>,
}
