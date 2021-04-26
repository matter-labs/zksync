// Built-in uses

// External uses

// Workspace uses
use zksync_api_types::v02::{
    block::BlockInfo,
    pagination::{
        BlockAndTxHash, Paginated, PaginationDirection, PaginationQuery, PendingOpsRequest,
    },
    transaction::{L1Transaction, L2Status, Transaction, TransactionData},
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
impl Paginate<Transaction> for CoreApiClient {
    type Index = PendingOpsRequest;

    async fn paginate(
        &mut self,
        query: &PaginationQuery<PendingOpsRequest>,
    ) -> Result<Paginated<Transaction, PendingOpsRequest>, Error> {
        let mut all_ops = self
            .get_unconfirmed_ops(query.from.address, query.from.account_id)
            .await
            .map_err(Error::core_api)?;
        let count = all_ops.len();

        let index = match query.direction {
            PaginationDirection::Newer => {
                all_ops.sort_by(|a, b| a.serial_id.cmp(&b.serial_id));
                all_ops
                    .iter()
                    .position(|a| a.serial_id >= query.from.serial_id)
            }
            PaginationDirection::Older => {
                all_ops.sort_by(|a, b| b.serial_id.cmp(&a.serial_id));
                all_ops
                    .iter()
                    .position(|a| a.serial_id <= query.from.serial_id)
            }
        };

        let list = match index {
            Some(index) => {
                let mut ops: Vec<_> = all_ops[index..].into_iter().collect();
                ops.truncate(query.limit as usize);
                ops.into_iter()
                    .map(|op| {
                        let tx_hash = op.tx_hash();
                        let tx = L1Transaction::from_pending_op(
                            op.data.clone(),
                            op.eth_hash,
                            op.serial_id,
                            tx_hash,
                        );
                        Transaction {
                            tx_hash,
                            block_number: None,
                            op: TransactionData::L1(tx),
                            status: L2Status::Queued,
                            fail_reason: None,
                            created_at: None,
                        }
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        Ok(Paginated::new(
            list,
            query.from,
            query.limit,
            query.direction,
            count as u32,
        ))
    }
}
