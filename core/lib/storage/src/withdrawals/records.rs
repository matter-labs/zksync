use zksync_types::withdrawals::WithdrawalPendingEvent;
use zksync_types::{Address, TokenId, H256, U256};

pub struct PendingWithdrawal {
    pub id: i32,
    pub account: Vec<u8>,
    pub amount: i64,
    pub token_id: i32,
    pub withdrawal_type: String,
    pub pending_tx_hash: Vec<u8>,
    pub pending_tx_block: i64,
    pub withdrawal_tx_hash: Option<Vec<u8>>,
    pub withdrawal_tx_block: Option<i64>,
}

impl From<PendingWithdrawal> for WithdrawalPendingEvent {
    fn from(value: PendingWithdrawal) -> Self {
        Self {
            block_number: value.pending_tx_block as u64,
            tx_hash: H256::from_slice(&value.pending_tx_hash),
            token_id: TokenId(value.token_id as u32),
            recipient: Address::from_slice(&value.account),
            amount: U256::from(value.amount),
            withdrawal_type: value.withdrawal_type,
        }
    }
}
