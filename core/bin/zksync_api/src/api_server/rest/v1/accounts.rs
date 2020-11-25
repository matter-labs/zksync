//! Accounts part of API implementation.

// Built-in uses

// External uses
use std::{collections::BTreeMap, fmt::Display, str::FromStr};

use actix_web::{
    web::{self, Json},
    Scope,
};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_storage::{
    chain::{
        account::AccountQuery as StorageAccountQuery,
        operations_ext::{
            records::TransactionsHistoryItem, SearchDirection as StorageSearchDirection,
        },
    },
    QueryResult,
};
use zksync_types::{
    Account, AccountId, Address, BlockNumber, Nonce, PriorityOp, PubKeyHash, TokenId,
};
use zksync_utils::BigUintSerdeWrapper;

// Local uses
use crate::{
    api_server::rest::helpers::remove_prefix,
    core_api_client::{CoreApiClient, EthBlockId},
    utils::token_db_cache::TokenDBCache,
};

use super::{client::Client, client::ClientError, Error as ApiError, JsonResult};

fn unable_to_find_token(token_id: TokenId) -> anyhow::Error {
    anyhow::anyhow!("Unable to find token with ID {}", token_id)
}

// Additional parser because actix-web doesn't understand enums in path extractor.
fn parse_account_query(query: String) -> Result<AccountQuery, ApiError> {
    query.parse().map_err(|err| {
        ApiError::internal("Must be specified either an account ID or an account address.")
            .detail(format!("An error occurred: {}", err))
    })
}

/// Shared data between `api/v1/accounts` endpoints.
#[derive(Clone)]
struct ApiAccountsData {
    tokens: TokenDBCache,
    core_api_client: CoreApiClient,
    confirmations_for_eth_event: BlockNumber,
}

impl ApiAccountsData {
    fn new(
        tokens: TokenDBCache,
        core_api_client: CoreApiClient,
        confirmations_for_eth_event: BlockNumber,
    ) -> Self {
        Self {
            tokens,
            core_api_client,
            confirmations_for_eth_event,
        }
    }

    async fn account_address(&self, query: AccountQuery) -> QueryResult<Option<Address>> {
        let address = match query {
            AccountQuery::Id(id) => {
                let mut storage = self.tokens.pool.access_storage().await?;

                let account_state = storage.chain().account_schema().account_state(id).await?;

                account_state
                    .committed
                    .map(|(_id, account)| account.address)
            }
            AccountQuery::Address(address) => Some(address),
        };

        Ok(address)
    }

    async fn account_info(&self, query: AccountQuery) -> QueryResult<Option<AccountInfo>> {
        let mut storage = self.tokens.pool.access_storage().await?;

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

        let committed = AccountState::from_storage(&account, &self.tokens).await?;
        let verified = match account_state.verified {
            Some(state) => AccountState::from_storage(&state.1, &self.tokens).await?,
            None => AccountState::default(),
        };

        let depositing = {
            let ongoing_ops = self
                .core_api_client
                .get_unconfirmed_deposits(account.address)
                .await?;

            DepositingBalances::from_pending_ops(
                ongoing_ops,
                self.confirmations_for_eth_event,
                &self.tokens,
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

    async fn tx_receipts(
        &self,
        address: Address,
        query: TxLocationQuery,
        limit: u64,
    ) -> QueryResult<Vec<TransactionsHistoryItem>> {
        let mut storage = self.tokens.pool.access_storage().await?;

        let location = (query.block as u64, query.index);
        let direction = query.direction.into();

        storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history_from(&address, location, direction, limit)
            .await
    }

    async fn pending_tx_receipts(&self, address: Address) -> QueryResult<()> {
        let mut storage = self.tokens.pool.access_storage().await?;

        // TODO implement special tx receipts selector.
        let txs = storage.chain().mempool_schema().load_txs().await?;
        todo!()
    }
}

// Data transfer objects.

#[derive(Debug, Serialize, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[serde(untagged, rename_all = "camelCase")]
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
    /// The greatest block number among all the deposits for a certain token.
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
    /// Account state in according of the actual committed block.
    pub committed: AccountState,
    /// Account state in according of the actual verified block.
    pub verified: AccountState,
    /// Unconfirmed account deposits.
    pub depositing: DepositingBalances,
}

/// The unique transaction location, which is describes by a pair:
/// (block number, transaction index in it).
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TxLocationQuery {
    /// The block containing the transaction.
    pub block: BlockNumber,
    /// Transaction index in block.
    pub index: u64,
    /// Search direction.
    pub direction: SearchDirection,
}

/// Direction to perform search of transactions to.
#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SearchDirection {
    /// Find transactions older than specified one.
    Older,
    /// Find transactions newer than specified one.
    Newer,
}

impl From<AccountQuery> for StorageAccountQuery {
    fn from(query: AccountQuery) -> Self {
        match query {
            AccountQuery::Id(id) => StorageAccountQuery::Id(id),
            AccountQuery::Address(address) => StorageAccountQuery::Address(address),
        }
    }
}

impl From<AccountId> for AccountQuery {
    fn from(v: AccountId) -> Self {
        Self::Id(v)
    }
}

impl From<Address> for AccountQuery {
    fn from(v: Address) -> Self {
        Self::Address(v)
    }
}

impl Display for AccountQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountQuery::Id(id) => id.fmt(f),
            AccountQuery::Address(address) => address.fmt(f),
        }
    }
}

