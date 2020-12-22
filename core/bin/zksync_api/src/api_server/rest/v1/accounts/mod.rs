//! Accounts part of API implementation.

// Public uses
pub use self::types::{
    AccountInfo, AccountOpReceipt, AccountQuery, AccountReceipts, AccountState, AccountTxReceipt,
    DepositingBalances, DepositingFunds, PendingAccountOpReceipt, TxLocation,
};

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_storage::{QueryResult, StorageProcessor};
use zksync_types::{AccountId, Address, BlockNumber, TokenId};

// Local uses
use crate::{core_api_client::CoreApiClient, utils::token_db_cache::TokenDBCache};

use self::types::{AccountReceiptsQuery, SearchDirection};
use super::{ApiError, JsonResult};

mod client;
#[cfg(test)]
mod tests;
mod types;

fn unable_to_find_token(token_id: TokenId) -> anyhow::Error {
    anyhow::anyhow!("Unable to find token with ID {}", token_id)
}

// Additional parser because actix-web doesn't understand enums in path extractor.
fn parse_account_query(query: String) -> Result<AccountQuery, ApiError> {
    query.parse().map_err(|err| {
        ApiError::bad_request("Must be specified either an account ID or an account address.")
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

    async fn access_storage(&self) -> QueryResult<StorageProcessor<'_>> {
        self.tokens.pool.access_storage().await.map_err(From::from)
    }

    async fn find_account_address(&self, query: String) -> Result<Address, ApiError> {
        let query = parse_account_query(query)?;
        self.account_address(query)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| {
                ApiError::bad_request("Unable to find account.")
                    .detail(format!("Given account {:?} is absent", query))
            })
    }

    async fn account_id(
        storage: &mut StorageProcessor<'_>,
        query: AccountQuery,
    ) -> QueryResult<Option<AccountId>> {
        match query {
            AccountQuery::Id(id) => Ok(Some(id)),
            AccountQuery::Address(address) => {
                storage
                    .chain()
                    .account_schema()
                    .account_id_by_address(address)
                    .await
            }
        }
    }

    async fn account_address(&self, query: AccountQuery) -> QueryResult<Option<Address>> {
        match query {
            AccountQuery::Id(id) => {
                let mut storage = self.access_storage().await?;
                storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(id)
                    .await
            }
            AccountQuery::Address(address) => Ok(Some(address)),
        }
    }

    async fn account_info(&self, query: AccountQuery) -> QueryResult<Option<AccountInfo>> {
        let mut storage = self.access_storage().await?;
        let account_id = if let Some(id) = Self::account_id(&mut storage, query).await? {
            id
        } else {
            return Ok(None);
        };

        let account_state = storage
            .chain()
            .account_schema()
            .account_state_by_id(account_id)
            .await?;

        // Drop storage access to avoid deadlocks.
        // TODO Rewrite `TokensDBCache` logic to make such errors impossible. ZKS-169
        drop(storage);

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
        location: TxLocation,
        direction: SearchDirection,
        limit: u32,
    ) -> QueryResult<Vec<AccountTxReceipt>> {
        let mut storage = self.access_storage().await?;

        let items = storage
            .chain()
            .operations_ext_schema()
            .get_account_transactions_receipts(
                address,
                location.block as u64,
                location.index,
                direction.into(),
                limit as u64,
            )
            .await?;

        Ok(items.into_iter().map(AccountTxReceipt::from).collect())
    }

    async fn op_receipts(
        &self,
        address: Address,
        location: TxLocation,
        direction: SearchDirection,
        limit: u32,
    ) -> QueryResult<Vec<AccountOpReceipt>> {
        let mut storage = self.access_storage().await?;

        let items = storage
            .chain()
            .operations_ext_schema()
            .get_account_operations_receipts(
                address,
                location.block as u64,
                location.index.unwrap_or_default(),
                direction.into(),
                limit as u64,
            )
            .await?;

        Ok(items.into_iter().map(AccountOpReceipt::from).collect())
    }

    async fn pending_op_receipts(
        &self,
        address: Address,
    ) -> QueryResult<Vec<PendingAccountOpReceipt>> {
        let ongoing_ops = self.core_api_client.get_unconfirmed_ops(address).await?;

        let receipts = ongoing_ops
            .into_iter()
            .map(PendingAccountOpReceipt::from_priority_op)
            .collect();

        Ok(receipts)
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

async fn account_tx_receipts(
    data: web::Data<ApiAccountsData>,
    web::Path(account_query): web::Path<String>,
    web::Query(location_query): web::Query<AccountReceiptsQuery>,
) -> JsonResult<Vec<AccountTxReceipt>> {
    let (location, direction, limit) = location_query.validate()?;
    let address = data.find_account_address(account_query).await?;

    let receipts = data
        .tx_receipts(address, location, direction, limit)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(receipts))
}

async fn account_op_receipts(
    data: web::Data<ApiAccountsData>,
    web::Path(account_query): web::Path<String>,
    web::Query(location_query): web::Query<AccountReceiptsQuery>,
) -> JsonResult<Vec<AccountOpReceipt>> {
    let (location, direction, limit) = location_query.validate()?;
    let address = data.find_account_address(account_query).await?;

    let receipts = data
        .op_receipts(address, location, direction, limit)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(receipts))
}

async fn account_pending_receipts(
    data: web::Data<ApiAccountsData>,
    web::Path(account_query): web::Path<String>,
) -> JsonResult<Vec<PendingAccountOpReceipt>> {
    let address = data.find_account_address(account_query).await?;

    let receipts = data
        .pending_op_receipts(address)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(receipts))
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
        .route(
            "{id}/transactions/receipts",
            web::get().to(account_tx_receipts),
        )
        .route(
            "{id}/operations/receipts",
            web::get().to(account_op_receipts),
        )
        .route(
            "{id}/operations/pending",
            web::get().to(account_pending_receipts),
        )
}
