//! Account part of API implementation.

// Built-in uses
use std::collections::BTreeMap;
use std::str::FromStr;

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_api_types::v02::{
    account::{Account, AccountAddressOrId, AccountStateType},
    pagination::{AccountTxsRequest, Paginated, PaginationQuery, PendingOpsRequest},
    transaction::Transaction,
};
use zksync_storage::ConnectionPool;
use zksync_types::{tx::TxHash, AccountId, Address, BlockNumber, SerialId};

// Local uses
use super::{
    error::{Error, InvalidDataError},
    paginate_trait::Paginate,
    response::ApiResult,
};
use crate::{
    api_try, core_api_client::CoreApiClient, fee_ticker::PriceError,
    utils::token_db_cache::TokenDBCache,
};

/// Shared data between `api/v1/accounts` endpoints.
#[derive(Clone)]
struct ApiAccountData {
    pool: ConnectionPool,
    tokens: TokenDBCache,
    core_api_client: CoreApiClient,
}

impl ApiAccountData {
    fn new(pool: ConnectionPool, tokens: TokenDBCache, core_api_client: CoreApiClient) -> Self {
        Self {
            pool,
            tokens,
            core_api_client,
        }
    }

    async fn get_id_by_address_or_id(
        &self,
        account_address_or_id: AccountAddressOrId,
    ) -> Result<AccountId, Error> {
        match account_address_or_id {
            AccountAddressOrId::Id(account_id) => Ok(account_id),
            AccountAddressOrId::Address(address) => {
                let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
                let account_id = storage
                    .chain()
                    .account_schema()
                    .account_id_by_address(address)
                    .await
                    .map_err(Error::storage)?;
                account_id.ok_or_else(|| Error::from(InvalidDataError::AccountNotFound))
            }
        }
    }

    async fn get_address_by_address_or_id(
        &self,
        account_address_or_id: AccountAddressOrId,
    ) -> Result<Address, Error> {
        match account_address_or_id {
            AccountAddressOrId::Id(account_id) => {
                let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
                let address = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(account_id)
                    .await
                    .map_err(Error::storage)?;
                address.ok_or_else(|| Error::from(InvalidDataError::AccountNotFound))
            }
            AccountAddressOrId::Address(address) => Ok(address),
        }
    }

    async fn parse_account_id_or_address(
        &self,
        account_address_or_id: &str,
    ) -> Result<AccountAddressOrId, Error> {
        if let Ok(account_id) = u32::from_str(account_address_or_id) {
            Ok(AccountAddressOrId::Id(AccountId(account_id)))
        } else {
            let address_str = if let Some(address_str) = account_address_or_id.strip_prefix("0x") {
                address_str
            } else {
                account_address_or_id
            };

            if let Ok(address) = Address::from_str(address_str) {
                Ok(AccountAddressOrId::Address(address))
            } else {
                Err(Error::from(InvalidDataError::InvalidAccountIdOrAddress))
            }
        }
    }

    async fn account_info(
        &self,
        account_id: AccountId,
        state_type: AccountStateType,
    ) -> Result<Option<Account>, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        let (last_update_in_block, account) = match state_type {
            AccountStateType::Committed => {
                let account = storage
                    .chain()
                    .account_schema()
                    .last_committed_state_for_account(account_id)
                    .await
                    .map_err(Error::storage)?;
                if let Some(account) = account {
                    let last_block = storage
                        .chain()
                        .account_schema()
                        .last_committed_block_with_update_for_acc(account_id)
                        .await
                        .map_err(Error::storage)?;
                    (last_block, account)
                } else {
                    return Ok(None);
                }
            }
            AccountStateType::Finalized => {
                let (last_block, account) = storage
                    .chain()
                    .account_schema()
                    .account_and_last_block(account_id)
                    .await
                    .map_err(Error::storage)?;
                if let Some(account) = account {
                    (BlockNumber(last_block as u32), account)
                } else {
                    return Ok(None);
                }
            }
        };
        let mut balances = BTreeMap::new();
        for (token_id, balance) in account.get_nonzero_balances() {
            let token_symbol = self
                .tokens
                .token_symbol(&mut storage, token_id)
                .await
                .map_err(Error::storage)?
                .ok_or_else(|| Error::from(PriceError::token_not_found(token_id)))?;

            balances.insert(token_symbol, balance);
        }
        Ok(Some(Account {
            account_id,
            address: account.address,
            nonce: account.nonce,
            pub_key_hash: account.pub_key_hash,
            last_update_in_block,
            balances,
        }))
    }

    async fn account_txs(
        &self,
        query: PaginationQuery<TxHash>,
        address: Address,
    ) -> Result<Paginated<Transaction, AccountTxsRequest>, Error> {
        let new_query = PaginationQuery {
            from: AccountTxsRequest {
                address,
                tx_hash: query.from,
            },
            limit: query.limit,
            direction: query.direction,
        };
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        storage.paginate(&new_query).await
    }

    async fn account_pending_txs(
        &self,
        query: PaginationQuery<SerialId>,
        address: Address,
        account_id: AccountId,
    ) -> Result<Paginated<Transaction, PendingOpsRequest>, Error> {
        let new_query = PaginationQuery {
            from: PendingOpsRequest {
                address,
                account_id,
                serial_id: query.from,
            },
            limit: query.limit,
            direction: query.direction,
        };
        let mut client = self.core_api_client.clone();
        client.paginate(&new_query).await
    }
}

