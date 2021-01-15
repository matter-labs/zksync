//! Config part of API implementation.

// Built-in uses

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::Address;

// Local uses
use super::client::{self, Client};

// Data transfer objects.

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Contracts {
    pub contract: Address,
}

/// Configuration API part.
impl Client {
    pub async fn contracts(&self) -> client::Result<Contracts> {
        self.get("config/contracts").send().await
    }

    pub async fn deposit_confirmations(&self) -> client::Result<u64> {
        self.get("config/deposit_confirmations").send().await
    }

    pub async fn network(&self) -> client::Result<String> {
        self.get("config/network").send().await
    }
}
