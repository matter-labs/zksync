use std::convert::TryFrom;
use std::fmt::{Display, Formatter};

use ethabi::Address;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use zksync_basic_types::TokenId;

use crate::{Log, H256, U256};

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum WithdrawalType {
    Withdrawal,
    FullExit,
    ForcedExit,
}

impl TryFrom<String> for WithdrawalType {
    type Error = WithdrawalPendingParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(match value.as_str() {
            "Withdrawal" => WithdrawalType::Withdrawal,
            "FullExit" => WithdrawalType::FullExit,
            "ForcedExit" => WithdrawalType::ForcedExit,
            _ => return Err(WithdrawalPendingParseError::TypeError),
        })
    }
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
            _ => return Err(WithdrawalPendingParseError::TypeError),
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
    pub log_index: u64,
    pub token_id: TokenId,
    pub recipient: Address,
    pub amount: U256,
    pub tx_hash: H256,
}

#[derive(Debug, Error)]
#[allow(clippy::large_enum_variant)]
pub enum WithdrawalPendingParseError {
    #[error("Cannot parse log for Withdrawal Pending Event {0:?}")]
    ParseError(Log),
    #[error("Type Error")]
    TypeError,
}

impl TryFrom<Log> for WithdrawalPendingEvent {
    type Error = WithdrawalPendingParseError;

    fn try_from(event: Log) -> Result<WithdrawalPendingEvent, WithdrawalPendingParseError> {
        if event.topics.len() != 3 || event.data.0.len() != 32 * 2 {
            return Err(WithdrawalPendingParseError::ParseError(event));
        }
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
        if event.topics.len() != 3 || event.data.0.len() != 32 {
            return Err(WithdrawalPendingParseError::ParseError(event));
        }

        let amount = U256::from_big_endian(&event.data.0);
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
