//! Accounts API client implementation

// Built-in uses

// External uses

// Workspace uses

// Local uses
use crate::api_server::v1::client::{Client, ClientError};

use super::types::{AccountInfo, AccountQuery, AccountReceipts, AccountReceiptsQuery};

/// Accounts API part.
impl Client {
    /// Gets account information
    pub async fn account_info(
        &self,
        account: impl Into<AccountQuery>,
    ) -> Result<Option<AccountInfo>, ClientError> {
        let account = account.into();

        self.get(&format!("accounts/{}", account)).send().await
    }

    pub async fn account_receipts(
        &self,
        account: impl Into<AccountQuery>,
        from: AccountReceipts,
        limit: u32,
    ) -> Result<(), ClientError> {
        let account = account.into();

        self.get(&format!("accounts/{}/receipts", account))
            .query(&AccountReceiptsQuery::new(from, limit))
            .send()
            .await
    }
}
