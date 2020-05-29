// External imports
use crate::schema::*;
use crate::utils::StoredBigUint;

#[derive(Debug, Identifiable, Insertable, QueryableByName, Queryable)]
#[table_name = "accounts"]
pub struct StorageAccount {
    pub id: i64,
    pub last_block: i64,
    pub nonce: i64,
    pub address: Vec<u8>,
    pub pubkey_hash: Vec<u8>,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "account_creates"]
pub struct StorageAccountCreation {
    pub account_id: i64,
    pub is_create: bool,
    pub block_number: i64,
    pub address: Vec<u8>,
    pub nonce: i64,
    pub update_order_id: i32,
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "account_balance_updates"]
pub struct StorageAccountUpdate {
    pub balance_update_id: i32,
    pub account_id: i64,
    pub block_number: i64,
    pub coin_id: i32,
    pub old_balance: StoredBigUint,
    pub new_balance: StoredBigUint,
    pub old_nonce: i64,
    pub new_nonce: i64,
    pub update_order_id: i32,
}

#[derive(Debug, Insertable)]
#[table_name = "account_balance_updates"]
pub struct StorageAccountUpdateInsert {
    pub update_order_id: i32,
    pub account_id: i64,
    pub block_number: i64,
    pub coin_id: i32,
    pub old_balance: StoredBigUint,
    pub new_balance: StoredBigUint,
    pub old_nonce: i64,
    pub new_nonce: i64,
}

#[derive(Debug, Insertable)]
#[table_name = "account_pubkey_updates"]
pub struct StorageAccountPubkeyUpdateInsert {
    pub update_order_id: i32,
    pub account_id: i64,
    pub block_number: i64,
    pub old_pubkey_hash: Vec<u8>,
    pub new_pubkey_hash: Vec<u8>,
    pub old_nonce: i64,
    pub new_nonce: i64,
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "account_pubkey_updates"]
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

#[derive(Debug, Identifiable, Insertable, QueryableByName, Queryable, Associations)]
#[belongs_to(StorageAccount, foreign_key = "account_id")]
#[primary_key(account_id, coin_id)]
#[table_name = "balances"]
pub struct StorageBalance {
    pub account_id: i64,
    pub coin_id: i32,
    pub balance: StoredBigUint,
}
