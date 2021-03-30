// Built-in uses
use std::str::FromStr;

// External uses
use chrono::{DateTime, Utc};
use num::BigUint;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Workspace uses
use zksync_crypto::{convert::FeConvert, serialization::FrSerde, Fr};
use zksync_storage::chain::{
    block::records::{BlockDetails, BlockTransactionItem},
    operations::records::StoredExecutedPriorityOperation,
    operations_ext::records::TxReceiptResponse,
};
use zksync_types::{
    tx::{EthBatchSignatures, TxEthSignature, TxHash},
    Address, BlockNumber, EthBlockId, OutputFeeType, PriorityOpId, TokenId, ZkSyncTx,
};
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

// TODO: remove `fee_type`, `gas_tx_amount`, `gas_price_wei`
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiFee {
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

// TODO: add `zkp_fee` and `gas_fee`
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiBatchFee {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

impl From<zksync_types::Fee> for ApiFee {
    fn from(fee: zksync_types::Fee) -> Self {
        ApiFee {
            fee_type: fee.fee_type,
            gas_tx_amount: fee.gas_tx_amount,
            gas_price_wei: fee.gas_price_wei,
            gas_fee: fee.gas_fee,
            zkp_fee: fee.zkp_fee,
            total_fee: fee.total_fee,
        }
    }
}

impl From<zksync_types::BatchFee> for ApiBatchFee {
    fn from(fee: zksync_types::BatchFee) -> Self {
        ApiBatchFee {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IncomingTxBatch {
    pub txs: Vec<ZkSyncTx>,
    pub signature: EthBatchSignatures,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IncomingTx {
    pub tx: ZkSyncTx,
    pub signature: Option<TxEthSignature>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TxData {
    pub tx: Transaction,
    pub eth_signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L1Status {
    //Pending,
    Committed,
    Finalized,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2Status {
    Queued,
    Committed,
    Finalized,
    Rejected,
}

impl From<L1Status> for L2Status {
    fn from(status: L1Status) -> Self {
        match status {
            L1Status::Committed => L2Status::Committed,
            L1Status::Finalized => L2Status::Finalized,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct L1Receipt {
    pub status: L1Status,
    pub eth_block: EthBlockId,
    pub rollup_block: Option<BlockNumber>,
    pub id: PriorityOpId,
}

impl From<(StoredExecutedPriorityOperation, bool)> for L1Receipt {
    fn from(op: (StoredExecutedPriorityOperation, bool)) -> L1Receipt {
        let eth_block = EthBlockId(op.0.eth_block as u64);
        let rollup_block = Some(BlockNumber(op.0.block_number as u32));
        let id = PriorityOpId(op.0.priority_op_serialid as u64);

        let finalized = op.1;

        let status = if finalized {
            L1Status::Finalized
        } else {
            L1Status::Committed
        };

        L1Receipt {
            status,
            eth_block,
            rollup_block,
            id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct L2Receipt {
    pub tx_hash: TxHash,
    pub rollup_block: Option<BlockNumber>,
    pub status: L2Status,
    pub fail_reason: Option<String>,
}

impl From<TxReceiptResponse> for L2Receipt {
    fn from(receipt: TxReceiptResponse) -> L2Receipt {
        let mut tx_hash_with_prefix = "sync-tx:".to_string();
        tx_hash_with_prefix.push_str(&receipt.tx_hash);
        let tx_hash = TxHash::from_str(&tx_hash_with_prefix).unwrap();
        let rollup_block = Some(BlockNumber(receipt.block_number as u32));
        let fail_reason = receipt.fail_reason;
        let status = if receipt.success {
            if receipt.verified {
                L2Status::Finalized
            } else {
                L2Status::Committed
            }
        } else {
            L2Status::Rejected
        };
        L2Receipt {
            tx_hash,
            rollup_block,
            status,
            fail_reason,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Receipt {
    L1(L1Receipt),
    L2(L2Receipt),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub tx_hash: TxHash,
    pub block_number: Option<BlockNumber>,
    pub op: Value,
    pub status: L2Status,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<(BlockTransactionItem, bool)> for Transaction {
    fn from(item: (BlockTransactionItem, bool)) -> Self {
        let tx_hash = TxHash::from_str(item.0.tx_hash.replace("0x", "sync-tx:").as_str()).unwrap();
        let status = if item.0.success.unwrap_or_default() {
            if item.1 {
                L2Status::Finalized
            } else {
                L2Status::Committed
            }
        } else {
            L2Status::Rejected
        };
        Self {
            tx_hash,
            block_number: Some(BlockNumber(item.0.block_number as u32)),
            op: item.0.op,
            status,
            fail_reason: item.0.fail_reason,
            created_at: item.0.created_at,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LastVariant {
    LastCommitted,
    LastFinalized,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum BlockPosition {
    Number(BlockNumber),
    Variant(LastVariant),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Usd {
    Usd,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum TokenIdOrUsd {
    Id(TokenId),
    Usd(Usd),
}
