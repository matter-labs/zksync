// External imports
use serde::{Deserialize, Serialize};
use sqlx::{types::BigDecimal, FromRow};
// Workspace imports
// Local imports
use crate::utils::{address_to_stored_string, stored_str_address_to_address};
use chrono::{DateTime, Utc};
use zksync_types::tokens::{TokenMarketVolume, TokenPrice};
use zksync_types::{AccountId, Address, Token, TokenId, H256, NFT};
use zksync_utils::big_decimal_to_ratio;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, FromRow)]
pub struct DbToken {
    pub id: i32,
    pub address: String,
    pub symbol: String,
    pub decimals: i16,
    pub is_nft: bool,
}

impl From<Token> for DbToken {
    fn from(token: Token) -> Self {
        Self {
            id: *token.id as i32,
            address: address_to_stored_string(&token.address),
            symbol: token.symbol,
            decimals: token.decimals as i16,
            is_nft: token.is_nft,
        }
    }
}

impl From<DbToken> for Token {
    fn from(val: DbToken) -> Token {
        Token {
            id: TokenId(val.id as u32),
            address: stored_str_address_to_address(&val.address),
            symbol: val.symbol,
            decimals: val.decimals as u8,
            is_nft: val.is_nft,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct DbTickerPrice {
    pub token_id: i32,
    pub usd_price: BigDecimal,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct StorageNFT {
    // Unique token id in zksync
    pub token_id: i32,
    // Counter of generated tokens for the creator
    // Required to enforce uniqueness of address
    pub serial_id: i32,
    pub creator_account_id: i32,
    pub creator_address: Vec<u8>,
    pub address: Vec<u8>,
    pub content_hash: Vec<u8>,
}

impl From<DbTickerPrice> for TokenPrice {
    fn from(val: DbTickerPrice) -> Self {
        Self {
            usd_price: big_decimal_to_ratio(&val.usd_price).expect("Price could not be negative"),
            last_updated: val.last_updated,
        }
    }
}

impl From<StorageNFT> for NFT {
    fn from(val: StorageNFT) -> Self {
        Self {
            id: TokenId(val.token_id as u32),
            serial_id: val.serial_id as u32,
            creator_address: Address::from_slice(val.creator_address.as_slice()),
            creator_id: AccountId(val.creator_account_id as u32),
            address: Address::from_slice(val.address.as_slice()),
            symbol: "".to_string(),
            content_hash: H256::from_slice(val.content_hash.as_slice()),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct DBMarketVolume {
    pub token_id: i32,
    pub market_volume: BigDecimal,
    pub last_updated: DateTime<Utc>,
}

impl From<DBMarketVolume> for TokenMarketVolume {
    fn from(val: DBMarketVolume) -> Self {
        Self {
            market_volume: big_decimal_to_ratio(&val.market_volume)
                .expect("Price could not be negative"),
            last_updated: val.last_updated,
        }
    }
}
