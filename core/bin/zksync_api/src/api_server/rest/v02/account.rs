//! Account part of API implementation.

// Built-in uses
use std::collections::BTreeMap;
use std::str::FromStr;

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_api_types::v02::{
    account::{Account, AccountAddressOrId, AccountState},
    pagination::{
        parse_query, AccountTxsRequest, ApiEither, Paginated, PaginationQuery, PendingOpsRequest,
    },
    transaction::{Transaction, TxHashSerializeWrapper},
};
use zksync_crypto::params::{MIN_NFT_TOKEN_ID, NFT_TOKEN_ID_VAL};
use zksync_storage::{ConnectionPool, StorageProcessor};
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

/// Shared data between `api/v02/accounts` endpoints.
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
    ) -> Result<Option<AccountId>, Error> {
        match account_address_or_id {
            AccountAddressOrId::Id(account_id) => Ok(Some(account_id)),
            AccountAddressOrId::Address(address) => {
                let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
                let account_id = storage
                    .chain()
                    .account_schema()
                    .account_id_by_address(address)
                    .await
                    .map_err(Error::storage)?;
                Ok(account_id)
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

    fn parse_account_id_or_address(
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

    async fn api_account(
        &self,
        account: zksync_types::Account,
        account_id: AccountId,
        last_update_in_block: BlockNumber,
        storage: &mut StorageProcessor<'_>,
    ) -> Result<Account, Error> {
        let mut balances = BTreeMap::new();
        let mut nfts = BTreeMap::new();
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
                        self.tokens
                            .get_nft_by_id(storage, token_id)
                            .await
                            .map_err(Error::storage)?
                            .ok_or_else(|| Error::from(PriceError::token_not_found(token_id)))?
                            .into(),
                    );
                }
                _ => {
                    let token_symbol = self
                        .tokens
                        .token_symbol(storage, token_id)
                        .await
                        .map_err(Error::storage)?
                        .ok_or_else(|| Error::from(PriceError::token_not_found(token_id)))?;
                    balances.insert(token_symbol, balance);
                }
            }
        }
        let minted_nfts = account
            .minted_nfts
            .iter()
            .map(|(id, nft)| (*id, nft.clone().into()))
            .collect();

        let account_type = storage
            .chain()
            .account_schema()
            .account_type_by_id(account_id)
            .await
            .map_err(Error::storage)?
            .map(|t| t.into());
        Ok(Account {
            account_id,
            address: account.address,
            nonce: account.nonce,
            pub_key_hash: account.pub_key_hash,
            last_update_in_block,
            balances,
            account_type,
            nfts,
            minted_nfts,
        })
    }

    async fn account_committed_info(
        &self,
        account_id: AccountId,
    ) -> Result<Option<Account>, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        let mut transaction = storage.start_transaction().await.map_err(Error::storage)?;
        let account = transaction
            .chain()
            .account_schema()
            .last_committed_state_for_account(account_id)
            .await
            .map_err(Error::storage)?
            .1;
        let result = if let Some(account) = account {
            let last_block = transaction
                .chain()
                .account_schema()
                .last_committed_block_with_update_for_acc(account_id)
                .await
                .map_err(Error::storage)?;
            Ok(Some(
                self.api_account(account, account_id, last_block, &mut transaction)
                    .await?,
            ))
        } else {
            Ok(None)
        };
        transaction.commit().await.map_err(Error::storage)?;
        result
    }

    async fn account_finalized_info(
        &self,
        account_id: AccountId,
    ) -> Result<Option<Account>, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        let mut transaction = storage.start_transaction().await.map_err(Error::storage)?;
        let (last_block, account) = transaction
            .chain()
            .account_schema()
            .account_and_last_block(account_id)
            .await
            .map_err(Error::storage)?;
        let result = if let Some(account) = account {
            Ok(Some(
                self.api_account(
                    account,
                    account_id,
                    BlockNumber(last_block as u32),
                    &mut transaction,
                )
                .await?,
            ))
        } else {
            Ok(None)
        };
        transaction.commit().await.map_err(Error::storage)?;
        result
    }

    async fn account_full_info(&self, account_id: AccountId) -> Result<AccountState, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        let mut transaction = storage.start_transaction().await.map_err(Error::storage)?;
        let (finalized_state, committed_state) = transaction
            .chain()
            .account_schema()
            .last_committed_state_for_account(account_id)
            .await
            .map_err(Error::storage)?;
        let finalized = if let Some(account) = finalized_state.1 {
            Some(
                self.api_account(
                    account,
                    account_id,
                    BlockNumber(finalized_state.0 as u32),
                    &mut transaction,
                )
                .await?,
            )
        } else {
            None
        };
        let committed = if let Some(account) = committed_state {
            let last_block = transaction
                .chain()
                .account_schema()
                .last_committed_block_with_update_for_acc(account_id)
                .await
                .map_err(Error::storage)?;
            Some(
                self.api_account(account, account_id, last_block, &mut transaction)
                    .await?,
            )
        } else {
            None
        };
        transaction.commit().await.map_err(Error::storage)?;
        Ok(AccountState {
            committed,
            finalized,
        })
    }

    async fn account_txs(
        &self,
        query: PaginationQuery<ApiEither<TxHash>>,
        address: Address,
    ) -> Result<Paginated<Transaction, TxHashSerializeWrapper>, Error> {
        let mut storage = self.pool.access_storage().await.map_err(Error::storage)?;
        let new_query = PaginationQuery {
            from: AccountTxsRequest {
                address,
                tx_hash: query.from,
            },
            limit: query.limit,
            direction: query.direction,
        };
        storage.paginate_checked(&new_query).await
    }

    /// Pending deposits can be matched only with addresses,
    /// while pending full exits can be matched only with account ids.
    /// If the account isn't created yet it doesn't have an id
    /// but we can still find pending deposits for its address that is why account_id is Option.
    async fn account_pending_txs(
        &self,
        query: PaginationQuery<ApiEither<SerialId>>,
        address: Address,
        account_id: Option<AccountId>,
    ) -> Result<Paginated<Transaction, SerialId>, Error> {
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
        client.paginate_checked(&new_query).await
    }
}

