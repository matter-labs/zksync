//! Web3 API types definitions.
//!
//! Most of the types are re-exported from the `web3` crate, but some of them maybe extended with
//! new variants (enums) or optional fields (structures).
//!
//! These "extensions" are required to provide more zkSync-specific information while remaining Web3-compilant.

// Built-in uses
use std::fmt;
use std::marker::PhantomData;
// External uses
use itertools::unfold;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
pub use web3::types::{
    Block, Bytes, Log, Transaction, TransactionReceipt, H160, H2048, H256, H64, U256, U64,
};
// Workspace uses
use zksync_storage::chain::operations_ext::records::Web3TxData;

/// Block Number
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BlockNumber {
    /// Last block that was committed on L1.
    Committed,
    /// Last block that was finalized on L1.
    Finalized,
    /// Latest block (may be the block that is currently open).
    Latest,
    /// Earliest block (genesis)
    Earliest,
    /// Alias for `BlockNumber::Latest`.
    Pending,
    /// Block by number from canon chain
    Number(U64),
}

impl Serialize for BlockNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            BlockNumber::Number(ref x) => serializer.serialize_str(&format!("0x{:x}", x)),
            BlockNumber::Committed => serializer.serialize_str("committed"),
            BlockNumber::Finalized => serializer.serialize_str("finalized"),
            BlockNumber::Latest => serializer.serialize_str("latest"),
            BlockNumber::Earliest => serializer.serialize_str("earliest"),
            BlockNumber::Pending => serializer.serialize_str("pending"),
        }
    }
}

impl<'de> Deserialize<'de> for BlockNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = BlockNumber;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("A block number or one of the supported aliases")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                let result = match value {
                    "committed" => BlockNumber::Committed,
                    "finalized" => BlockNumber::Finalized,
                    "latest" => BlockNumber::Latest,
                    "earliest" => BlockNumber::Earliest,
                    "pending" => BlockNumber::Pending,
                    num => {
                        let number =
                            U64::deserialize(de::value::BorrowedStrDeserializer::new(num))?;
                        BlockNumber::Number(number)
                    }
                };

                Ok(result)
            }
        }
        deserializer.deserialize_str(V)
    }
}

#[derive(Debug, Clone)]
pub struct TxData {
    pub block_hash: H256,
    pub block_number: u32,
    pub block_index: Option<u32>,
    pub from: H160,
    pub to: Option<H160>,
    pub nonce: u32,
    pub tx_hash: H256,
}

impl From<Web3TxData> for TxData {
    fn from(tx: Web3TxData) -> TxData {
        TxData {
            block_hash: H256::from_slice(&tx.block_hash),
            block_number: tx.block_number as u32,
            block_index: tx.block_index.map(|i| i as u32),
            from: H160::from_slice(&tx.from_account),
            to: tx.to_account.map(|to| H160::from_slice(&to)),
            nonce: tx.nonce as u32,
            tx_hash: H256::from_slice(&tx.tx_hash),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum BlockInfo {
    BlockWithHashes(Block<H256>),
    BlockWithTxs(Block<Transaction>),
}

impl BlockInfo {
    fn new_block<T>(
        hash: H256,
        parent_hash: H256,
        block_number: zksync_types::BlockNumber,
        timestamp: u64,
        transactions: Vec<T>,
    ) -> Block<T> {
        Block {
            hash: Some(hash),
            parent_hash,
            uncles_hash: H256::zero(),
            author: H160::zero(),
            state_root: hash,
            transactions_root: hash,
            receipts_root: hash,
            number: Some(block_number.0.into()),
            gas_used: 0.into(),
            gas_limit: 50000.into(),
            extra_data: Vec::new().into(),
            logs_bloom: None,
            timestamp: timestamp.into(),
            difficulty: 0.into(),
            total_difficulty: Some(0.into()),
            seal_fields: Vec::new(),
            uncles: Vec::new(),
            transactions,
            size: None,
            mix_hash: Some(H256::zero()),
            nonce: Some(H64::zero()),
        }
    }

    pub fn new_with_hashes(
        hash: H256,
        parent_hash: H256,
        block_number: zksync_types::BlockNumber,
        timestamp: u64,
        transactions: Vec<H256>,
    ) -> Self {
        Self::BlockWithHashes(Self::new_block(
            hash,
            parent_hash,
            block_number,
            timestamp,
            transactions,
        ))
    }

    pub fn new_with_txs(
        hash: H256,
        parent_hash: H256,
        block_number: zksync_types::BlockNumber,
        timestamp: u64,
        transactions: Vec<Transaction>,
    ) -> Self {
        Self::BlockWithTxs(Self::new_block(
            hash,
            parent_hash,
            block_number,
            timestamp,
            transactions,
        ))
    }
}

/// Either value or array of values.
#[derive(Default, Debug, PartialEq, Clone)]
pub struct ValueOrArray<T>(pub Vec<T>);

impl<'de, T: fmt::Debug + Deserialize<'de>> ::serde::Deserialize<'de> for ValueOrArray<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        struct Visitor<T>(PhantomData<T>);

        impl<'de, T: fmt::Debug + Deserialize<'de>> de::Visitor<'de> for Visitor<T> {
            type Value = ValueOrArray<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("Expected value or sequence")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                use serde::de::IntoDeserializer;

                Deserialize::deserialize(value.into_deserializer())
                    .map(|value| ValueOrArray(vec![value]))
            }

            fn visit_seq<S>(self, visitor: S) -> Result<Self::Value, S::Error>
            where
                S: de::SeqAccess<'de>,
            {
                unfold(visitor, |vis| vis.next_element().transpose())
                    .collect::<Result<_, _>>()
                    .map(ValueOrArray)
            }
        }

        deserializer.deserialize_any(Visitor(PhantomData))
    }
}

impl<T> Serialize for ValueOrArray<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0.len() {
            0 => serializer.serialize_none(),
            1 => Serialize::serialize(&self.0[0], serializer),
            _ => Serialize::serialize(&self.0, serializer),
        }
    }
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    pub from_block: Option<BlockNumber>,
    pub to_block: Option<BlockNumber>,
    pub address: Option<ValueOrArray<H160>>,
    pub topics: Option<Vec<Option<ValueOrArray<H256>>>>,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Event {
    ZkSyncTransfer,
    ZkSyncWithdraw,
    ZkSyncForcedExit,
    ZkSyncChangePubKey,
    ZkSyncDeposit,
    ZkSyncFullExit,
    ZkSyncMintNFT,
    ZkSyncWithdrawNFT,
    ZkSyncSwap,
    ERCTransfer, // ERC20 and ERC721 transfers have same topics
}

#[derive(Debug, Clone, Copy)]
pub struct CommonLogData {
    pub block_hash: Option<H256>,
    pub block_number: Option<U64>,
    pub transaction_hash: H256,
    pub transaction_index: Option<U64>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
    pub from: Option<H160>,
    pub to: H160,
    pub gas: Option<U256>,
    pub gas_price: Option<U256>,
    pub value: Option<U256>,
    pub data: Option<Bytes>,
}
