//! Accounts API client implementation

// Built-in uses

// External uses

// Workspace uses

// Local uses
use crate::api_server::v1::client::{Client, ClientError};

use super::types::{
    AccountInfo, AccountOpReceipt, AccountQuery, AccountReceipts, AccountReceiptsQuery,
    AccountTxReceipt, PendingAccountOpReceipt,
};

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

    pub async fn account_tx_receipts(
        &self,
        account: impl Into<AccountQuery>,
        from: AccountReceipts,
        limit: u32,
    ) -> Result<Vec<AccountTxReceipt>, ClientError> {
        let account = account.into();

        self.get(&format!("accounts/{}/transactions/receipts", account))
            .query(&AccountReceiptsQuery::new(from, limit))
            .send()
            .await
    }

    pub async fn account_op_receipts(
        &self,
        account: impl Into<AccountQuery>,
        from: AccountReceipts,
        limit: u32,
    ) -> Result<Vec<AccountOpReceipt>, ClientError> {
        let account = account.into();

        self.get(&format!("accounts/{}/operations/receipts", account))
            .query(&AccountReceiptsQuery::new(from, limit))
            .send()
            .await
    }

    pub async fn account_pending_ops(
        &self,
        account: impl Into<AccountQuery>,
    ) -> Result<Vec<PendingAccountOpReceipt>, ClientError> {
        let account = account.into();

        self.get(&format!("accounts/{}/operations/pending", account))
            .send()
            .await
    }
}
