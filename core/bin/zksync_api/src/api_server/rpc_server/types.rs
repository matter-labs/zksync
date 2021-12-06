use std::collections::HashMap;

// External uses
use jsonrpc_core::{Error, Result};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_api_types::v02::{
    account::{DepositingAccountBalances, EthAccountType},
    token::NFT,
};
use zksync_crypto::params::{MIN_NFT_TOKEN_ID, NFT_TOKEN_ID_VAL};
use zksync_storage::StorageProcessor;
use zksync_types::{Account, AccountId, Address, Nonce, PubKeyHash, TokenId};
use zksync_utils::BigUintSerdeWrapper;

// Local uses
use crate::utils::token_db_cache::TokenDBCache;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAccountState {
    pub balances: HashMap<String, BigUintSerdeWrapper>,
    pub nfts: HashMap<TokenId, NFT>,
    pub minted_nfts: HashMap<TokenId, NFT>,
    pub nonce: Nonce,
    pub pub_key_hash: PubKeyHash,
}

impl ResponseAccountState {
    pub async fn try_restore(
        storage: &mut StorageProcessor<'_>,
        tokens: &TokenDBCache,
        account: Account,
    ) -> Result<Self> {
        let mut balances = HashMap::new();
        let mut nfts = HashMap::new();
        for (token_id, balance) in account.get_nonzero_balances() {
            match token_id.0 {
                NFT_TOKEN_ID_VAL => {
                    // Don't include special token to balances or nfts
                }
                MIN_NFT_TOKEN_ID..=NFT_TOKEN_ID_VAL => {
                    // https://github.com/rust-lang/rust/issues/37854
                    // Exclusive range is an experimental feature, but we have already checked the last value in the previous step
                    nfts.insert(
                        token_id,
                        tokens
                            .get_nft_by_id(storage, token_id)
                            .await
                            .map_err(|_| Error::internal_error())?
                            .ok_or_else(Error::internal_error)?
                            .into(),
                    );
                }
                _ => {
                    let token_symbol = tokens
                        .token_symbol(storage, token_id)
                        .await
                        .map_err(|_| Error::internal_error())?
                        .ok_or_else(Error::internal_error)?;
                    balances.insert(token_symbol, balance);
                }
            }
        }
        let minted_nfts = account
            .minted_nfts
            .iter()
            .map(|(id, nft)| (*id, nft.clone().into()))
            .collect();

        Ok(Self {
            balances,
            nfts,
            minted_nfts,
            nonce: account.nonce,
            pub_key_hash: account.pub_key_hash,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AccountStateInfo {
    pub account_id: Option<AccountId>,
    pub committed: ResponseAccountState,
    pub verified: ResponseAccountState,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoResp {
    pub address: Address,
    pub id: Option<AccountId>,
    pub depositing: DepositingAccountBalances,
    pub committed: ResponseAccountState,
    pub verified: ResponseAccountState,
    pub account_type: Option<EthAccountType>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockInfo {
    pub block_number: i64,
    pub committed: bool,
    pub verified: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInfoResp {
    pub executed: bool,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub block: Option<BlockInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ETHOpInfoResp {
    pub executed: bool,
    pub block: Option<BlockInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractAddressResp {
    pub main_contract: String,
    pub gov_contract: String,
}

/// The metadata of the JSON-RPC call retrieved from the HTTP request of the call
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestMetadata {
    /// The ip of the call origin
    pub ip: String,
}
