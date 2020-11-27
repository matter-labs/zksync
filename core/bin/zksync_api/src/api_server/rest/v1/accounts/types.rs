//! Data transfer objects used in the accounts API implementation

// Built-in uses
use std::{collections::BTreeMap, fmt::Display, str::FromStr};

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_storage::{
    chain::{
        account::AccountQuery as StorageAccountQuery,
        operations_ext::{
            records::TransactionsHistoryItem, SearchDirection as StorageSearchDirection,
        },
    },
    QueryResult,
};
use zksync_types::{
    tx::TxHash, Account, AccountId, Address, BlockNumber, Nonce, PriorityOp, PubKeyHash,
};
use zksync_utils::BigUintSerdeWrapper;

// Local uses
use crate::{
    api_server::{rest::helpers::remove_prefix, v1::MAX_LIMIT},
    core_api_client::EthBlockId,
    utils::token_db_cache::TokenDBCache,
};

use super::{
    super::{transactions::TxReceipt, ApiError},
    unable_to_find_token,
};

#[derive(Debug, Serialize, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[serde(untagged, rename_all = "camelCase")]
pub enum AccountQuery {
    Id(AccountId),
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingFunds {
    pub amount: BigUintSerdeWrapper,
    /// The greatest block number among all the deposits for a certain token.
    pub expected_accept_block: BlockNumber,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingBalances {
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
    /// Account state in according of the actual committed block.
    pub committed: AccountState,
    /// Account state in according of the actual verified block.
    pub verified: AccountState,
    /// Unconfirmed account deposits.
    pub depositing: DepositingBalances,
}

/// The unique transaction location, which is describes by a pair:
/// (block number, transaction index in it).
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
pub struct TxLocation {
    /// The block containing the transaction.
    pub block: BlockNumber,
    /// Transaction index in block.
    pub index: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccountReceipts {
    Older(TxLocation),
    Newer(TxLocation),
    Latest,
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
    pub index: Option<u64>,
    pub direction: Option<SearchDirection>,
    pub limit: BlockNumber,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountTxReceipt {
    #[serde(flatten)]
    pub location: TxLocation,
    pub receipt: TxReceipt,
    pub hash: Option<TxHash>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingAccountTxReceipt {
    block: u64,
    // TODO find proper type.
    hash: TxHash,
}

impl From<AccountQuery> for StorageAccountQuery {
    fn from(query: AccountQuery) -> Self {
        match query {
            AccountQuery::Id(id) => StorageAccountQuery::Id(id),
            AccountQuery::Address(address) => StorageAccountQuery::Address(address),
        }
    }
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
            AccountQuery::Address(address) => address.fmt(f),
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
        ongoing_ops: Vec<(EthBlockId, PriorityOp)>,
        confirmations_for_eth_event: BlockNumber,
        tokens: &TokenDBCache,
    ) -> QueryResult<Self> {
        let mut balances = BTreeMap::new();

        for (received_on_block, op) in ongoing_ops {
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
                Self::from_parts(location, SearchDirection::Newer, limit)
            }

            AccountReceipts::Newer(location) => {
                Self::from_parts(location, SearchDirection::Newer, limit)
            }

            AccountReceipts::Latest => Self::from_parts(
                TxLocation {
                    block: BlockNumber::MAX,
                    index: 0,
                },
                SearchDirection::Newer,
                limit,
            ),
        }
    }

    fn from_parts(location: TxLocation, direction: SearchDirection, limit: u32) -> Self {
        Self {
            direction: Some(direction),
            block: Some(location.block),
            index: Some(location.index),
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
                    index: 0,
                },
                SearchDirection::Older,
            ),
            (Some(block), Some(index), Some(direction)) => (TxLocation { block, index }, direction),

            _ => {
                return Err(ApiError::bad_request("Incorrect transaction location")
                    .detail("All parameters must be passed: block, index, direction."))
            }
        };

        Ok((location, direction, self.limit))
    }
}

impl TxLocation {
    fn from_tx_id(tx_id: &str) -> Option<Self> {
        let mut iter = tx_id.splitn(2, ',').filter_map(|x| x.parse::<u64>().ok());

        let block = iter.next()? as BlockNumber;
        let index = iter.next()?;

        Some(TxLocation { block, index })
    }
}

impl From<TransactionsHistoryItem> for AccountTxReceipt {
    fn from(inner: TransactionsHistoryItem) -> Self {
        let location = TxLocation::from_tx_id(&inner.tx_id)
            .unwrap_or_else(|| panic!("Database provided an incorrect transaction ID"));

        let hash = inner.hash.map(|s| {
            TxHash::from_str(&s).unwrap_or_else(|err| {
                panic!("Database provided an incorrect transaction hash: {}", err)
            })
        });

        // TODO Ask for correctness.
        let receipt = match (inner.success, inner.verified) {
            (None, _) => TxReceipt::Executed,
            (Some(false), _) => TxReceipt::Rejected {
                reason: inner.fail_reason,
            },
            (Some(true), false) => TxReceipt::Committed {
                block: location.block,
            },
            (Some(true), true) => TxReceipt::Verified {
                block: location.block,
            },
        };

        Self {
            location,
            receipt,
            hash,
        }
    }
}

impl PendingAccountTxReceipt {
    pub fn from_priority_op(block_id: EthBlockId, op: PriorityOp) -> Self {
        let hash = TxHash::from_slice(&op.eth_hash).expect("Incorrect hash sent by eth_watch");

        Self {
            block: block_id,
            hash,
        }
    }
}
