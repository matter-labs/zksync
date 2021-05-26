// Built-in uses

// External uses

// Workspace uses
use zksync_api_types::v02::{
    block::BlockInfo,
    pagination::{
        AccountTxsRequest, BlockAndTxHash, Paginated, PaginationQuery, PendingOpsRequest,
    },
    transaction::Transaction,
};
use zksync_storage::StorageProcessor;
use zksync_types::{BlockNumber, Token, TokenId};

// Local uses
use super::{
    block::block_info_from_details,
    error::{Error, InvalidDataError},
    paginate_trait::Paginate,
};
use crate::core_api_client::CoreApiClient;

#[async_trait::async_trait]
impl Paginate<Token, TokenId> for StorageProcessor<'_> {
    async fn paginate(
        &mut self,
        query: &PaginationQuery<TokenId>,
    ) -> Result<Paginated<Token, TokenId>, Error> {
        let tokens = self
            .tokens_schema()
            .load_token_page(query)
            .await
            .map_err(Error::storage)?;
        let count = self
            .tokens_schema()
            .get_count()
            .await
            .map_err(Error::storage)? as u32;
        Ok(Paginated::new(
            tokens,
            query.from,
            query.limit,
            query.direction,
            count,
        ))
    }
}

#[async_trait::async_trait]
impl Paginate<BlockInfo, BlockNumber> for StorageProcessor<'_> {
    async fn paginate(
        &mut self,
        query: &PaginationQuery<BlockNumber>,
    ) -> Result<Paginated<BlockInfo, BlockNumber>, Error> {
        let blocks = self
            .chain()
            .block_schema()
            .load_block_page(query)
            .await
            .map_err(Error::storage)?;
        let blocks: Vec<BlockInfo> = blocks.into_iter().map(block_info_from_details).collect();
        let count = *self
            .chain()
            .block_schema()
            .get_last_committed_confirmed_block()
            .await
            .map_err(Error::storage)?;
        Ok(Paginated::new(
            blocks,
            query.from,
            query.limit,
            query.direction,
            count,
        ))
    }
}

#[async_trait::async_trait]
impl Paginate<Transaction, BlockAndTxHash> for StorageProcessor<'_> {
    async fn paginate(
        &mut self,
        query: &PaginationQuery<BlockAndTxHash>,
    ) -> Result<Paginated<Transaction, BlockAndTxHash>, Error> {
        let txs = self
            .chain()
            .block_schema()
            .get_block_transactions_page(query)
            .await
            .map_err(Error::storage)?
            .ok_or_else(|| Error::from(InvalidDataError::TransactionNotFound))?;
        let count = self
            .chain()
            .block_schema()
            .get_block_transactions_count(query.from.block_number)
            .await
            .map_err(Error::storage)?;
        Ok(Paginated::new(
            txs,
            query.from,
            query.limit,
            query.direction,
            count,
        ))
    }
}

#[async_trait::async_trait]
impl Paginate<Transaction, AccountTxsRequest> for StorageProcessor<'_> {
    async fn paginate(
        &mut self,
        query: &PaginationQuery<AccountTxsRequest>,
    ) -> Result<Paginated<Transaction, AccountTxsRequest>, Error> {
        let txs = self
            .chain()
            .operations_ext_schema()
            .get_account_transactions(query)
            .await
            .map_err(Error::storage)?
            .ok_or_else(|| Error::from(InvalidDataError::TransactionNotFound))?;
        let count = self
            .chain()
            .operations_ext_schema()
            .get_account_transactions_count(query.from.address)
            .await
            .map_err(Error::storage)?;
        Ok(Paginated::new(
            txs,
            query.from,
            query.limit,
            query.direction,
            count,
        ))
    }
}

#[async_trait::async_trait]
impl Paginate<Transaction, PendingOpsRequest> for CoreApiClient {
    async fn paginate(
        &mut self,
        query: &PaginationQuery<PendingOpsRequest>,
    ) -> Result<Paginated<Transaction, PendingOpsRequest>, Error> {
        let result = self
            .get_unconfirmed_ops(query)
            .await
            .map_err(Error::core_api)?;
        Ok(result)
    }
}
