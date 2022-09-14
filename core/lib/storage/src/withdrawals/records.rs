use crate::BigDecimal;
use std::convert::TryFrom;
use zksync_types::withdrawals::{WithdrawalPendingEvent, WithdrawalType};
use zksync_types::{Address, TokenId, H256, U256};

pub struct PendingWithdrawal {
    pub id: i32,
    pub account: Vec<u8>,
    pub full_amount: BigDecimal,
    pub remaining_amount: BigDecimal,
    pub token_id: i32,
    pub withdrawal_type: String,
    pub tx_hash: Vec<u8>,
    pub tx_block: i64,
    pub tx_log_index: i64,
}

pub struct FinalizedWithdrawal {
    pub id: i32,
    pub amount: BigDecimal,
    pub pending_withdrawals_id: i64,
    pub tx_hash: Vec<u8>,
    pub tx_block: i64,
    pub tx_log_index: i64,
}

pub struct ExtendedFinalizedWithdrawal {
    pub amount: BigDecimal,
    pub account: Vec<u8>,
    pub token_id: i32,
    pub withdrawal_type: String,
    pub tx_hash: Vec<u8>,
    pub tx_block: i64,
    pub tx_log_index: i64,
}

impl From<ExtendedFinalizedWithdrawal> for WithdrawalPendingEvent {
    fn from(value: ExtendedFinalizedWithdrawal) -> Self {
        let amount = U256::from_dec_str(&value.amount.to_string()).unwrap();
        Self {
            block_number: value.tx_block as u64,
            tx_hash: H256::from_slice(&value.tx_hash),
            token_id: TokenId(value.token_id as u32),
            recipient: Address::from_slice(&value.account),
            amount,
            log_index: value.tx_log_index as u64,
            withdrawal_type: WithdrawalType::try_from(value.withdrawal_type).unwrap(),
        }
    }
}
