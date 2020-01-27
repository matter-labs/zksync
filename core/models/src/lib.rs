#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

pub mod abi;
pub mod circuit;
pub mod config_options;
pub mod merkle_tree;
pub mod node;
pub mod params;
pub mod primitives;

// TODO: refactor, find new home for all this stuff

use crate::node::block::Block;
use crate::node::AccountUpdates;
use crate::node::BlockNumber;
use ethabi::{decode, ParamType};
use failure::format_err;
use serde_bytes;
use std::convert::TryFrom;
use web3::types::{Address, Log, U256};

use std::fmt;

#[derive(Clone, Copy, Debug)]
pub struct GenericFrHolder<F: franklin_crypto::bellman::pairing::ff::PrimeField>(pub F);

impl<F: franklin_crypto::bellman::pairing::ff::PrimeField> serde::Serialize for GenericFrHolder<F> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        let hex = to_hex(self.0);
        serializer.serialize_str(&format!("0x{}", hex))
    }
}

pub fn to_hex<F: franklin_crypto::bellman::pairing::ff::PrimeField>(value: &F) -> String {
    use franklin_crypto::bellman::pairing::ff::PrimeFieldRepr;

    let mut buf: Vec<u8> = vec![];
    value.into_repr().write_be(&mut buf).unwrap();
    hex::encode(&buf)
}

struct ReprVisitor<F: franklin_crypto::bellman::pairing::ff::PrimeField> {
    _marker: std::marker::PhantomData<F>
}

pub fn from_hex<F: franklin_crypto::bellman::pairing::ff::PrimeField>(value: &str) -> Result<F, String> {
    use franklin_crypto::bellman::pairing::ff::PrimeFieldRepr;
    
    let value = if value.starts_with("0x") { &value[2..] } else { value };
    if value.len() % 2 != 0 {return Err(format!("hex length must be even for full byte encoding: {}", value))}
    let mut buf = hex::decode(&value).map_err(|_| format!("could not decode hex: {}", value))?;
    buf.reverse();
    let mut repr = F::Repr::default();
    buf.resize(repr.as_ref().len() * 8, 0);
    repr.read_le(&buf[..]).map_err(|e| format!("could not read {}: {}", value, &e))?;
    F::from_repr(repr).map_err(|e| format!("could not convert into prime field: {}: {}", value, &e))
}

impl<'de, F: franklin_crypto::bellman::pairing::ff::PrimeField> serde::de::Visitor<'de> for ReprVisitor<F> {
    type Value = GenericFrHolder<F>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a hex string with prefix: 0x012ab...")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: ::serde::de::Error,
    {
        let value = from_hex::<F>(&value[2..]).map_err(|e| E::custom(e))?;

        Ok(GenericFrHolder(value))
    }
}

impl<'de, F: franklin_crypto::bellman::pairing::ff::PrimeField> serde::Deserialize<'de> for GenericFrHolder<F> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(ReprVisitor::<F> {
            _marker: std::marker::PhantomData
        })
    }
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
