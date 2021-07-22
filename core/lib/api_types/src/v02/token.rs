use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use zksync_types::{AccountId, Address, Token, TokenId, H256};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiToken {
    pub id: TokenId,
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub enabled_for_fees: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenPrice {
    pub token_id: TokenId,
    pub token_symbol: String,
    pub price_in: String,
    pub decimals: u8,
    pub price: BigDecimal,
}

impl ApiToken {
    pub fn from_token_and_eligibility(token: Token, eligibility: bool) -> Self {
        ApiToken {
            id: token.id,
            address: token.address,
            symbol: token.symbol,
            decimals: token.decimals,
            enabled_for_fees: eligibility,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct NFT {
    pub id: TokenId,
    pub content_hash: H256,
    pub creator_id: AccountId,
    pub creator_address: Address,
    pub serial_id: u32,
    pub address: Address,
    pub symbol: String,
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiNFT {
    pub id: TokenId,
    pub content_hash: H256,
    pub creator_id: AccountId,
    pub creator_address: Address,
    pub serial_id: u32,
    pub address: Address,
    pub symbol: String,
    pub current_factory: Address,
    pub withdrawn_factory: Option<Address>,
}
