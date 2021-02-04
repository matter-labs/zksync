//! Blocks part of API implementation.

// Built-in uses

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{tx::TxHash, Address, TokenId};
use zksync_utils::BigUintSerdeAsRadix10Str;

use num::BigUint;

// Local uses
use super::client::{self, Client};

// Data transfer objects.

#[derive(Serialize, Deserialize)]
pub struct IsForcedExitEnabledResponse {
    pub enabled: bool,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForcedExitRequestFee {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub request_fee: BigUint,
}

#[derive(Deserialize, Serialize)]
pub struct ForcedExitRegisterRequest {
    pub target: Address,
    pub tokens: Vec<TokenId>,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub price_in_wei: BigUint,
}

impl Client {
    pub async fn are_forced_exit_requests_enabled(
        &self,
    ) -> client::Result<IsForcedExitEnabledResponse> {
        self.get("forced_exit_requests/enabled").send().await
    }

    pub async fn get_forced_exit_request_fee(&self) -> client::Result<ForcedExitRequestFee> {
        self.get("forced_exit_requests/fee").send().await
    }

    pub async fn submit_forced_exit_request(
        &self,
        regiter_request: ForcedExitRegisterRequest,
    ) -> client::Result<TxHash> {
        self.post("forced_exit_requests/submit")
            .body(&regiter_request)
            .send()
            .await
    }
}
