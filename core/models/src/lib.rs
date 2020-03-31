#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

pub mod abi;
pub mod circuit;
pub mod config_options;
pub mod ethereum;
pub mod merkle_tree;
pub mod misc;
pub mod node;
pub mod params;
pub mod primitives;
pub mod prover_utils;
pub mod serialization;

// TODO: refactor, find new home for all this stuff

pub use crypto_exports::franklin_crypto;
pub use crypto_exports::rand;

use crate::node::block::Block;
use crate::node::AccountUpdates;
use crate::node::BlockNumber;
use ethabi::{decode, ParamType};
use failure::format_err;
use franklin_crypto::bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
use serde_bytes;
use std::convert::TryFrom;
use web3::types::{Address, Log, U256};

/// Returns hex representation of the field element without `0x` prefix.
pub fn fe_to_hex<F: PrimeField>(value: &F) -> String {
    let mut buf: Vec<u8> = Vec::with_capacity(32);
    value.into_repr().write_be(&mut buf).unwrap();
    hex::encode(&buf)
}

pub fn fe_from_hex<F: PrimeField>(value: &str) -> Result<F, failure::Error> {
    let value = if value.starts_with("0x") {
        &value[2..]
    } else {
        value
    };

    let mut buf = hex::decode(&value)
        .map_err(|e| format_err!("could not decode hex: {}, reason: {}", value, e))?;
    buf.reverse();
    let mut repr = F::Repr::default();
    buf.resize(repr.as_ref().len() * 8, 0);
    repr.read_le(&buf[..])
        .map_err(|e| format_err!("could not read {}: {}", value, e))?;
    F::from_repr(repr)
        .map_err(|e| format_err!("could not convert into prime field: {}: {}", value, e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMeta {
    pub addr: String,
    pub nonce: u32,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct NetworkStatus {
    pub next_block_at_max: Option<u64>,
    pub last_committed: BlockNumber,
    pub last_verified: BlockNumber,
    pub total_transactions: u32,
    pub outstanding_txs: u32,
}

pub type EncodedProof = [U256; 8];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub struct EthBlockData {
    #[serde(with = "serde_bytes")]
    public_data: Vec<u8>,
}

pub struct ProverRequest(pub BlockNumber);

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    Commit,
    Verify { proof: Box<EncodedProof> },
}

impl Action {
    pub fn get_type(&self) -> ActionType {
        match self {
            Action::Commit => ActionType::COMMIT,
            Action::Verify { .. } => ActionType::VERIFY,
        }
    }
}

impl std::string::ToString for Action {
    fn to_string(&self) -> String {
        self.get_type().to_string()
    }
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: Option<i64>,
    pub action: Action,
    pub block: Block,
    pub accounts_updated: AccountUpdates,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitRequest {
    pub block: Block,
    pub accounts_updated: AccountUpdates,
}

pub const ACTION_COMMIT: &str = "COMMIT";
pub const ACTION_VERIFY: &str = "VERIFY";

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub enum ActionType {
    COMMIT,
    VERIFY,
}

impl std::string::ToString for ActionType {
    fn to_string(&self) -> String {
        match self {
            ActionType::COMMIT => ACTION_COMMIT.to_owned(),
            ActionType::VERIFY => ACTION_VERIFY.to_owned(),
        }
    }
}

impl std::str::FromStr for ActionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ACTION_COMMIT => Ok(Self::COMMIT),
            ACTION_VERIFY => Ok(Self::VERIFY),
            _ => Err(format!(
                "Should be either: {} or {}",
                ACTION_COMMIT, ACTION_VERIFY
            )),
        }
    }
}

#[derive(Debug)]
pub struct TokenAddedEvent {
    pub address: Address,
    pub id: u32,
}

impl TryFrom<Log> for TokenAddedEvent {
    type Error = failure::Error;

    fn try_from(event: Log) -> Result<TokenAddedEvent, failure::Error> {
        let mut dec_ev = decode(&[ParamType::Address, ParamType::Uint(32)], &event.data.0)
            .map_err(|e| format_err!("Event data decode: {:?}", e))?;
        Ok(TokenAddedEvent {
            address: dec_ev.remove(0).to_address().unwrap(),
            id: dec_ev
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u32)
                .unwrap(),
        })
    }
}
