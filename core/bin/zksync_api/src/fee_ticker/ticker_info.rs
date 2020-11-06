//! Additional methods gathering the information required
//! by ticker for operating.

// External deps
use async_trait::async_trait;
// Workspace deps
use zksync_storage::ConnectionPool;
use zksync_types::Address;
// Local deps

/// Api responsible for querying for TokenPrices
#[async_trait]
pub trait FeeTickerInfo {
    /// Check whether account exists in the zkSync network or not.
    /// Returns `true` if account does not yet exist in the zkSync network.
    async fn is_account_new(&mut self, address: Address) -> bool;
}

pub struct TickerInfo {
    db: ConnectionPool,
}

impl TickerInfo {
    pub fn new(db: ConnectionPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl FeeTickerInfo for TickerInfo {
    async fn is_account_new(&mut self, address: Address) -> bool {
        let mut storage = self
            .db
            .access_storage()
            .await
            .expect("Unable to establish connection to db");

        let account_state = storage
            .chain()
            .account_schema()
            .account_state_by_address(&address)
            .await
            .expect("Unable to query account state from the database");

        // If account is `Some(_)` then it's not new.
        account_state.committed.is_none()
    }
}
