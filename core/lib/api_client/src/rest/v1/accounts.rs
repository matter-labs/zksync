//! Accounts API client implementation

// Built-in uses
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
    str::FromStr,
};

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{
    tx::TxHash, AccountId, Address, BlockNumber, Nonce, PriorityOp, PubKeyHash, TokenId, H256,
};
use zksync_utils::{remove_prefix, BigUintSerdeWrapper};

// Local uses
use super::{
    client::{Client, ClientError},
    transactions::Receipt,
};

// Data transfer objects

/// Account search query.
#[derive(Debug, Serialize, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[serde(untagged, rename_all = "camelCase")]
pub enum AccountQuery {
    /// Search account by ID.
    Id(AccountId),
    /// Search account by address.
    Address(Address),
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct NFT {
    id: TokenId,
    content_hash: H256,
    creator_id: AccountId,
    creator_address: Address,
    serial_id: u32,
    address: Address,
    symbol: String,
}

impl From<zksync_types::NFT> for NFT {
    fn from(val: zksync_types::NFT) -> Self {
        Self {
            id: val.id,
            content_hash: val.content_hash,
            creator_id: val.creator_id,
            creator_address: val.creator_address,
            serial_id: val.serial_id,
            address: val.address,
            symbol: val.symbol,
        }
    }
}
/// Account state at the time of the zkSync block commit or verification.
/// This means that each account has various states.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    /// Account wallet balances.
    pub balances: BTreeMap<String, BigUintSerdeWrapper>,
    pub nfts: HashMap<TokenId, NFT>,
    pub minted_nfts: HashMap<TokenId, NFT>,
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
        if let Ok(id) = s.parse() {
            return Ok(Self::Id(AccountId(id)));
        }

        let s = remove_prefix(s);
        s.parse().map(Self::Address).map_err(|e| e.to_string())
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
            other => Err(format!("Unknown search direction: {}", other)),
        }
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
                    // TODO: use `zksync_storage::MAX_BLOCK_NUMBER` instead
                    block: BlockNumber(u32::MAX),
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
            limit: BlockNumber(limit),
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

/// Accounts API part.
impl Client {
    /// Gets account information
    pub async fn account_info(
        &self,
        account: impl Into<AccountQuery>,
    ) -> Result<Option<AccountInfo>, ClientError> {
        let account = account.into();

        self.get(&format!("accounts/{}", account)).send().await
    }

    pub async fn account_tx_receipts(
        &self,
        account: impl Into<AccountQuery>,
        from: AccountReceipts,
        limit: u32,
    ) -> Result<Vec<AccountTxReceipt>, ClientError> {
        let account = account.into();

        self.get(&format!("accounts/{}/transactions/receipts", account))
            .query(&AccountReceiptsQuery::new(from, limit))
            .send()
            .await
    }

    pub async fn account_op_receipts(
        &self,
        account: impl Into<AccountQuery>,
        from: AccountReceipts,
        limit: u32,
    ) -> Result<Vec<AccountOpReceipt>, ClientError> {
        let account = account.into();

        self.get(&format!("accounts/{}/operations/receipts", account))
            .query(&AccountReceiptsQuery::new(from, limit))
            .send()
            .await
    }

    pub async fn account_pending_ops(
        &self,
        account: impl Into<AccountQuery>,
    ) -> Result<Vec<PendingAccountOpReceipt>, ClientError> {
        let account = account.into();

        self.get(&format!("accounts/{}/operations/pending", account))
            .send()
            .await
    }
}