async fn account_committed_info(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
) -> ApiResult<Option<Account>> {
    let address_or_id = api_try!(data.parse_account_id_or_address(&account_id_or_address));
    let account_id = api_try!(data.get_id_by_address_or_id(address_or_id).await);
    if let Some(account_id) = account_id {
        data.account_committed_info(account_id).await.into()
    } else {
        ApiResult::Ok(None)
    }
}

async fn account_finalized_info(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
) -> ApiResult<Option<Account>> {
    let address_or_id = api_try!(data.parse_account_id_or_address(&account_id_or_address));
    let account_id = api_try!(data.get_id_by_address_or_id(address_or_id).await);
    if let Some(account_id) = account_id {
        data.account_finalized_info(account_id).await.into()
    } else {
        ApiResult::Ok(None)
    }
}

async fn account_full_info(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
) -> ApiResult<AccountState> {
    let address_or_id = api_try!(data.parse_account_id_or_address(&account_id_or_address));
    let account_id = api_try!(data.get_id_by_address_or_id(address_or_id).await);
    if let Some(account_id) = account_id {
        data.account_full_info(account_id).await.into()
    } else {
        ApiResult::Ok(AccountState {
            committed: None,
            finalized: None,
        })
    }
}

async fn account_txs(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
    web::Query(query): web::Query<PaginationQuery<String>>,
) -> ApiResult<Paginated<Transaction, TxHashSerializeWrapper>> {
    let query = api_try!(parse_query(query).map_err(Error::from));
    let address_or_id = api_try!(data.parse_account_id_or_address(&account_id_or_address));
    let address = api_try!(data.get_address_by_address_or_id(address_or_id).await);
    data.account_txs(query, address).await.into()
}

