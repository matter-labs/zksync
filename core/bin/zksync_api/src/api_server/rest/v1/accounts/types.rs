//! Data transfer objects used in the accounts API implementation

// Built-in uses
use std::{collections::BTreeMap, fmt::Display, str::FromStr};

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_storage::{
    chain::operations_ext::{
        records::{AccountOpReceiptResponse, AccountTxReceiptResponse},
        SearchDirection as StorageSearchDirection,
    },
    QueryResult, MAX_BLOCK_NUMBER,
};
use zksync_types::{
    tx::TxHash, Account, AccountId, Address, BlockNumber, Nonce, PriorityOp, PubKeyHash, H256,
};
use zksync_utils::BigUintSerdeWrapper;

// Local uses
use crate::{
    api_server::{helpers::remove_prefix, v1::MAX_LIMIT},
    utils::token_db_cache::TokenDBCache,
};

use super::{
    super::{transactions::Receipt, ApiError},
    unable_to_find_token,
};

/// Account search query.
#[derive(Debug, Serialize, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[serde(untagged, rename_all = "camelCase")]
pub enum AccountQuery {
    /// Search account by ID.
    Id(AccountId),
    /// Search account by address.
    Address(Address),
}

/// Account state at the time of the zkSync block commit or verification.
/// This means that each account has various states.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    /// Account wallet balances.
    pub balances: BTreeMap<String, BigUintSerdeWrapper>,
    /// zkSync account nonce.
    pub nonce: Nonce,
    /// Hash of the account's owner public key.
    pub pub_key_hash: PubKeyHash,
}

/// Pending amount for the deposit.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingFunds {
    /// Amount in wei.
    pub amount: BigUintSerdeWrapper,
    /// The greatest block number among all the deposits for a certain token.
    pub expected_accept_block: BlockNumber,
}

/// Depositing balances
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingBalances {
    /// The amount of deposits by token symbols.
    pub balances: BTreeMap<String, DepositingFunds>,
}

/// Account summary info in the zkSync network.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    /// Account address.
    pub address: Address,
    /// Unique identifier of the account in the zkSync network.
    pub id: AccountId,
    /// Account state in accordance with the actual committed block.
    pub committed: AccountState,
    /// Account state in accordance with the actual verified block.
    pub verified: AccountState,
    /// Unconfirmed account deposits.
    pub depositing: DepositingBalances,
}

/// The unique transaction location, which is describes by a pair:
/// (block number, transaction index in it).
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TxLocation {
    /// The block containing the transaction.
    pub block: BlockNumber,
    /// Transaction index in block. Absent for rejected transactions.
    pub index: Option<u32>,
}

/// Account receipts search options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccountReceipts {
    /// Search for older receipts starting from a given location.
    Older(TxLocation),
    /// Search for newer receipts starting from a given location.
    Newer(TxLocation),
    /// Search for latest receipts.
    Latest,
}

impl AccountReceipts {
    pub fn newer_than(block: BlockNumber, index: Option<u32>) -> Self {
        Self::Newer(TxLocation { block, index })
    }

    pub fn older_than(block: BlockNumber, index: Option<u32>) -> Self {
        Self::Older(TxLocation { block, index })
    }
}

/// Direction to perform search of transactions to.
#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SearchDirection {
    /// Find transactions older than specified one.
    Older,
    /// Find transactions newer than specified one.
    Newer,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountReceiptsQuery {
    pub block: Option<BlockNumber>,
    pub index: Option<u32>,
    pub direction: Option<SearchDirection>,
    pub limit: BlockNumber,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountTxReceipt {
    pub index: Option<u32>,
    #[serde(flatten)]
    pub receipt: Receipt,
    pub hash: TxHash,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountOpReceipt {
    pub index: u32,
    #[serde(flatten)]
    pub receipt: Receipt,
    pub hash: H256,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingAccountOpReceipt {
    pub eth_block: u64,
    pub hash: H256,
}

impl From<AccountId> for AccountQuery {
    fn from(v: AccountId) -> Self {
        Self::Id(v)
    }
}

impl From<Address> for AccountQuery {
    fn from(v: Address) -> Self {
        Self::Address(v)
    }
}

impl Display for AccountQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountQuery::Id(id) => id.fmt(f),
            AccountQuery::Address(address) => write!(f, "{:x}", address),
        }
    }
}

impl FromStr for AccountQuery {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = s.parse::<AccountId>() {
            return Ok(Self::Id(id));
        }

        let s = remove_prefix(s);
        s.parse::<Address>()
            .map(Self::Address)
            .map_err(|e| e.to_string())
    }
}

impl Display for SearchDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchDirection::Older => "older".fmt(f),
            SearchDirection::Newer => "newer".fmt(f),
        }
    }
}

impl FromStr for SearchDirection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "older" => Ok(Self::Older),
            "newer" => Ok(Self::Newer),
            other => Err(format!("Unkown search direction: {}", other)),
        }
    }
}

impl AccountState {
    pub(crate) async fn from_storage(
        account: &Account,
        tokens: &TokenDBCache,
    ) -> QueryResult<Self> {
        let mut balances = BTreeMap::new();
        for (token_id, balance) in account.get_nonzero_balances() {
            let token_symbol = tokens
                .token_symbol(token_id)
                .await?
                .ok_or_else(|| unable_to_find_token(token_id))?;

            balances.insert(token_symbol, balance);
        }

        Ok(Self {
            balances,
            nonce: account.nonce,
            pub_key_hash: account.pub_key_hash,
        })
    }
}

