//! Requests and responses used by the REST API.

use serde::{Deserialize, Serialize};
use zksync_types::{Account, AccountId};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestnetConfigResponse {
    pub contract_address: String,
}

#[derive(Debug, Serialize)]
pub struct WithdrawalProcessingTimeResponse {
    pub normal: u64,
    pub fast: u64,
}

#[derive(Debug, Serialize)]
pub struct AccountStateResponse {
    // None if account is not created yet.
    pub id: Option<AccountId>,
    pub commited: Account,
    pub verified: Account,
}

#[derive(Debug, Deserialize)]
pub struct TxHistoryQuery {
    pub tx_id: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Deserialize)]
pub struct HandleBlocksQuery {
    pub max_block: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Deserialize)]
pub struct BlockExplorerSearchQuery {
    pub query: String,
}
