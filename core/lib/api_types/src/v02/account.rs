use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zksync_types::{AccountId, Address, BlockNumber, Nonce, PubKeyHash};
use zksync_utils::BigUintSerdeWrapper;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Account {
    pub account_id: AccountId,
    pub address: Address,
    pub nonce: Nonce,
    pub pub_key_hash: PubKeyHash,
    pub last_update_in_block: BlockNumber,
    pub balances: BTreeMap<String, BigUintSerdeWrapper>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum AccountStateType {
    Committed,
    Finalized,
}
