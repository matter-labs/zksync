// External imports
use sqlx::types::BigDecimal;
// Workspace imports
// Local imports
use zksync_types::{tx::TxHash, TokenId};

pub struct Subsidy {
    pub tx_hash: TxHash,
    pub usd_amount_scaled: u64,
    pub full_cost_usd_scaled: u64,
    pub token_id: TokenId,
    pub token_amount: BigDecimal,
    pub full_cost_token: BigDecimal,
    pub subsidy_type: String,
}
