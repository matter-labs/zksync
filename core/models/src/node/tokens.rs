use crate::config_options::parse_env;
use crate::node::{Address, TokenId};
use crate::primitives::UnsignedRatioSerializeAsDecimal;
use chrono::{DateTime, Utc};
use num::{rational::Ratio, BigUint};
use std::fs::read_to_string;
use std::path::PathBuf;

/// Order of the fields are important (from more specific types to less specific types)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(untagged, rename_all = "camelCase")]
pub enum TokenLike {
    Id(TokenId),
    Address(Address),
    Symbol(String),
}

impl From<TokenId> for TokenLike {
    fn from(id: TokenId) -> Self {
        Self::Id(id)
    }
}

/// Token supported in zkSync protocol
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Token {
    /// id is used for tx signature and serialization
    pub id: TokenId,
    /// Contract address of ERC20 token or Address::zero() for "ETH"
    pub address: Address,
    /// Token symbol (e.g. "ETH" or "USDC")
    pub symbol: String,
    /// Token precision (e.g. 18 for "ETH" and some ERC20-tokens)
    pub precision: u8,
}

/// Tokens that added when deploying contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenGenesisListItem {
    /// Address (prefixed with 0x)
    pub address: String,
    /// precision (18 for default ETH-like tokens)
    pub precision: u8,
    /// Token symbol
    pub symbol: String,
}

impl Token {
    pub fn new(id: TokenId, address: Address, symbol: &str, precision: u8) -> Self {
        Self {
            id,
            address,
            symbol: symbol.to_string(),
            precision,
        }
    }
}

pub fn get_genesis_token_list(network: &str) -> Result<Vec<TokenGenesisListItem>, failure::Error> {
    let mut file_path = parse_env::<PathBuf>("ZKSYNC_HOME");
    file_path.push("etc");
    file_path.push("tokens");
    file_path.push(network);
    file_path.set_extension("json");
    Ok(serde_json::from_str(&read_to_string(file_path)?)?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPrice {
    #[serde(with = "UnsignedRatioSerializeAsDecimal")]
    pub usd_price: Ratio<BigUint>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq)]
pub enum TxFeeTypes {
    Withdraw,
    Transfer,
}
