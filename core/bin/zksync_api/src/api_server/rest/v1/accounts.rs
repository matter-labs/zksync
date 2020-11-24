//! Accounts part of API implementation.

// Built-in uses

// External uses
use std::collections::BTreeMap;

use actix_web::{
    web::{self, Json},
    Scope,
};
use num::BigUint;
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_storage::{chain::account::AccountQuery as StorageAccountQuery, QueryResult};
use zksync_types::{
    Account, AccountId, Address, BlockNumber, Nonce, PriorityOp, PubKeyHash, TokenId,
};
use zksync_utils::BigUintSerdeWrapper;

// Local uses
use crate::{
    api_server::{rpc_server::types::OngoingDeposit, tx_sender::TxSender},
    core_api_client::EthBlockId,
    utils::token_db_cache::TokenDBCache,
};

use super::{client::Client, Error as ApiError, JsonResult};

fn unable_to_find_token(token_id: TokenId) -> anyhow::Error {
    anyhow::anyhow!("Unable to find token with ID {}", token_id)
}

/// Shared data between `api/v1/accounts` endpoints.
#[derive(Clone)]
struct ApiAccountsData {
    tx_sender: TxSender,
    confirmations_for_eth_event: BlockNumber,
}

impl ApiAccountsData {
    fn new(tx_sender: TxSender, confirmations_for_eth_event: BlockNumber) -> Self {
        Self {
            tx_sender,
            confirmations_for_eth_event,
        }
    }

    async fn account_info(&self, query: AccountQuery) -> QueryResult<Option<AccountInfo>> {
        let mut storage = self.tx_sender.pool.access_storage().await?;

        let account_state = storage
            .chain()
            .account_schema()
            .account_state(query)
            .await?;

        // TODO This code uses same logic as the old RPC, but I'm not sure that if it is correct.
        let (account_id, account) = if let Some(state) = account_state.committed {
            state
        } else {
            // This account has not been committed.
            return Ok(None);
        };

        let committed = AccountState::from_storage(&account, &self.tx_sender.tokens).await?;
        let verified = match account_state.verified {
            Some(state) => AccountState::from_storage(&state.1, &self.tx_sender.tokens).await?,
            None => AccountState::default(),
        };

        let depositing = {
            let ongoing_ops = self
                .tx_sender
                .core_api_client
                .get_unconfirmed_deposits(account.address)
                .await?;

            DepositingBalances::from_pending_ops(
                ongoing_ops,
                self.confirmations_for_eth_event,
                &self.tx_sender.tokens,
            )
            .await?
        };

        let info = AccountInfo {
            address: account.address,
            id: account_id,
            committed,
            verified,
            depositing,
        };

        Ok(Some(info))
    }
}

// Data transfer objects.

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[serde(rename_all = "camelCase")]
pub enum AccountQuery {
    Id(AccountId),
    Address(Address),
}

/// Account state at the time of the zkSync block commit or verification.
/// This means that each account has various states.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    /// Account wallet balances.
    pub balances: BTreeMap<String, BigUintSerdeWrapper>,
    /// zkSync account nonce.
    pub nonce: Nonce,
    /// Hash of the account's owner public key.
    pub pub_key_hash: PubKeyHash,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingFunds {
    pub amount: BigUintSerdeWrapper,
    pub expected_accept_block: BlockNumber,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingBalances {
    pub balances: BTreeMap<String, DepositingFunds>,
}

/// Account summary info in the zkSync network.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    /// Account address.
    pub address: Address,
    /// Unique identifier of the account in the zkSync network.
    pub id: AccountId,
    // pub depositing: DepositingAccountBalances,
    /// Account state in according of the actual committed block.
    pub committed: AccountState,
    /// Account state in according of the actual verified block.
    pub verified: AccountState,

    pub depositing: DepositingBalances,
}

impl From<AccountQuery> for StorageAccountQuery {
    fn from(query: AccountQuery) -> Self {
        match query {
            AccountQuery::Id(id) => StorageAccountQuery::Id(id),
            AccountQuery::Address(address) => StorageAccountQuery::Address(address),
        }
    }
}

impl AccountState {
    pub(crate) async fn from_storage(
        account: &Account,
        tokens: &TokenDBCache,
    ) -> QueryResult<Self> {
        let mut balances = BTreeMap::new();
        for (token_id, balance) in account.get_nonzero_balances() {
            let token_symbol = tokens
                .token_symbol(token_id)
                .await?
                .ok_or_else(|| unable_to_find_token(token_id))?;

            balances.insert(token_symbol, balance);
        }

        Ok(Self {
            balances,
            nonce: account.nonce,
            pub_key_hash: account.pub_key_hash,
        })
    }
}

impl DepositingBalances {
    pub(crate) async fn from_pending_ops(
        ongoing_ops: Vec<(EthBlockId, PriorityOp)>,
        confirmations_for_eth_event: BlockNumber,
        tokens: &TokenDBCache,
    ) -> QueryResult<Self> {
        let mut balances = BTreeMap::new();

        for (block, op) in ongoing_ops {
            // TODO do not use types from RPC API.
            let op = OngoingDeposit::new(block, op);

            let token_symbol = tokens
                .token_symbol(op.token_id)
                .await?
                .ok_or_else(|| unable_to_find_token(op.token_id))?;

            let expected_accept_block =
                op.received_on_block as BlockNumber + confirmations_for_eth_event;

            let balance = balances
                .entry(token_symbol)
                .or_insert_with(DepositingFunds::default);

            balance.amount.0 += BigUint::from(op.amount);

            // `balance.expected_accept_block` should be the greatest block number among
            // all the deposits for a certain token.
            if expected_accept_block > balance.expected_accept_block {
                balance.expected_accept_block = expected_accept_block;
            }
        }

        Ok(Self { balances })
    }
}

// Client implementation

/// Accounts API part.
impl Client {}

// Server implementation

async fn account_info(
    data: web::Data<ApiAccountsData>,
    web::Path(query): web::Path<AccountQuery>,
) -> JsonResult<Option<AccountInfo>> {
    data.account_info(query)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

pub fn api_scope(env_options: &ConfigurationOptions, tx_sender: TxSender) -> Scope {
    let data = ApiAccountsData::new(
        tx_sender,
        env_options.confirmations_for_eth_event as BlockNumber,
    );

    web::scope("accounts")
        .data(data)
        .route("{}", web::get().to(account_info))
}

#[cfg(test)]
mod tests {
    use super::{super::test_utils::TestServerConfig, *};

    #[actix_rt::test]
    async fn test_accounts_scope() -> anyhow::Result<()> {
        Ok(())
    }
}
