use crate::api_server::rpc_server::types::{BlockInfo, ResponseAccountState};
use crate::utils::token_db_cache::TokenDBCache;
use lru_cache::LruCache;
use zksync_storage::chain::operations::records::StoredExecutedPriorityOperation;
use zksync_storage::chain::operations_ext::records::TxReceiptResponse;
use zksync_storage::ConnectionPool;
use zksync_types::tx::TxHash;
use zksync_types::BlockNumber;
use zksync_types::{AccountId, ActionType, Address};

pub struct NotifierState {
    pub(super) cache_of_executed_priority_operations:
        LruCache<u32, StoredExecutedPriorityOperation>,
    pub(super) cache_of_transaction_receipts: LruCache<Vec<u8>, TxReceiptResponse>,
    pub(super) cache_of_blocks_info: LruCache<BlockNumber, BlockInfo>,
    pub(super) tokens_cache: TokenDBCache,

    pub(super) db_pool: ConnectionPool,
}

impl NotifierState {
    pub fn new(cache_capacity: usize, db_pool: ConnectionPool) -> Self {
        let tokens_cache = TokenDBCache::new(db_pool.clone());

        Self {
            cache_of_executed_priority_operations: LruCache::new(cache_capacity),
            cache_of_transaction_receipts: LruCache::new(cache_capacity),
            cache_of_blocks_info: LruCache::new(cache_capacity),
            tokens_cache,
            db_pool,
        }
    }

    pub async fn get_tx_receipt(
        &mut self,
        hash: &TxHash,
    ) -> Result<Option<TxReceiptResponse>, anyhow::Error> {
        let res = if let Some(tx_receipt) = self
            .cache_of_transaction_receipts
            .get_mut(&hash.as_ref().to_vec())
        {
            Some(tx_receipt.clone())
        } else {
            let mut storage = self.db_pool.access_storage().await?;
            let tx_receipt = storage
                .chain()
                .operations_ext_schema()
                .tx_receipt(hash.as_ref())
                .await?;

            if let Some(tx_receipt) = tx_receipt.clone() {
                if tx_receipt.verified {
                    self.cache_of_transaction_receipts
                        .insert(hash.as_ref().to_vec(), tx_receipt);
                }
            }

            tx_receipt
        };
        Ok(res)
    }

    pub async fn get_block_info(
        &mut self,
        block_number: u32,
    ) -> Result<Option<BlockInfo>, anyhow::Error> {
        let res = if let Some(block_info) = self.cache_of_blocks_info.get_mut(&block_number) {
            block_info.clone()
        } else {
            let mut storage = self.db_pool.access_storage().await?;
            let mut transaction = storage.start_transaction().await?;
            let block_info = if let Some(block_with_op) = transaction
                .chain()
                .block_schema()
                .get_block(block_number)
                .await?
            {
                let verified = if let Some(block_verify) = transaction
                    .chain()
                    .operations_schema()
                    .get_operation(block_number, ActionType::VERIFY)
                    .await
                {
                    block_verify.confirmed
                } else {
                    false
                };

                BlockInfo {
                    block_number: i64::from(block_with_op.block_number),
                    committed: true,
                    verified,
                }
            } else {
                // Tx is executed, but block is not created. Probably, it's in the pending block,
                // no need to worry right now.
                return Ok(None);
            };

            transaction.commit().await?;

            // Unverified blocks can still change, so we can't cache them.
            // Since request for non-existing block will return the last committed block,
            // we must also check that block number matches the requested one.
            if block_info.verified && block_info.block_number == block_number as i64 {
                self.cache_of_blocks_info
                    .insert(block_info.block_number as u32, block_info.clone());
            }

            block_info
        };
        Ok(Some(res))
    }

    pub async fn get_executed_priority_operation(
        &mut self,
        serial_id: u32,
    ) -> Result<Option<StoredExecutedPriorityOperation>, anyhow::Error> {
        let res = if let Some(executed_op) = self
            .cache_of_executed_priority_operations
            .get_mut(&serial_id)
        {
            Some(executed_op.clone())
        } else {
            let mut storage = self.db_pool.access_storage().await?;
            let executed_op = storage
                .chain()
                .operations_schema()
                .get_executed_priority_operation(serial_id)
                .await?;

            if let Some(executed_op) = executed_op.clone() {
                self.cache_of_executed_priority_operations
                    .insert(serial_id, executed_op);
            }

            executed_op
        };
        Ok(res)
    }

    pub async fn get_account_info(
        &self,
        address: Address,
        action: ActionType,
    ) -> anyhow::Result<(AccountId, ResponseAccountState)> {
        let mut storage = self.db_pool.access_storage().await?;
        let account_state = storage
            .chain()
            .account_schema()
            .account_state_by_address(&address)
            .await?;

        let account_id = if let Some(id) = account_state.committed.as_ref().map(|(id, _)| id) {
            *id
        } else {
            anyhow::bail!("AccountId is unknown");
        };

        let account_state = if let Some(account) = match action {
            ActionType::COMMIT => account_state.committed,
            ActionType::VERIFY => account_state.verified,
        }
        .map(|(_, a)| a)
        {
            ResponseAccountState::try_restore(account, &self.tokens_cache).await?
        } else {
            ResponseAccountState::default()
        };

        Ok((account_id, account_state))
    }

    pub async fn get_account_state(
        &self,
        id: AccountId,
        action: ActionType,
    ) -> anyhow::Result<Option<ResponseAccountState>> {
        let mut storage = self.db_pool.access_storage().await?;

        let stored_account = match action {
            ActionType::COMMIT => {
                storage
                    .chain()
                    .account_schema()
                    .last_committed_state_for_account(id)
                    .await?
            }
            ActionType::VERIFY => {
                storage
                    .chain()
                    .account_schema()
                    .last_verified_state_for_account(id)
                    .await?
            }
        };

        let account = if let Some(account) = stored_account {
            ResponseAccountState::try_restore(account, &self.tokens_cache)
                .await
                .ok()
        } else {
            None
        };

        Ok(account)
    }
}
