//! Web3 API types definitions.
//!
//! Most of the types are re-exported from the `web3` crate, but some of them maybe extended with
//! new variants (enums) or optional fields (structures).
//!
//! These "extensions" are required to provide more zkSync-specific information while remaining Web3-compilant.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
pub use web3::types::{Address, Block, Transaction, H160, H256, H64, U256, U64};

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
    pub block_number: u32,
    pub block_index: Option<u32>,
    pub from: H160,
    pub to: Option<H160>,
    pub nonce: u32,
    pub tx_hash: H256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockInfo {
    Block(Block<H256>),
    BlockWithTxs(Block<Transaction>),
}