async fn account_committed_info(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
) -> ApiResult<Option<Account>> {
    let address_or_id = api_try!(
        data.parse_account_id_or_address(&account_id_or_address)
            .await
    );
    let account_id = api_try!(data.get_id_by_address_or_id(address_or_id).await);
    data.account_info(account_id, AccountStateType::Committed)
        .await
        .into()
}

async fn account_finalized_info(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
) -> ApiResult<Option<Account>> {
    let address_or_id = api_try!(
        data.parse_account_id_or_address(&account_id_or_address)
            .await
    );
    let account_id = api_try!(data.get_id_by_address_or_id(address_or_id).await);
    data.account_info(account_id, AccountStateType::Finalized)
        .await
        .into()
}

async fn account_txs(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
    web::Query(query): web::Query<PaginationQuery<TxHash>>,
) -> ApiResult<Paginated<Transaction, AccountTxsRequest>> {
    let address_or_id = api_try!(
        data.parse_account_id_or_address(&account_id_or_address)
            .await
    );
    let address = api_try!(data.get_address_by_address_or_id(address_or_id).await);
    data.account_txs(query, address).await.into()
}

async fn account_pending_txs(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
    web::Query(query): web::Query<PaginationQuery<SerialId>>,
) -> ApiResult<Paginated<Transaction, PendingOpsRequest>> {
    let address_or_id = api_try!(
        data.parse_account_id_or_address(&account_id_or_address)
            .await
    );
    // Both id and address are needed because pending deposits can be matched only with addresses,
    // while pending full exits can be matched only with account ids.
    let account_id = api_try!(data.get_id_by_address_or_id(address_or_id.clone()).await);
    let address = api_try!(data.get_address_by_address_or_id(address_or_id).await);
    data.account_pending_txs(query, address, account_id)
        .await
        .into()
}

