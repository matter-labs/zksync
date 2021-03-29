use super::{
    error::Error,
    types::{BlockInfo, L2Status, Transaction},
};
use serde::Serialize;
use std::str::FromStr;
use zksync_storage::{chain::block::records::BlockTransactionItem, StorageProcessor};
use zksync_types::{
    aggregated_operations::AggregatedActionType,
    pagination::{BlockAndTxHash, Paginated, PaginationQuery},
    tx::TxHash,
    BlockNumber, Token, TokenId,
};

#[async_trait::async_trait]
pub trait Paginate<T: Serialize> {
    type F: Serialize;

    async fn paginate(
        &mut self,
        query: PaginationQuery<Self::F>,
    ) -> Result<Paginated<T, Self::F>, Error>;
}

#[async_trait::async_trait]
impl Paginate<Token> for StorageProcessor<'_> {
    type F = TokenId;

    async fn paginate(
        &mut self,
        query: PaginationQuery<TokenId>,
    ) -> Result<Paginated<Token, TokenId>, Error> {
        let tokens = self
            .tokens_schema()
            .load_token_page(&query)
            .await
            .map_err(Error::internal)?;
        let count = self
            .tokens_schema()
            .get_count()
            .await
            .map_err(Error::internal)? as u32;
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
    type F = BlockNumber;

    async fn paginate(
        &mut self,
        query: PaginationQuery<BlockNumber>,
    ) -> Result<Paginated<BlockInfo, BlockNumber>, Error> {
        let blocks = self
            .chain()
            .block_schema()
            .load_block_page(&query)
            .await
            .map_err(Error::internal)?;
        let blocks = blocks.into_iter().map(BlockInfo::from).collect::<Vec<_>>();
        let count = *self
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await
            .map_err(Error::internal)?;
        Ok(Paginated::new(
            blocks,
            query.from,
            count,
            query.limit,
            query.direction,
        ))
    }
}

fn transaction_from_items(item: BlockTransactionItem, is_block_finalized: bool) -> Transaction {
    let tx_hash = TxHash::from_str(item.tx_hash.replace("0x", "sync-tx:").as_str()).unwrap();
    let status = if item.success.unwrap_or_default() {
        if is_block_finalized {
            L2Status::Finalized
        } else {
            L2Status::Committed
        }
    } else {
        L2Status::Rejected
    };
    Transaction {
        tx_hash,
        block_number: Some(BlockNumber(item.block_number as u32)),
        op: item.op,
        status,
        fail_reason: item.fail_reason,
        created_at: item.created_at,
    }
}

#[async_trait::async_trait]
impl Paginate<Transaction> for StorageProcessor<'_> {
    type F = BlockAndTxHash;

    async fn paginate(
        &mut self,
        query: PaginationQuery<BlockAndTxHash>,
    ) -> Result<Paginated<Transaction, BlockAndTxHash>, Error> {
        let raw_txs = self
            .chain()
            .block_schema()
            .get_block_transactions_page(&query)
            .await
            .map_err(Error::internal)?;
        let txs: Vec<Transaction>;
        if raw_txs.is_none() {
            return Err(Error::invalid_data(format!(
                "No tx with hash {} in block {}",
                query.from.tx_hash.to_string(),
                query.from.block_number
            )));
        } else {
            let is_block_finalized = self
                .chain()
                .operations_schema()
                .get_stored_aggregated_operation(
                    query.from.block_number,
                    AggregatedActionType::ExecuteBlocks,
                )
                .await
                .map(|operation| operation.confirmed)
                .unwrap_or_default();
            txs = raw_txs
                .unwrap()
                .into_iter()
                .map(|tx| transaction_from_items(tx, is_block_finalized))
                .collect();
        }
        let count = self
            .chain()
            .block_schema()
            .get_block_transactions_count(query.from.block_number)
            .await
            .map_err(Error::internal)?;
        Ok(Paginated::new(
            txs,
            query.from,
            count,
            query.limit,
            query.direction,
        ))
    }
}
