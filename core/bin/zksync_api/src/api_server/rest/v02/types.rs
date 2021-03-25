// External uses
use chrono::{DateTime, Utc};
use num::BigUint;
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_crypto::{convert::FeConvert, serialization::FrSerde, Fr};
use zksync_storage::chain::block::records::BlockDetails;
use zksync_types::{tx::TxHash, Address, BlockNumber, OutputFeeType, TokenId};
use zksync_utils::BigUintSerdeAsRadix10Str;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct BlockInfo {
    pub block_number: BlockNumber,
    #[serde(with = "FrSerde")]
    pub new_state_root: Fr,
    pub block_size: u64,
    pub commit_tx_hash: Option<TxHash>,
    pub verify_tx_hash: Option<TxHash>,
    pub committed_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

impl From<BlockDetails> for BlockInfo {
    fn from(details: BlockDetails) -> BlockInfo {
        BlockInfo {
            block_number: BlockNumber(details.block_number as u32),
            new_state_root: Fr::from_bytes(&details.new_state_root).unwrap_or_else(|err| {
                panic!(
                    "Database provided an incorrect new_state_root field: {:?}, an error occurred {}",
                    details.new_state_root, err
                )
            }),
            block_size: details.block_size as u64,
            commit_tx_hash: details.commit_tx_hash.map(|bytes| {
                TxHash::from_slice(&bytes).unwrap_or_else(|| {
                    panic!(
                        "Database provided an incorrect commit_tx_hash field: {:?}",
                        hex::encode(bytes)
                    )
                })
            }),
            verify_tx_hash: details.verify_tx_hash.map(|bytes| {
                TxHash::from_slice(&bytes).unwrap_or_else(|| {
                    panic!(
                        "Database provided an incorrect verify_tx_hash field: {:?}",
                        hex::encode(bytes)
                    )
                })
            }),
            committed_at: details.committed_at,
            verified_at: details.verified_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fee {
    pub fee_type: OutputFeeType,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_tx_amount: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_price_wei: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub zkp_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BatchFee {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

impl From<zksync_types::Fee> for Fee {
    fn from(fee: zksync_types::Fee) -> Self {
        Fee {
            fee_type: fee.fee_type,
            gas_tx_amount: fee.gas_tx_amount,
            gas_price_wei: fee.gas_price_wei,
            gas_fee: fee.gas_fee,
            zkp_fee: fee.zkp_fee,
            total_fee: fee.total_fee,
        }
    }
}

impl From<zksync_types::BatchFee> for BatchFee {
    fn from(fee: zksync_types::BatchFee) -> Self {
        BatchFee {
            total_fee: fee.total_fee,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ApiToken {
    pub id: TokenId,
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub enabled_for_fees: bool,
}