async fn account_pending_txs(
    data: web::Data<ApiAccountData>,
    web::Path(account_id_or_address): web::Path<String>,
    web::Query(query): web::Query<PaginationQuery<String>>,
) -> ApiResult<Paginated<Transaction, SerialId>> {
    let query = api_try!(parse_query(query).map_err(Error::from));
    let address_or_id = api_try!(data.parse_account_id_or_address(&account_id_or_address));
    let address = api_try!(
        data.get_address_by_address_or_id(address_or_id.clone())
            .await
    );
    let account_id = api_try!(data.get_id_by_address_or_id(address_or_id).await);
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

    web::scope("accounts")
        .data(data)
        .route(
            "{account_id_or_address}/committed",
            web::get().to(account_committed_info),
        )
        .route(
            "{account_id_or_address}/finalized",
            web::get().to(account_finalized_info),
        )
        .route("{account_id_or_address}", web::get().to(account_full_info))
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
    use chrono::Utc;
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
    use zksync_types::{AccountId, Address, H256};

    type PendingOpsHandle = Arc<Mutex<serde_json::Value>>;

    fn create_pending_ops_handle() -> PendingOpsHandle {
        Arc::new(Mutex::new(json!({
            "list": [],
            "pagination": {
                "from": 1,
                "limit": 1,
                "direction": "newer",
                "count": 0
            }
        })))
    }

    #[derive(Debug, Deserialize)]
    struct PendingOpsFlattenRequest {
        pub address: Address,
        pub account_id: Option<AccountId>,
        pub serial_id: String,
        pub limit: u32,
        pub direction: PaginationDirection,
    }

    fn get_unconfirmed_ops_loopback(
        ops_handle: PendingOpsHandle,
    ) -> (CoreApiClient, actix_web::test::TestServer) {
        async fn get_ops(
            data: web::Data<PendingOpsHandle>,
            web::Query(_query): web::Query<PendingOpsFlattenRequest>,
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
                Some(shared_data),
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

            let tx = &transactions[1];
            let op = tx.op.as_object().unwrap();

            let id = if op.contains_key("accountId") {
                serde_json::from_value(op["accountId"].clone()).unwrap()
            } else {
                serde_json::from_value(op["creatorId"].clone()).unwrap()
            };
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
                    account_id: Some(AccountId::default()),
                    serial_id: ApiEither::from(0),
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
            .account_info(&account_id.to_string(), "committed")
            .await?;
        let account_committed_info_by_id: Account = deserialize_response_result(response)?;

        let address = account_committed_info_by_id.address;
        let response = client
            .account_info(&format!("{:?}", address), "committed")
            .await?;
        let account_committed_info_by_address: Account = deserialize_response_result(response)?;
        assert_eq!(
            account_committed_info_by_id,
            account_committed_info_by_address
        );

        let response = client
            .account_info(&format!("{:?}", address), "finalized")
            .await?;
        let account_finalized_info: Option<Account> = deserialize_response_result(response)?;

        let response = client.account_full_info(&format!("{:?}", address)).await?;
        let account_full_info: AccountState = deserialize_response_result(response)?;
        assert_eq!(
            account_full_info.committed,
            Some(account_committed_info_by_id)
        );
        assert_eq!(account_full_info.finalized, account_finalized_info);

        let query = PaginationQuery {
            from: ApiEither::from(tx_hash),
            limit: 1,
            direction: PaginationDirection::Newer,
        };
        let response = client.account_txs(&query, &account_id.to_string()).await?;
        let txs: Paginated<Transaction, TxHash> = deserialize_response_result(response)?;
        assert_eq!(txs.list[0].tx_hash, tx_hash);
        // Provide unconfirmed pending ops.
        *server.pending_ops.lock().await = json!({
            "list": [
                {
                    "txHash": TxHash::from_slice(&[0u8; 32]),
                    "blockNumber": Option::<BlockNumber>::None,
                    "op": {
                        "type": "Deposit",
                        "from": Address::default(),
                        "tokenId": 0,
                        "amount": "100500",
                        "to": address,
                        "accountId": Option::<AccountId>::None,
                        "ethHash": H256::from_slice(&[0u8; 32]),
                        "id": 10,
                        "txHash": TxHash::from_slice(&[0u8; 32])
                    },
                    "status": "queued",
                    "failReason": Option::<String>::None,
                    "createdAt": Utc::now()
                }
            ],
            "pagination": {
                "from": 1,
                "limit": 1,
                "count": 1,
                "direction": "newer"
            }
        });

        let query = PaginationQuery {
            from: ApiEither::from(1),
            limit: 1,
            direction: PaginationDirection::Newer,
        };
        let response = client
            .account_pending_txs(&query, &account_id.to_string())
            .await?;
        let txs: Paginated<Transaction, SerialId> = deserialize_response_result(response)?;
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
