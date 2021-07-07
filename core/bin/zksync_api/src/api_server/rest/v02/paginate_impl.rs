// Built-in uses

// External uses

// Workspace uses
use zksync_api_types::{
    v02::{
        block::BlockInfo,
        pagination::{
            AccountTxsRequest, ApiEither, BlockAndTxHash, Paginated, PaginationQuery,
            PendingOpsRequest,
        },
        transaction::{Transaction, TxHashSerializeWrapper},
    },
    Either,
};
use zksync_storage::StorageProcessor;
use zksync_types::{BlockNumber, SerialId, Token, TokenId};

// Local uses
use super::{
    block::block_info_from_details,
    error::{Error, InvalidDataError},
    paginate_trait::Paginate,
};
use crate::core_api_client::CoreApiClient;

#[async_trait::async_trait]
impl Paginate<ApiEither<TokenId>> for StorageProcessor<'_> {
    type OutputObj = Token;
    type OutputId = TokenId;

    async fn paginate(
        &mut self,
        query: &PaginationQuery<ApiEither<TokenId>>,
    ) -> Result<Paginated<Token, TokenId>, Error> {
        let mut transaction = self.start_transaction().await.map_err(Error::storage)?;

        let token_id = match query.from.inner {
            Either::Left(token_id) => token_id,
            Either::Right(_) => TokenId(
                transaction
                    .tokens_schema()
                    .get_count()
                    .await
                    .map_err(Error::storage)?,
            ),
        };

        let query = PaginationQuery {
            from: token_id,
            limit: query.limit,
            direction: query.direction,
        };

        let tokens = transaction
            .tokens_schema()
            .load_token_page(&query)
            .await
            .map_err(Error::storage)?;
        let count = transaction
            .tokens_schema()
            .get_count()
            .await
            .map_err(Error::storage)?;
        transaction.commit().await.map_err(Error::storage)?;

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
impl Paginate<ApiEither<BlockNumber>> for StorageProcessor<'_> {
    type OutputObj = BlockInfo;
    type OutputId = BlockNumber;

    async fn paginate(
        &mut self,
        query: &PaginationQuery<ApiEither<BlockNumber>>,
    ) -> Result<Paginated<BlockInfo, BlockNumber>, Error> {
        let mut transaction = self.start_transaction().await.map_err(Error::storage)?;

        let last_block = transaction
            .chain()
            .block_schema()
            .get_last_committed_confirmed_block()
            .await
            .map_err(Error::storage)?;

        let block_number = match query.from.inner {
            Either::Left(block_number) => block_number,
            Either::Right(_) => last_block,
        };

        let query = PaginationQuery {
            from: block_number,
            limit: query.limit,
            direction: query.direction,
        };

        let blocks = transaction
            .chain()
            .block_schema()
            .load_block_page(&query)
            .await
            .map_err(Error::storage)?;
        let blocks: Vec<BlockInfo> = blocks.into_iter().map(block_info_from_details).collect();

        transaction.commit().await.map_err(Error::storage)?;

        Ok(Paginated::new(
            blocks,
            query.from,
            query.limit,
            query.direction,
            *last_block,
        ))
    }
}

#[async_trait::async_trait]
impl Paginate<BlockAndTxHash> for StorageProcessor<'_> {
    type OutputObj = Transaction;
    type OutputId = TxHashSerializeWrapper;

    async fn paginate(
        &mut self,
        query: &PaginationQuery<BlockAndTxHash>,
    ) -> Result<Paginated<Transaction, TxHashSerializeWrapper>, Error> {
        let mut transaction = self.start_transaction().await.map_err(Error::storage)?;

        let tx_hash = match query.from.tx_hash.inner {
            Either::Left(tx_hash) => tx_hash,
            Either::Right(_) => {
                if let Some(tx_hash) = transaction
                    .chain()
                    .operations_ext_schema()
                    .get_block_last_tx_hash(query.from.block_number)
                    .await
                    .map_err(Error::storage)?
                {
                    tx_hash
                } else {
                    return Ok(Paginated::new(
                        Vec::new(),
                        Default::default(),
                        query.limit,
                        query.direction,
                        0,
                    ));
                }
            }
        };

        let query = PaginationQuery {
            from: BlockAndTxHash {
                block_number: query.from.block_number,
                tx_hash: ApiEither::from(tx_hash),
            },
            limit: query.limit,
            direction: query.direction,
        };

        let txs = transaction
            .chain()
            .block_schema()
            .get_block_transactions_page(&query)
            .await
            .map_err(Error::storage)?
            .ok_or_else(|| Error::from(InvalidDataError::TransactionNotFound))?;
        let count = transaction
            .chain()
            .block_schema()
            .get_block_transactions_count(query.from.block_number)
            .await
            .map_err(Error::storage)?;

        transaction.commit().await.map_err(Error::storage)?;

        Ok(Paginated::new(
            txs,
            TxHashSerializeWrapper(tx_hash),
            query.limit,
            query.direction,
            count,
        ))
    }
}

#[async_trait::async_trait]
impl Paginate<AccountTxsRequest> for StorageProcessor<'_> {
    type OutputObj = Transaction;
    type OutputId = TxHashSerializeWrapper;

    async fn paginate(
        &mut self,
        query: &PaginationQuery<AccountTxsRequest>,
    ) -> Result<Paginated<Transaction, TxHashSerializeWrapper>, Error> {
        let mut transaction = self.start_transaction().await.map_err(Error::storage)?;

        let tx_hash = match query.from.tx_hash.inner {
            Either::Left(tx_hash) => tx_hash,
            Either::Right(_) => {
                if let Some(tx_hash) = transaction
                    .chain()
                    .operations_ext_schema()
                    .get_account_last_tx_hash(query.from.address)
                    .await
                    .map_err(Error::storage)?
                {
                    tx_hash
                } else {
                    return Ok(Paginated::new(
                        Vec::new(),
                        Default::default(),
                        query.limit,
                        query.direction,
                        0,
                    ));
                }
            }
        };

        let query = PaginationQuery {
            from: AccountTxsRequest {
                address: query.from.address,
                tx_hash: ApiEither::from(tx_hash),
            },
            limit: query.limit,
            direction: query.direction,
        };

        let txs = transaction
            .chain()
            .operations_ext_schema()
            .get_account_transactions(&query)
            .await
            .map_err(Error::storage)?
            .ok_or_else(|| Error::from(InvalidDataError::TransactionNotFound))?;
        let count = transaction
            .chain()
            .operations_ext_schema()
            .get_account_transactions_count(query.from.address)
            .await
            .map_err(Error::storage)?;

        transaction.commit().await.map_err(Error::storage)?;

        Ok(Paginated::new(
            txs,
            TxHashSerializeWrapper(tx_hash),
            query.limit,
            query.direction,
            count,
        ))
    }
}

#[async_trait::async_trait]
impl Paginate<PendingOpsRequest> for CoreApiClient {
    type OutputObj = Transaction;
    type OutputId = SerialId;

    async fn paginate(
        &mut self,
        query: &PaginationQuery<PendingOpsRequest>,
    ) -> Result<Paginated<Transaction, SerialId>, Error> {
        let result = self
            .get_unconfirmed_ops(&query)
            .await
            .map_err(Error::core_api)?;
        Ok(result)
    }
}
