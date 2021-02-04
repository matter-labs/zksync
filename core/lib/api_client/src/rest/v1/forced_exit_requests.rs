//! Blocks part of API implementation.

// Built-in uses

// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Workspace uses
use zksync_crypto::{serialization::FrSerde, Fr};
use zksync_types::{tx::TxHash, Address, BlockNumber, TokenId};
use zksync_utils::BigUintSerdeAsRadix10Str;

use num::BigUint;

// Local uses
use super::{
    client::{self, Client},
    Pagination,
};

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
    pub async fn is_forced_exit_enabled(&self) -> client::Result<IsForcedExitEnabledResponse> {
        self.get("forced_exit/enabled").send().await
    }

    pub async fn get_forced_exit_request_fee(&self) -> client::Result<ForcedExitRequestFee> {
        self.get("forced_exit/fee").send().await
    }

    pub async fn submit_forced_exit_request(
        &self,
        regiter_request: ForcedExitRegisterRequest,
    ) -> client::Result<TxHash> {
        self.post("forced_exit/submit")
            .body(&regiter_request)
            .send()
            .await
    }
}