pub fn api_scope(
    pool: ConnectionPool,
    tokens: TokenDBCache,
    core_api_client: CoreApiClient,
) -> Scope {
    let data = ApiAccountData::new(pool, tokens, core_api_client);

    web::scope("account")
        .data(data)
        .route(
            "{account_id_or_address}/committed",
            web::get().to(account_committed_info),
        )
        .route(
            "{account_id_or_address}/finalized",
            web::get().to(account_finalized_info),
        )
        .route(
            "{account_id_or_address}/transactions",
            web::get().to(account_txs),
        )
        .route(
            "{account_id_or_address}/transactions/pending",
            web::get().to(account_pending_txs),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_server::rest::v02::{
        test_utils::{deserialize_response_result, TestServerConfig},
        SharedData,
    };
    use actix_web::{web::Json, App};
    use serde::Deserialize;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use zksync_api_client::rest::client::Client;
    use zksync_api_types::v02::{
        pagination::{PaginationDirection, PaginationQuery, PendingOpsRequest},
        transaction::{L1Transaction, TransactionData},
        ApiVersion,
    };
    use zksync_storage::StorageProcessor;
    use zksync_types::{AccountId, Address};

    type PendingOpsHandle = Arc<Mutex<serde_json::Value>>;

    fn create_pending_ops_handle() -> PendingOpsHandle {
        Arc::new(Mutex::new(json!({
            "list": [],
            "pagination": {
                "from": {
                    "address": Address::default(),
                    "account_id": AccountId::default(),
                    "serial_id": 1
                },
                "limit": 1,
                "direction": "newer",
                "count": 0,
            }
        })))
    }

    #[derive(Debug, Deserialize)]
    struct PendingOpsParams {
        pub address: Address,
        pub account_id: AccountId,
        pub serial_id: u64,
        pub limit: u32,
        pub direction: PaginationDirection,
    }

    fn get_unconfirmed_ops_loopback(
        ops_handle: PendingOpsHandle,
    ) -> (CoreApiClient, actix_web::test::TestServer) {
        async fn get_ops(
            data: web::Data<PendingOpsHandle>,
            web::Query(_query): web::Query<PendingOpsParams>,
        ) -> Json<serde_json::Value> {
            Json(data.lock().await.clone())
        }

        let server = actix_web::test::start(move || {
            App::new().service(
                web::scope("unconfirmed_ops")
                    .data(ops_handle.clone())
                    .route("", web::get().to(get_ops)),
            )
        });

        let url = server.url("").trim_end_matches('/').to_owned();
        (CoreApiClient::new(url), server)
    }

    struct TestServer {
        core_server: actix_web::test::TestServer,
        api_server: actix_web::test::TestServer,
        pool: ConnectionPool,
        pending_ops: PendingOpsHandle,
    }

    impl TestServer {
        async fn new() -> anyhow::Result<(Client, Self)> {
            let cfg = TestServerConfig::default();
            cfg.fill_database().await?;

            let pending_ops = create_pending_ops_handle();
            let (core_client, core_server) = get_unconfirmed_ops_loopback(pending_ops.clone());

            let pool = cfg.pool.clone();

            let shared_data = SharedData {
                net: cfg.config.chain.eth.network,
                api_version: ApiVersion::V02,
            };
            let (api_client, api_server) = cfg.start_server(
                move |cfg: &TestServerConfig| {
                    api_scope(cfg.pool.clone(), TokenDBCache::new(), core_client.clone())
                },
                shared_data,
            );

            Ok((
                api_client,
                Self {
                    core_server,
                    api_server,
                    pool,
                    pending_ops,
                },
            ))
        }

        async fn account_id_and_tx_hash(
            storage: &mut StorageProcessor<'_>,
            block: BlockNumber,
        ) -> anyhow::Result<(AccountId, TxHash)> {
            let transactions = storage
                .chain()
                .block_schema()
                .get_block_transactions(block)
                .await?;

            let tx = &transactions[0];
            let op = tx.op.as_object().unwrap();

            let id = serde_json::from_value(op["accountId"].clone()).unwrap();
            Ok((id, TxHash::from_str(&tx.tx_hash).unwrap()))
        }

        async fn stop(self) {
            self.api_server.stop().await;
            self.core_server.stop().await;
        }
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn unconfirmed_deposits_loopback() -> anyhow::Result<()> {
        let (client, server) = get_unconfirmed_ops_loopback(create_pending_ops_handle());

        client
            .get_unconfirmed_ops(&PaginationQuery {
                from: PendingOpsRequest {
                    address: Address::default(),
                    account_id: AccountId::default(),
                    serial_id: 0,
                },
                limit: 0,
                direction: PaginationDirection::Newer,
            })
            .await?;

        server.stop().await;
        Ok(())
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn accounts_scope() -> anyhow::Result<()> {
        let (client, server) = TestServer::new().await?;

        // Get account information.
        let (account_id, tx_hash) = TestServer::account_id_and_tx_hash(
            &mut server.pool.access_storage().await?,
            BlockNumber(1),
        )
        .await?;

        let response = client
            .account_info_v02(&account_id.to_string(), "committed")
            .await?;
        let account_info_by_id: Account = deserialize_response_result(response)?;

        let address = account_info_by_id.address;
        let response = client
            .account_info_v02(&format!("{:?}", address), "committed")
            .await?;
        let account_info_by_address: Account = deserialize_response_result(response)?;
        assert_eq!(account_info_by_id, account_info_by_address);

        let query = PaginationQuery {
            from: tx_hash,
            limit: 1,
            direction: PaginationDirection::Newer,
        };
        let response = client.account_txs(&query, &account_id.to_string()).await?;
        let txs: Paginated<Transaction, AccountTxsRequest> = deserialize_response_result(response)?;
        assert_eq!(txs.list[0].tx_hash, tx_hash);

        // Provide unconfirmed pending ops.
        *server.pending_ops.lock().await = json!({
            "list": [
                {
                    "serial_id": 10,
                    "data": {
                        "type": "Deposit",
                        "account_id": account_id,
                        "amount": "100500",
                        "from": Address::default(),
                        "to": address,
                        "token": 0,
                    },
                    "deadline_block": 10,
                    "eth_hash": vec![0u8; 32],
                    "eth_block": 5,
                    "eth_block_index": 2
                },
            ],
            "pagination": {
                "from": {
                    "serial_id": 1,
                    "address": address,
                    "account_id": account_id
                },
                "limit": 1,
                "count": 1,
                "direction": "newer"
            }
        });

        let query = PaginationQuery {
            from: 1,
            limit: 1,
            direction: PaginationDirection::Newer,
        };
        let response = client
            .account_pending_txs(&query, &account_id.to_string())
            .await?;
        let txs: Paginated<Transaction, PendingOpsRequest> = deserialize_response_result(response)?;
        match &txs.list[0].op {
            TransactionData::L1(tx) => match tx {
                L1Transaction::Deposit(deposit) => {
                    assert_eq!(deposit.id, 10);
                }
                _ => panic!("should return deposit"),
            },
            _ => panic!("account_pending_txs returned L2 tx"),
        }

        server.stop().await;
        Ok(())
    }
}
