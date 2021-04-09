// Built-in uses

// External uses

// Workspace uses
use zksync_api_types::v02::pagination::{BlockAndTxHash, Paginated, PaginationQuery};
use zksync_storage::StorageProcessor;
use zksync_types::{aggregated_operations::AggregatedActionType, BlockNumber, Token, TokenId};

// Local uses
use super::{
    block::BlockInfo,
    error::{Error, TxError},
    paginate_trait::Paginate,
    transaction::Transaction,
};

#[async_trait::async_trait]
impl Paginate<Token> for StorageProcessor<'_> {
    type Index = TokenId;

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
            count,
            query.limit,
            query.direction,
        ))
    }
}

#[async_trait::async_trait]
impl Paginate<BlockInfo> for StorageProcessor<'_> {
    type Index = BlockNumber;

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
        let blocks: Vec<BlockInfo> = blocks.into_iter().map(BlockInfo::from).collect();
        let count = *self
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await
            .map_err(Error::storage)?;
        Ok(Paginated::new(
            blocks,
            query.from,
            count,
            query.limit,
            query.direction,
        ))
    }
}

#[async_trait::async_trait]
impl Paginate<Transaction> for StorageProcessor<'_> {
    type Index = BlockAndTxHash;

    async fn paginate(
        &mut self,
        query: &PaginationQuery<BlockAndTxHash>,
    ) -> Result<Paginated<Transaction, BlockAndTxHash>, Error> {
        let raw_txs = self
            .chain()
            .block_schema()
            .get_block_transactions_page(query)
            .await
            .map_err(Error::storage)?
            .ok_or_else(|| Error::from(TxError::TransactionNotFound))?;
        let is_block_finalized = self
            .chain()
            .operations_schema()
            .get_stored_aggregated_operation(
                query.from.block_number,
                AggregatedActionType::ExecuteBlocks,
            )
            .await
            .map(|operation| operation.confirmed)
            .unwrap_or(false);
        let txs = raw_txs
            .into_iter()
            .map(|tx| Transaction::from_item_and_finalization(tx, is_block_finalized))
            .collect();
        let count = self
            .chain()
            .block_schema()
            .get_block_transactions_count(query.from.block_number)
            .await
            .map_err(Error::storage)?;
        Ok(Paginated::new(
            txs,
            query.from,
            count,
            query.limit,
            query.direction,
        ))
    }
}