impl From<SearchDirection> for StorageSearchDirection {
    fn from(inner: SearchDirection) -> Self {
        match inner {
            SearchDirection::Older => StorageSearchDirection::Older,
            SearchDirection::Newer => StorageSearchDirection::Newer,
        }
    }
}

impl DepositingBalances {
    pub(crate) async fn from_pending_ops(
        ongoing_ops: Vec<PriorityOp>,
        confirmations_for_eth_event: BlockNumber,
        tokens: &TokenDBCache,
    ) -> QueryResult<Self> {
        let mut balances = BTreeMap::new();

        for op in ongoing_ops {
            let received_on_block = op.eth_block;
            let (amount, token_id) = match op.data {
                zksync_types::ZkSyncPriorityOp::Deposit(deposit) => (deposit.amount, deposit.token),
                zksync_types::ZkSyncPriorityOp::FullExit(other) => {
                    panic!("Incorrect input for DepositingBalances: {:?}", other);
                }
            };

            let token_symbol = tokens
                .token_symbol(token_id)
                .await?
                .ok_or_else(|| unable_to_find_token(token_id))?;

            let expected_accept_block =
                received_on_block as BlockNumber + confirmations_for_eth_event;

            let balance = balances
                .entry(token_symbol)
                .or_insert_with(DepositingFunds::default);

            balance.amount.0 += amount;

            // `balance.expected_accept_block` should be the greatest block number among
            // all the deposits for a certain token.
            if expected_accept_block > balance.expected_accept_block {
                balance.expected_accept_block = expected_accept_block;
            }
        }

        Ok(Self { balances })
    }
}

impl AccountReceiptsQuery {
    pub fn new(from: AccountReceipts, limit: u32) -> Self {
        match from {
            AccountReceipts::Older(location) => {
                Self::from_parts(location, SearchDirection::Older, limit)
            }

            AccountReceipts::Newer(location) => {
                Self::from_parts(location, SearchDirection::Newer, limit)
            }

            AccountReceipts::Latest => Self::from_parts(
                TxLocation {
                    block: MAX_BLOCK_NUMBER,
                    index: None,
                },
                SearchDirection::Older,
                limit,
            ),
        }
    }

    fn from_parts(location: TxLocation, direction: SearchDirection, limit: u32) -> Self {
        Self {
            direction: Some(direction),
            block: Some(location.block),
            index: location.index,
            limit,
        }
    }

    pub fn validate(self) -> Result<(TxLocation, SearchDirection, BlockNumber), ApiError> {
        if self.limit == 0 && self.limit > MAX_LIMIT {
            return Err(ApiError::bad_request("Incorrect limit")
                .detail(format!("Limit should be between {} and {}", 1, MAX_LIMIT)));
        }

        let (location, direction) = match (self.block, self.index, self.direction) {
            // Just try to fetch latest transactions.
            (None, None, None) => (
                TxLocation {
                    block: BlockNumber::MAX,
                    index: None,
                },
                SearchDirection::Older,
            ),
            (Some(block), index, Some(direction)) => (TxLocation { block, index }, direction),

            _ => {
                return Err(ApiError::bad_request("Incorrect transaction location")
                    .detail("All parameters must be passed: block, index, direction."))
            }
        };

        Ok((location, direction, self.limit))
    }
}

impl From<AccountTxReceiptResponse> for AccountTxReceipt {
    fn from(inner: AccountTxReceiptResponse) -> Self {
        let block = inner.block_number as BlockNumber;
        let index = inner.block_index.map(|x| x as u32);
        let hash = TxHash::from_slice(&inner.tx_hash).unwrap_or_else(|| {
            panic!(
                "Database provided an incorrect tx_hash field: {}",
                hex::encode(&inner.tx_hash)
            )
        });

        if !inner.success {
            return Self {
                index,
                hash,
                receipt: Receipt::Rejected {
                    reason: inner.fail_reason,
                },
            };
        }

        let receipt = match (
            inner.commit_tx_hash.is_some(),
            inner.verify_tx_hash.is_some(),
        ) {
            (false, false) => Receipt::Executed,
            (true, false) => Receipt::Committed { block },
            (true, true) => Receipt::Verified { block },
            (false, true) => panic!(
                "Database provided an incorrect account tx reciept: {:?}",
                inner
            ),
        };

        Self {
            index,
            receipt,
            hash,
        }
    }
}

impl From<AccountOpReceiptResponse> for AccountOpReceipt {
    fn from(inner: AccountOpReceiptResponse) -> Self {
        let block = inner.block_number as BlockNumber;
        let index = inner.block_index as u32;
        let hash = H256::from_slice(&inner.eth_hash);

        let receipt = match (
            inner.commit_tx_hash.is_some(),
            inner.verify_tx_hash.is_some(),
        ) {
            (false, false) => Receipt::Executed,
            (true, false) => Receipt::Committed { block },
            (true, true) => Receipt::Verified { block },
            (false, true) => panic!(
                "Database provided an incorrect account tx reciept: {:?}",
                inner
            ),
        };

        Self {
            index,
            receipt,
            hash,
        }
    }
}

impl PendingAccountOpReceipt {
    pub fn from_priority_op(op: PriorityOp) -> Self {
        Self {
            eth_block: op.eth_block,
            hash: op.eth_hash,
        }
    }
}
