use crate::config_options::parse_env;
use crate::node::{Address, TokenId};
use std::fs::read_to_string;
use std::path::PathBuf;

/// Order of the fields are important (from more specific types to less specific types)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
    pub fn new(id: TokenId, address: Address, symbol: &str) -> Self {
        Self {
            id,
            address,
            symbol: symbol.to_string(),
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
