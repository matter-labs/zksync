use chrono::{DateTime, Utc};
use num::BigUint;
use zksync_basic_types::{AccountId, TokenId};
use zksync_utils::BigUintSerdeAsRadix10Str;

use serde::{Deserialize, Serialize};

pub type ForcedExitRequestId = i64;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ForcedExitRequest {
    pub id: ForcedExitRequestId,
    pub account_id: AccountId,
    pub token_id: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub price_in_wei: BigUint,
    pub valid_until: DateTime<Utc>,
}
