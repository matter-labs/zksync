use super::token::NFT;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zksync_types::{AccountId, Address, BlockNumber, Nonce, PubKeyHash, TokenId};
use zksync_utils::BigUintSerdeWrapper;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    pub committed: Option<Account>,
    pub finalized: Option<Account>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account_id: AccountId,
    pub address: Address,
    pub nonce: Nonce,
    pub pub_key_hash: PubKeyHash,
    pub last_update_in_block: BlockNumber,
    pub balances: BTreeMap<String, BigUintSerdeWrapper>,
    pub nfts: BTreeMap<TokenId, NFT>,
    pub minted_nfts: BTreeMap<TokenId, NFT>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AccountAddressOrId {
    Address(Address),
    Id(AccountId),
}
