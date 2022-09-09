use crate::Log;
use crate::{H256, U256};
use ethabi::Address;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt::{Display, Formatter};
use thiserror::Error;
use zksync_basic_types::TokenId;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WithdrawalType {
    Withdrawal,
    FullExit,
    ForcedExit,
}
impl Display for WithdrawalType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl TryFrom<U256> for WithdrawalType {
    type Error = WithdrawalPendingParseError;

    fn try_from(value: U256) -> Result<Self, Self::Error> {
        Ok(match value.as_u32() {
            0 => WithdrawalType::Withdrawal,
            1 => WithdrawalType::ForcedExit,
            2 => WithdrawalType::FullExit,
            _ => {
                return Err(WithdrawalPendingParseError::ParseError(Log {
                    address: Default::default(),
                    topics: vec![],
                    data: Default::default(),
                    block_hash: None,
                    block_number: None,
                    transaction_hash: None,
                    transaction_index: None,
                    log_index: None,
                    transaction_log_index: None,
                    log_type: None,
                    removed: None,
                }))
            }
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WithdrawalPendingEvent {
    pub block_number: u64,
    pub tx_hash: H256,
    pub token_id: TokenId,
    pub recipient: Address,
    pub amount: U256,
    pub withdrawal_type: WithdrawalType,
    pub log_index: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WithdrawalEvent {
    pub block_number: u64,
    pub token_id: TokenId,
    pub recipient: Address,
    pub amount: U256,
    pub tx_hash: H256,
    pub log_index: u64,
}

#[derive(Debug, Error)]
pub enum WithdrawalPendingParseError {
    #[error("Cannot parse log for Withdrawal Pending Event {0:?}")]
    ParseError(Log),
}

impl TryFrom<Log> for WithdrawalPendingEvent {
    type Error = WithdrawalPendingParseError;

    fn try_from(event: Log) -> Result<WithdrawalPendingEvent, WithdrawalPendingParseError> {
        if event.topics.len() != 3 {
            return Err(WithdrawalPendingParseError::ParseError(event));
        }
        assert_eq!(event.data.0.len(), 32 * 2);
        let amount = U256::from_big_endian(&event.data.0[..32]);
        let tx_type = WithdrawalType::try_from(U256::from_big_endian(&event.data.0[32..]))?;
        Ok(WithdrawalPendingEvent {
            block_number: event.block_number.unwrap().as_u64(),
            tx_hash: event.transaction_hash.unwrap(),
            token_id: TokenId(
                U256::from_big_endian(&event.topics[1].as_fixed_bytes()[..]).as_u32(),
            ),
            recipient: Address::from_slice(&event.topics[2].as_fixed_bytes()[12..]),
            amount,
            withdrawal_type: tx_type,
            log_index: event.log_index.unwrap().as_u64(),
        })
    }
}

impl TryFrom<Log> for WithdrawalEvent {
    type Error = WithdrawalPendingParseError;

    fn try_from(event: Log) -> Result<WithdrawalEvent, WithdrawalPendingParseError> {
        if event.topics.len() != 3 {
            return Err(WithdrawalPendingParseError::ParseError(event));
        }

        assert_eq!(event.data.0.len(), 32);
        let amount = U256::from_big_endian(&event.data.0[..32]);
        Ok(WithdrawalEvent {
            block_number: event.block_number.unwrap().as_u64(),
            recipient: Address::from_slice(&event.topics[1].as_fixed_bytes()[12..]),
            token_id: TokenId(
                U256::from_big_endian(&event.topics[2].as_fixed_bytes()[..]).as_u32(),
            ),
            amount,
            tx_hash: event.transaction_hash.unwrap(),
            log_index: event.log_index.unwrap().as_u64(),
        })
    }
}
