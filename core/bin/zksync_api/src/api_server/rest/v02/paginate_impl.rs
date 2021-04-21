// Built-in uses

// External uses

// Workspace uses
use zksync_api_types::v02::{
    block::BlockInfo,
    pagination::{BlockAndTxHash, Paginated, PaginationQuery},
    transaction::Transaction,
};
use zksync_storage::StorageProcessor;
use zksync_types::{BlockNumber, Token, TokenId};

// Local uses
use super::{
    block::block_info_from_details,
    error::{Error, TxError},
    paginate_trait::Paginate,
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
            query.limit,
            query.direction,
            count,
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
        let blocks: Vec<BlockInfo> = blocks.into_iter().map(block_info_from_details).collect();
        let count = *self
            .chain()
            .block_schema()
            .get_last_committed_block()
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
impl Paginate<Transaction> for StorageProcessor<'_> {
    type Index = BlockAndTxHash;

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
            .ok_or_else(|| Error::from(TxError::TransactionNotFound))?;
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
