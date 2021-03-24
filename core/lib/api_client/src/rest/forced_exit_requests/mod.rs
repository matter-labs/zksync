//! Blocks part of API implementation.

// Built-in uses

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{forced_exit_requests::ForcedExitRequest, Address, TokenId};
use zksync_utils::BigUintSerdeAsRadix10Str;

use num::BigUint;

// Local uses
use crate::rest::v1::Client;
use crate::rest::v1::ClientResult;

// Data transfer objects.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfo {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub request_fee: BigUint,
    pub max_tokens_per_request: u8,
    pub recomended_tx_interval_millis: i64,
    pub forced_exit_contract_address: Address,
    pub wait_confirmations: u64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ForcedExitRequestStatus {
    Enabled(ConfigInfo),
    Disabled,
}

#[derive(Deserialize, Serialize)]
pub struct ForcedExitRegisterRequest {
    pub target: Address,
    pub tokens: Vec<TokenId>,
    // Even though the price is constant, we still need to specify it,
    // since the price might change (with config)
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub price_in_wei: BigUint,
}

const FORCED_EXIT_REQUESTS_SCOPE: &str = "/api/forced_exit_requests/v0.1/";

impl Client {
    pub async fn get_forced_exit_requests_status(&self) -> ClientResult<ForcedExitRequestStatus> {
        self.get_with_scope(FORCED_EXIT_REQUESTS_SCOPE, "status")
            .send()
            .await
    }

    pub async fn submit_forced_exit_request(
        &self,
        regiter_request: ForcedExitRegisterRequest,
    ) -> ClientResult<ForcedExitRequest> {
        self.post_with_scope(FORCED_EXIT_REQUESTS_SCOPE, "submit")
            .body(&regiter_request)
            .send()
            .await
    }
}
