// External imports
use sqlx::{types::BigDecimal, FromRow};
use zksync_types::{AccountId, Address, TokenId, H256, NFT};

#[derive(Debug, FromRow)]
pub struct StorageAccount {
    pub id: i64,
    pub last_block: i64,
    pub nonce: i64,
    pub address: Vec<u8>,
    pub pubkey_hash: Vec<u8>,
}

#[derive(Debug, FromRow)]
pub struct StorageAccountCreation {
    pub account_id: i64,
    pub is_create: bool,
    pub block_number: i64,
    pub address: Vec<u8>,
    pub nonce: i64,
    pub update_order_id: i32,
}

#[derive(Debug, FromRow)]
pub struct StorageAccountUpdate {
    pub balance_update_id: i32,
    pub account_id: i64,
    pub block_number: i64,
    pub coin_id: i32,
    pub old_balance: BigDecimal,
    pub new_balance: BigDecimal,
    pub old_nonce: i64,
    pub new_nonce: i64,
    pub update_order_id: i32,
}

#[derive(Debug, FromRow)]
pub struct StorageMintNFTUpdate {
    pub token_id: i32,
    pub serial_id: i32,
    pub creator_account_id: i32,
    pub creator_address: Vec<u8>,
    pub address: Vec<u8>,
    pub content_hash: Vec<u8>,
    pub update_order_id: i32,
    pub block_number: i64,
    pub symbol: String,
}

impl From<StorageMintNFTUpdate> for NFT {
    fn from(val: StorageMintNFTUpdate) -> Self {
        Self {
            id: TokenId(val.token_id as u32),
            serial_id: val.serial_id as u32,
            creator_address: Address::from_slice(val.creator_address.as_slice()),
            creator_id: AccountId(val.creator_account_id as u32),
            address: Address::from_slice(val.address.as_slice()),
            symbol: val.symbol,
            content_hash: H256::from_slice(val.content_hash.as_slice()),
        }
    }
}

#[derive(Debug, FromRow)]
pub struct StorageAccountPubkeyUpdate {
    pub pubkey_update_id: i32,
    pub update_order_id: i32,
    pub account_id: i64,
    pub block_number: i64,
    pub old_pubkey_hash: Vec<u8>,
    pub new_pubkey_hash: Vec<u8>,
    pub old_nonce: i64,
    pub new_nonce: i64,
}

#[derive(Debug, FromRow, Clone)]
pub struct StorageBalance {
    pub account_id: i64,
    pub coin_id: i32,
    pub balance: BigDecimal,
}

#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(rename = "eth_account_type")]
pub enum EthAccountType {
    Owned,
    CREATE2,
}

#[derive(Debug, Clone, FromRow)]
pub struct StorageAccountType {
    pub account_id: i64,
    pub account_type: EthAccountType,
}
