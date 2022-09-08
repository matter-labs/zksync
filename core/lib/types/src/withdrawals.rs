use crate::Log;
use crate::{H256, U256};
use ethabi::Address;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use thiserror::Error;
use zksync_basic_types::TokenId;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WithdrawalPendingEvent {
    pub block_number: u64,
    pub tx_hash: H256,
    pub token_id: TokenId,
    pub recipient: Address,
    pub amount: U256,
    pub withdrawal_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WithdrawalEvent {
    pub block_number: u64,
    pub token_id: TokenId,
    pub recipient: Address,
    pub amount: U256,
    pub tx_hash: H256,
}

#[derive(Debug, Error)]
pub enum WithdrawalPendingParseError {
    #[error("Cannot parse log for Withdrawal Pending Event {0:?}")]
    ParseError(Log),
}

impl TryFrom<Log> for WithdrawalPendingEvent {
    type Error = WithdrawalPendingParseError;

    fn try_from(event: Log) -> Result<WithdrawalPendingEvent, WithdrawalPendingParseError> {
        if event.topics.len() != 5 {
            return Err(WithdrawalPendingParseError::ParseError(event));
        }

        assert_eq!(event.data.0.len(), 32 * 2);
        let amount = U256::from_big_endian(&event.data.0[..32]);
        let tx_type = U256::from_big_endian(&event.data.0[32..]);
        dbg!(tx_type);
        Ok(WithdrawalPendingEvent {
            block_number: event.block_number.unwrap().as_u64(),
            tx_hash: event.transaction_hash.unwrap(),
            token_id: TokenId(
                U256::from_big_endian(&event.topics[1].as_fixed_bytes()[..]).as_u32(),
            ),
            recipient: Address::from_slice(&event.topics[2].as_fixed_bytes()[12..]),
            amount,
            withdrawal_type: Default::default(),
        })
    }
}

impl TryFrom<Log> for WithdrawalEvent {
    type Error = WithdrawalPendingParseError;

    fn try_from(event: Log) -> Result<WithdrawalEvent, WithdrawalPendingParseError> {
        if event.topics.len() != 5 {
            return Err(WithdrawalPendingParseError::ParseError(event));
        }

        Ok(WithdrawalEvent {
            block_number: event.block_number.unwrap().as_u64(),
            token_id: TokenId(
                U256::from_big_endian(&event.topics[1].as_fixed_bytes()[..]).as_u32(),
            ),
            recipient: Address::from_slice(&event.topics[2].as_fixed_bytes()[12..]),
            amount: U256::from_big_endian(&event.topics[3].as_fixed_bytes()[..]),
            tx_hash: event.transaction_hash.unwrap(),
        })
    }
}
