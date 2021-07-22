use crate::{v02::block::BlockStatus, TxWithSignature};
use chrono::{DateTime, Utc};
use num::BigUint;
use serde::{Deserialize, Serialize};
use zksync_types::{
    tx::{
        ChangePubKey, Close, EthBatchSignatures, ForcedExit, MintNFT, Swap, Transfer, TxHash,
        Withdraw, WithdrawNFT,
    },
    AccountId, Address, BlockNumber, EthBlockId, SerialId, TokenId, ZkSyncOp, ZkSyncPriorityOp,
    H256,
};
use zksync_utils::{BigUintSerdeAsRadix10Str, ZeroPrefixHexSerde};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTxBatch {
    pub txs: Vec<TxWithSignature>,
    pub signature: Option<EthBatchSignatures>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum TxInBlockStatus {
    Queued,
    Committed,
    Finalized,
    Rejected,
}

impl From<BlockStatus> for TxInBlockStatus {
    fn from(status: BlockStatus) -> Self {
        match status {
            BlockStatus::Committed => TxInBlockStatus::Committed,
            BlockStatus::Finalized => TxInBlockStatus::Finalized,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxData {
    pub tx: Transaction,
    pub eth_signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct L1Receipt {
    pub status: TxInBlockStatus,
    pub eth_block: EthBlockId,
    pub rollup_block: Option<BlockNumber>,
    pub id: SerialId,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct L2Receipt {
    #[serde(serialize_with = "ZeroPrefixHexSerde::serialize")]
    pub tx_hash: TxHash,
    pub rollup_block: Option<BlockNumber>,
    pub status: TxInBlockStatus,
    pub fail_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Receipt {
    L1(L1Receipt),
    L2(L2Receipt),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    #[serde(serialize_with = "ZeroPrefixHexSerde::serialize")]
    pub tx_hash: TxHash,
    pub block_number: Option<BlockNumber>,
    pub op: TransactionData,
    pub status: TxInBlockStatus,
    pub fail_reason: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransactionData {
    L1(L1Transaction),
    L2(L2Transaction),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum L2Transaction {
    Transfer(Box<Transfer>),
    Withdraw(Box<WithdrawData>),
    #[doc(hidden)]
    Close(Box<Close>),
    ChangePubKey(Box<ChangePubKey>),
    ForcedExit(Box<ForcedExitData>),
    MintNFT(Box<MintNFT>),
    Swap(Box<Swap>),
    WithdrawNFT(Box<WithdrawNFTData>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForcedExitData {
    #[serde(flatten)]
    pub tx: ForcedExit,
    pub eth_tx_hash: Option<H256>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawData {
    #[serde(flatten)]
    pub tx: Withdraw,
    pub eth_tx_hash: Option<H256>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawNFTData {
    #[serde(flatten)]
    pub tx: WithdrawNFT,
    pub eth_tx_hash: Option<H256>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum L1Transaction {
    Deposit(ApiDeposit),
    FullExit(ApiFullExit),
}

impl L1Transaction {
    pub fn from_executed_op(
        op: ZkSyncOp,
        eth_hash: H256,
        id: SerialId,
        tx_hash: TxHash,
    ) -> Option<Self> {
        match op {
            ZkSyncOp::Deposit(deposit) => Some(Self::Deposit(ApiDeposit {
                from: deposit.priority_op.from,
                token_id: deposit.priority_op.token,
                amount: deposit.priority_op.amount,
                to: deposit.priority_op.to,
                account_id: Some(deposit.account_id),
                eth_hash,
                id,
                tx_hash,
            })),
            ZkSyncOp::FullExit(deposit) => Some(Self::FullExit(ApiFullExit {
                token_id: deposit.priority_op.token,
                account_id: deposit.priority_op.account_id,
                eth_hash,
                id,
                tx_hash,
            })),
            _ => None,
        }
    }

    pub fn from_pending_op(
        op: ZkSyncPriorityOp,
        eth_hash: H256,
        id: SerialId,
        tx_hash: TxHash,
    ) -> Self {
        match op {
            ZkSyncPriorityOp::Deposit(deposit) => Self::Deposit(ApiDeposit {
                from: deposit.from,
                token_id: deposit.token,
                amount: deposit.amount,
                to: deposit.to,
                account_id: None,
                eth_hash,
                id,
                tx_hash,
            }),
            ZkSyncPriorityOp::FullExit(deposit) => Self::FullExit(ApiFullExit {
                token_id: deposit.token,
                account_id: deposit.account_id,
                eth_hash,
                id,
                tx_hash,
            }),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiDeposit {
    pub from: Address,
    pub token_id: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
    pub to: Address,
    pub account_id: Option<AccountId>,
    pub eth_hash: H256,
    pub id: SerialId,
    #[serde(serialize_with = "ZeroPrefixHexSerde::serialize")]
    pub tx_hash: TxHash,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiFullExit {
    pub account_id: AccountId,
    pub token_id: TokenId,
    pub eth_hash: H256,
    pub id: SerialId,
    #[serde(serialize_with = "ZeroPrefixHexSerde::serialize")]
    pub tx_hash: TxHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TxHashSerializeWrapper(
    #[serde(serialize_with = "ZeroPrefixHexSerde::serialize")] pub TxHash,
);

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubmitBatchResponse {
    pub transaction_hashes: Vec<TxHashSerializeWrapper>,
    #[serde(serialize_with = "ZeroPrefixHexSerde::serialize")]
    pub batch_hash: TxHash,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiTxBatch {
    #[serde(serialize_with = "ZeroPrefixHexSerde::serialize")]
    pub batch_hash: TxHash,
    pub transaction_hashes: Vec<TxHashSerializeWrapper>,
    pub created_at: DateTime<Utc>,
    pub batch_status: BatchStatus,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchStatus {
    pub updated_at: DateTime<Utc>,
    pub last_state: TxInBlockStatus,
}