impl FromStr for AccountQuery {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = s.parse::<AccountId>() {
            return Ok(Self::Id(id));
        }

        let s = remove_prefix(s);
        s.parse::<Address>()
            .map(Self::Address)
            .map_err(|e| e.to_string())
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

impl From<SearchDirection> for StorageSearchDirection {
    fn from(inner: SearchDirection) -> Self {
        match inner {
            SearchDirection::Older => StorageSearchDirection::Older,
            SearchDirection::Newer => StorageSearchDirection::Newer,
        }
    }
}

impl DepositingBalances {
    pub(crate) async fn from_pending_ops(
        ongoing_ops: Vec<(EthBlockId, PriorityOp)>,
        confirmations_for_eth_event: BlockNumber,
        tokens: &TokenDBCache,
    ) -> QueryResult<Self> {
        let mut balances = BTreeMap::new();

        for (received_on_block, op) in ongoing_ops {
            let (amount, token_id) = match op.data {
                zksync_types::ZkSyncPriorityOp::Deposit(deposit) => (deposit.amount, deposit.token),
                zksync_types::ZkSyncPriorityOp::FullExit(other) => {
                    panic!("Incorrect input for DepositingBalances: {:?}", other);
                }
            };

            let token_symbol = tokens
                .token_symbol(token_id)
                .await?
                .ok_or_else(|| unable_to_find_token(token_id))?;

            let expected_accept_block =
                received_on_block as BlockNumber + confirmations_for_eth_event;

            let balance = balances
                .entry(token_symbol)
                .or_insert_with(DepositingFunds::default);

            balance.amount.0 += amount;

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
impl Client {
    /// Gets account information
    pub async fn account_info(
        &self,
        query: impl Into<AccountQuery>,
    ) -> Result<Option<AccountInfo>, ClientError> {
        let query = query.into();

        self.get(&format!("accounts/{}", query)).send().await
    }
}

// Server implementation

async fn account_info(
    data: web::Data<ApiAccountsData>,
    web::Path(query): web::Path<String>,
) -> JsonResult<Option<AccountInfo>> {
    let query = parse_account_query(query)?;

    data.account_info(query)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

pub fn api_scope(
    env_options: &ConfigurationOptions,
    tokens: TokenDBCache,
    core_api_client: CoreApiClient,
) -> Scope {
    let data = ApiAccountsData::new(
        tokens,
        core_api_client,
        env_options.confirmations_for_eth_event as BlockNumber,
    );

    web::scope("accounts")
        .data(data)
        .route("{id}", web::get().to(account_info))
}

#[cfg(test)]
mod tests {
    use actix_web::App;

    use super::{super::test_utils::TestServerConfig, *};

    fn get_unconfirmed_deposits_loopback() -> (CoreApiClient, actix_web::test::TestServer) {
        async fn get_unconfirmed_deposits(
            _path: web::Path<String>,
        ) -> Json<Vec<serde_json::Value>> {
            Json(vec![])
        }

        let server = actix_web::test::start(move || {
            App::new().route(
                "unconfirmed_deposits/{address}",
                web::get().to(get_unconfirmed_deposits),
            )
        });

        let mut url = server.url("");
        url.pop(); // Pop last '/' symbol.

        (CoreApiClient::new(url), server)
    }

    struct TestServer {
        core_server: actix_web::test::TestServer,
        api_server: actix_web::test::TestServer,
    }

    impl TestServer {
        async fn new() -> anyhow::Result<(Client, Self)> {
            let (core_client, core_server) = get_unconfirmed_deposits_loopback();

            let cfg = TestServerConfig::default();
            cfg.fill_database().await?;

            let (api_client, api_server) = cfg.start_server(move |cfg| {
                api_scope(
                    &cfg.env_options,
                    TokenDBCache::new(cfg.pool.clone()),
                    core_client.clone(),
                )
            });

            Ok((
                api_client,
                Self {
                    core_server,
                    api_server,
                },
            ))
        }

        async fn stop(self) {
            self.api_server.stop().await;
            self.core_server.stop().await;
        }
    }

    #[actix_rt::test]
    async fn test_get_unconfirmed_deposits_loopback() -> anyhow::Result<()> {
        let (client, server) = get_unconfirmed_deposits_loopback();

        client.get_unconfirmed_deposits(Address::default()).await?;

        server.stop().await;
        Ok(())
    }

    #[actix_rt::test]
    async fn test_accounts_scope() -> anyhow::Result<()> {
        let (client, server) = TestServer::new().await?;

        // Get account information.
        let account_info = client.account_info(0).await?.unwrap();
        let address = account_info.address;
        assert_eq!(client.account_info(address).await?, Some(account_info));

        server.stop().await;
        Ok(())
    }
}
