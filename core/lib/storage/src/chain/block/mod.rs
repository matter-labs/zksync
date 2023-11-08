// Built-in deps
use std::time::{Instant, SystemTime, UNIX_EPOCH};
// External imports
// Workspace imports
use zksync_api_types::{
    v02::{
        pagination::{BlockAndTxHash, PaginationDirection, PaginationQuery},
        transaction::Transaction,
    },
    Either,
};
use zksync_crypto::convert::FeConvert;
use zksync_types::{
    aggregated_operations::AggregatedActionType,
    block::{Block, BlockMetadata, ExecutedOperations, IncompleteBlock, PendingBlock},
    event::block::BlockStatus,
    AccountId, BlockNumber, Fr, ZkSyncOp, H256, U256,
};
// Local imports
use self::records::{
    BlockTransactionItem, StorageBlock, StorageBlockDetails, StorageBlockMetadata,
    StoragePendingBlock, StorageRootHash, TransactionItem,
};
use crate::{
    chain::operations::{
        records::{
            NewExecutedPriorityOperation, NewExecutedTransaction, StoredExecutedPriorityOperation,
            StoredExecutedTransaction,
        },
        OperationsSchema,
    },
    chain::{account::records::EthAccountType, block::records::StorageIncompleteBlock},
    QueryResult, StorageProcessor,
};

pub(crate) mod conversion;
pub mod records;

/// Block schema is a primary sidechain storage controller.
///
/// Besides block getters/setters, it provides an `execute_operation` method,
/// which is essential for the sidechain logic, as it causes the state updates in the chain.
#[derive(Debug)]
pub struct BlockSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> BlockSchema<'a, 'c> {
    /// Given a block, stores its transactions in the database.
    pub async fn save_block_transactions(
        &mut self,
        block_number: BlockNumber,
        operations: Vec<ExecutedOperations>,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        for block_tx in operations.into_iter() {
            match block_tx {
                ExecutedOperations::Tx(tx) => {
                    // Update account type
                    // This method is called in the committer, so account type update takes effect
                    // starting the next miniblock. If the user wishes to send ChangePubKey + another Tx from
                    // CREATE2 account in the same miniblock, they will have to do it in a batch
                    if let Some(ZkSyncOp::ChangePubKeyOffchain(tx)) = &tx.op {
                        let account_type = if matches!(&tx.tx.eth_auth_data, Some(auth) if auth.is_create2())
                        {
                            EthAccountType::CREATE2
                        } else {
                            EthAccountType::Owned
                        };

                        let current_type = transaction
                            .chain()
                            .account_schema()
                            .account_type_by_id(tx.account_id)
                            .await?;

                        let new_account_type = match (current_type, account_type) {
                            // You can not change No2FA to Owned here
                            (Some(EthAccountType::No2FA(hash)), EthAccountType::Owned) => {
                                EthAccountType::No2FA(hash)
                            }
                            _ => account_type,
                        };

                        transaction
                            .chain()
                            .account_schema()
                            .set_account_type(tx.account_id, new_account_type)
                            .await?;
                    }

                    let new_tx = NewExecutedTransaction::prepare_stored_tx(
                        *tx,
                        block_number,
                        &mut transaction,
                    )
                    .await?;
                    transaction
                        .chain()
                        .operations_schema()
                        .store_executed_tx(new_tx)
                        .await?;
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    // Store the executed operation in the corresponding schema.
                    // For priority operation we should only store it in the Operations schema.
                    let new_priority_op = NewExecutedPriorityOperation::prepare_stored_priority_op(
                        *prior_op,
                        block_number,
                    );

                    // Store the executed operation in the corresponding schema.
                    transaction
                        .chain()
                        .operations_schema()
                        .store_executed_priority_op(new_priority_op)
                        .await?;
                }
            }
        }

        transaction.commit().await?;
        metrics::histogram!("sql.chain.block.save_block_transactions", start.elapsed());
        Ok(())
    }

    // Helper method for retrieving blocks from the database.
    pub async fn get_storage_block(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Option<StorageBlock>> {
        let start = Instant::now();
        let block = sqlx::query_as!(
            StorageBlock,
            "SELECT * FROM blocks WHERE number = $1",
            i64::from(*block)
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.block.get_storage_block", start.elapsed());

        Ok(block)
    }

    /// Given the block number, attempts to retrieve it from the database.
    /// Returns `None` if the block with provided number does not exist yet.
    pub async fn get_block(&mut self, block: BlockNumber) -> QueryResult<Option<Block>> {
        let start = Instant::now();
        // Load block header.
        let stored_block = if let Some(block) = self.get_storage_block(block).await? {
            block
        } else {
            return Ok(None);
        };

        // Load transactions for this block.
        let block_transactions = self.get_block_executed_ops(block).await?;

        // Encode the root hash as `0xFF..FF`.
        let new_root_hash =
            FeConvert::from_bytes(&stored_block.root_hash).expect("Unparsable root hash");

        let commitment = H256::from_slice(&stored_block.commitment);
        // Return the obtained block in the expected format.
        let result = Some(Block::new(
            block,
            new_root_hash,
            AccountId(stored_block.fee_account_id as u32),
            block_transactions,
            (
                stored_block.unprocessed_prior_op_before as u64,
                stored_block.unprocessed_prior_op_after as u64,
            ),
            stored_block.block_size as usize,
            U256::from(stored_block.commit_gas_limit as u64),
            U256::from(stored_block.verify_gas_limit as u64),
            commitment,
            stored_block.timestamp.unwrap_or_default() as u64,
        ));

        metrics::histogram!("sql.chain.block.get_block", start.elapsed());

        Ok(result)
    }

    /// Given the block number, attempts to get metadata related to block.
    /// Returns `None` if not found.
    pub async fn get_block_metadata(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Option<BlockMetadata>> {
        let start = Instant::now();

        let db_result = sqlx::query_as!(
            StorageBlockMetadata,
            "SELECT * FROM block_metadata WHERE block_number = $1",
            i64::from(*block)
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.block.get_block_metadata", start.elapsed());

        let result = db_result.map(|md| BlockMetadata {
            fast_processing: md.fast_processing,
        });

        Ok(result)
    }

    /// Same as `get_block_executed_ops`, but returns a vector of `ZkSyncOp` instead
    /// of `ExecutedOperations`.
    pub async fn get_block_operations(&mut self, block: BlockNumber) -> QueryResult<Vec<ZkSyncOp>> {
        let start = Instant::now();
        let executed_ops = self.get_block_executed_ops(block).await?;
        let result = executed_ops
            .into_iter()
            .filter_map(|exec_op| match exec_op {
                ExecutedOperations::Tx(tx) => tx.op,
                ExecutedOperations::PriorityOp(priorop) => Some(priorop.op),
            })
            .collect();
        metrics::histogram!("sql.chain.block.get_block_operations", start.elapsed());
        Ok(result)
    }

    /// Retrieves both L1 and L2 operations stored in the block with the given number.
    pub async fn get_block_transactions(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Vec<BlockTransactionItem>> {
        let start = Instant::now();
        let block_txs = sqlx::query_as!(
            BlockTransactionItem,
            r#"
                WITH transactions AS (
                    SELECT
                        '0x' || encode(tx_hash, 'hex') as tx_hash,
                        tx as op,
                        block_number,
                        block_index,
                        success,
                        fail_reason,
                        created_at,
                        batch_id,
                        sequence_number
                    FROM executed_transactions
                    WHERE block_number = $1
                ), priority_ops AS (
                    SELECT
                        '0x' || encode(eth_hash, 'hex') as tx_hash,
                        operation as op,
                        block_number,
                        block_index as "block_index?",
                        true as success,
                        Null as fail_reason,
                        created_at,
                        Null::bigint as batch_id,
                        sequence_number
                    FROM executed_priority_operations
                    WHERE block_number = $1
                ), everything AS (
                    SELECT * FROM transactions
                    UNION ALL
                    SELECT * FROM priority_ops
                )
                SELECT
                    tx_hash as "tx_hash!",
                    block_number as "block_number!",
                    op as "op!",
                    block_index as "block_index?",
                    success as "success!",
                    fail_reason as "fail_reason?",
                    created_at as "created_at!",
                    batch_id as "batch_id?"
                FROM everything
                ORDER BY sequence_number DESC
            "#,
            i64::from(*block)
        )
        .fetch_all(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.block.get_block_transactions", start.elapsed());
        Ok(block_txs)
    }

    /// Given the block number, loads all the operations that were executed in that block.
    pub async fn get_block_executed_ops(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Vec<ExecutedOperations>> {
        let start = Instant::now();
        let mut executed_operations = Vec::new();

        // Load both executed transactions and executed priority operations
        // from the database.
        let (executed_ops, executed_priority_ops) = {
            let executed_ops = sqlx::query_as!(
                StoredExecutedTransaction,
                "SELECT * FROM executed_transactions WHERE block_number = $1",
                i64::from(*block)
            )
            .fetch_all(self.0.conn())
            .await?;

            let executed_priority_ops = sqlx::query_as!(
                StoredExecutedPriorityOperation,
                "SELECT * FROM executed_priority_operations WHERE block_number = $1",
                i64::from(*block)
            )
            .fetch_all(self.0.conn())
            .await?;

            (executed_ops, executed_priority_ops)
        };

        // Transform executed operations to be `ExecutedOperations`.
        let executed_ops = executed_ops
            .into_iter()
            .map(|stored_exec| stored_exec.into_executed_tx())
            .map(|tx| ExecutedOperations::Tx(Box::new(tx)));
        executed_operations.extend(executed_ops);

        // Transform executed priority operations to be `ExecutedOperations`.
        let executed_priority_ops = executed_priority_ops
            .into_iter()
            .map(|op| ExecutedOperations::PriorityOp(Box::new(op.into_executed())));
        executed_operations.extend(executed_priority_ops);

        // Sort the operations, so all the failed operations will be at the very end
        // of the list.
        executed_operations.sort_by_key(|exec_op| {
            match exec_op {
                ExecutedOperations::Tx(tx) => {
                    if let Some(idx) = tx.block_index {
                        idx
                    } else {
                        // failed operations are at the end.
                        u32::MAX
                    }
                }
                ExecutedOperations::PriorityOp(op) => op.block_index,
            }
        });

        metrics::histogram!("sql.chain.block.get_block_executed_ops", start.elapsed());
        Ok(executed_operations)
    }

    /// Loads the block headers for the given amount of blocks in the descending order.
    pub async fn load_block_range_desc(
        &mut self,
        max_block: BlockNumber,
        limit: u32,
    ) -> QueryResult<Vec<StorageBlockDetails>> {
        let start = Instant::now();
        // This query does the following:
        // - joins the `operations` and `eth_tx_hashes` (using the intermediate `eth_ops_binding` table)
        //   tables to collect the data:
        //   block number, ethereum transaction hash, action type and action creation timestamp;
        // - joins the `blocks` table with result of the join twice: once for committed operations
        //   and verified operations;
        // - collects the {limit} blocks in the descending order with the data gathered above.
        let details = sqlx::query_as!(
            StorageBlockDetails,
            r#"
            WITH aggr_comm AS (
                SELECT 
                    aggregate_operations.created_at, 
                    eth_operations.final_hash, 
                    commit_aggregated_blocks_binding.block_number 
                FROM aggregate_operations
                    INNER JOIN commit_aggregated_blocks_binding ON aggregate_operations.id = commit_aggregated_blocks_binding.op_id
                    INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                    INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                WHERE aggregate_operations.confirmed = true 
            ),
            aggr_exec as (
                 SELECT 
                    aggregate_operations.created_at, 
                    eth_operations.final_hash, 
                    execute_aggregated_blocks_binding.block_number 
                FROM aggregate_operations
                    INNER JOIN execute_aggregated_blocks_binding ON aggregate_operations.id = execute_aggregated_blocks_binding.op_id
                    INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                    INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                WHERE aggregate_operations.confirmed = true 
            )
            SELECT
                blocks.number AS "block_number!",
                blocks.root_hash AS "new_state_root!",
                blocks.block_size AS "block_size!",
                committed.final_hash AS "commit_tx_hash?",
                verified.final_hash AS "verify_tx_hash?",
                committed.created_at AS "committed_at!",
                verified.created_at AS "verified_at?"
            FROM blocks
                     INNER JOIN aggr_comm committed ON blocks.number = committed.block_number
                     LEFT JOIN aggr_exec verified ON blocks.number = verified.block_number
            WHERE
                blocks.number <= $1
            ORDER BY blocks.number DESC
            LIMIT $2;
            "#,
            i64::from(*max_block),
            i64::from(limit)
        ).fetch_all(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.block.load_block_range", start.elapsed());
        Ok(details)
    }

    /// Loads the block headers for the given amount of blocks in the ascending order.
    pub async fn load_block_range_asc(
        &mut self,
        min_block: BlockNumber,
        limit: u32,
    ) -> QueryResult<Vec<StorageBlockDetails>> {
        let start = Instant::now();
        // This query does the following:
        // - joins the `operations` and `eth_tx_hashes` (using the intermediate `eth_ops_binding` table)
        //   tables to collect the data:
        //   block number, ethereum transaction hash, action type and action creation timestamp;
        // - joins the `blocks` table with result of the join twice: once for committed operations
        //   and verified operations;
        // - collects the {limit} blocks in the ascending order with the data gathered above.
        let details = sqlx::query_as!(
            StorageBlockDetails,
            r#"
            WITH aggr_comm AS (
                SELECT 
                    aggregate_operations.created_at, 
                    eth_operations.final_hash, 
                    commit_aggregated_blocks_binding.block_number 
                FROM aggregate_operations
                    INNER JOIN commit_aggregated_blocks_binding ON aggregate_operations.id = commit_aggregated_blocks_binding.op_id
                    INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                    INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                WHERE aggregate_operations.confirmed = true 
            ),
            aggr_exec as (
                 SELECT 
                    aggregate_operations.created_at, 
                    eth_operations.final_hash, 
                    execute_aggregated_blocks_binding.block_number 
                FROM aggregate_operations
                    INNER JOIN execute_aggregated_blocks_binding ON aggregate_operations.id = execute_aggregated_blocks_binding.op_id
                    INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                    INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                WHERE aggregate_operations.confirmed = true 
            )
            SELECT
                blocks.number AS "block_number!",
                blocks.root_hash AS "new_state_root!",
                blocks.block_size AS "block_size!",
                committed.final_hash AS "commit_tx_hash?",
                verified.final_hash AS "verify_tx_hash?",
                committed.created_at AS "committed_at!",
                verified.created_at AS "verified_at?"
            FROM blocks
                     INNER JOIN aggr_comm committed ON blocks.number = committed.block_number
                     LEFT JOIN aggr_exec verified ON blocks.number = verified.block_number
            WHERE
                blocks.number >= $1
            ORDER BY blocks.number ASC
            LIMIT $2;
            "#,
            i64::from(*min_block),
            i64::from(limit)
        ).fetch_all(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.block.load_block_range_asc", start.elapsed());
        Ok(details)
    }

    /// Loads the block headers for the given pagination query
    pub async fn load_block_page(
        &mut self,
        query: &PaginationQuery<BlockNumber>,
    ) -> QueryResult<Vec<StorageBlockDetails>> {
        let details = match query.direction {
            PaginationDirection::Newer => {
                self.load_block_range_asc(query.from, query.limit).await?
            }
            PaginationDirection::Older => {
                self.load_block_range_desc(query.from, query.limit).await?
            }
        };

        Ok(details)
    }

    /// Performs a database search with an uncertain query, which can be either of:
    /// - Hash of commit/verify Ethereum transaction for the block.
    /// - The state root hash of the block.
    /// - The number of the block.
    ///
    /// Will return `None` if the query is malformed or there is no block that matches
    /// the query.
    pub async fn find_block_by_height_or_hash(
        &mut self,
        query: String,
    ) -> Option<StorageBlockDetails> {
        let start = Instant::now();
        // If the input looks like hash, add the hash lookup part.
        // return an empty vector if it's not a hex string
        let hash_bytes = conversion::decode_hex_with_prefix(&query).unwrap_or_default();

        // If the input can be interpreted as integer, add the block number lookup part.
        let block_number = if let Ok(int_query) = query.parse::<i64>() {
            int_query
        } else {
            // It doesn't look like a number, provide -1 for no match.
            -1i64
        };

        // If input doesn't look like hash or integer, no query
        // should be performed.
        if block_number == -1i64 && hash_bytes.is_empty() {
            return None;
        }

        // This query does the following:
        // - joins the `operations` and `eth_tx_hashes` (using the intermediate `eth_ops_binding` table)
        //   tables to collect the data:
        //   block number, ethereum transaction hash, action type and action creation timestamp;
        // - joins the `blocks` table with result of the join twice: once for committed operations
        //   and verified operations;
        // - takes the only block that satisfies one of the following criteria
        //   + query equals to the ETH commit transaction hash (in form of `0x00{..}00`);
        //   + query equals to the ETH verify transaction hash (in form of `0x00{..}00`);
        //   + query equals to the state hash obtained in the block (in form of `sync-bl:00{..}00`);
        //   + query equals to the number of the block.
        let result = sqlx::query_as!(
            StorageBlockDetails,
            r#"
            WITH aggr_comm AS (
                SELECT 
                    aggregate_operations.created_at, 
                    eth_operations.final_hash, 
                    commit_aggregated_blocks_binding.block_number 
                FROM aggregate_operations
                    INNER JOIN commit_aggregated_blocks_binding ON aggregate_operations.id = commit_aggregated_blocks_binding.op_id
                    INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                    INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                WHERE aggregate_operations.confirmed = true 
            ),
            aggr_exec as (
                 SELECT 
                    aggregate_operations.created_at, 
                    eth_operations.final_hash, 
                    execute_aggregated_blocks_binding.block_number 
                FROM aggregate_operations
                    INNER JOIN execute_aggregated_blocks_binding ON aggregate_operations.id = execute_aggregated_blocks_binding.op_id
                    INNER JOIN eth_aggregated_ops_binding ON aggregate_operations.id = eth_aggregated_ops_binding.op_id
                    INNER JOIN eth_operations ON eth_operations.id = eth_aggregated_ops_binding.eth_op_id
                WHERE aggregate_operations.confirmed = true 
            )
            SELECT
                blocks.number AS "block_number!",
                blocks.root_hash AS "new_state_root!",
                blocks.block_size AS "block_size!",
                committed.final_hash AS "commit_tx_hash?",
                verified.final_hash AS "verify_tx_hash?",
                committed.created_at AS "committed_at!",
                verified.created_at AS "verified_at?"
            FROM blocks
                     INNER JOIN aggr_comm committed ON blocks.number = committed.block_number
                     LEFT JOIN aggr_exec verified ON blocks.number = verified.block_number
            WHERE false
                OR committed.final_hash = $1
                OR verified.final_hash = $1
                OR blocks.root_hash = $1
                OR blocks.number = $2
            ORDER BY blocks.number DESC
            LIMIT 1;
            "#,
            hash_bytes,
            block_number
        ).fetch_optional(self.0.conn())
            .await
            .ok()
            .flatten();

        metrics::histogram!(
            "sql.chain.block.find_block_by_height_or_hash",
            start.elapsed()
        );
        result
    }

    /// Returns the number of last block saved to the database.
    pub async fn get_last_saved_block(&mut self) -> QueryResult<BlockNumber> {
        let start = Instant::now();
        let count = sqlx::query!("SELECT MAX(number) FROM blocks")
            .fetch_one(self.0.conn())
            .await?
            .max
            .unwrap_or(0);
        metrics::histogram!("sql.chain.block.get_last_committed_block", start.elapsed());
        Ok(BlockNumber(count as u32))
    }

    /// Returns the number of last block for which an aggregated operation exists.
    pub async fn get_last_committed_block(&mut self) -> QueryResult<BlockNumber> {
        let start = Instant::now();
        let result = OperationsSchema(self.0)
            .get_last_block_by_aggregated_action(AggregatedActionType::CommitBlocks, None)
            .await;
        metrics::histogram!("sql.chain.block.get_last_committed_block", start.elapsed());
        result
    }

    /// Returns the number of existing incomplete block.
    /// Returns `None` if there are no incomplte blocks in the database.
    ///
    /// Note: Used only for testing.
    #[cfg(test)]
    pub(crate) async fn get_last_incomplete_block_number(
        &mut self,
    ) -> QueryResult<Option<BlockNumber>> {
        let start = Instant::now();
        let result = sqlx::query!("SELECT max(number) FROM incomplete_blocks")
            .fetch_one(self.0.conn())
            .await?
            .max
            .map(|block| BlockNumber(block as u32));
        metrics::histogram!("sql.chain.block.get_last_incomplete_block", start.elapsed());
        Ok(result)
    }

    /// Returns the number of last block which commit is confirmed on Ethereum.
    pub async fn get_last_committed_confirmed_block(&mut self) -> QueryResult<BlockNumber> {
        let start = Instant::now();
        let result = OperationsSchema(self.0)
            .get_last_block_by_aggregated_action(AggregatedActionType::CommitBlocks, Some(true))
            .await;
        metrics::histogram!(
            "sql.chain.block.get_last_committed_confirmed_block",
            start.elapsed()
        );
        result
    }

    /// Returns the number of last block for which proof has been created.
    ///
    /// Note: having a proof for the block doesn't mean that state was updated. Chain state
    /// is updated only after corresponding transaction is confirmed on the Ethereum blockchain.
    /// In order to see the last block with updated state, use `get_last_verified_confirmed_block` method.
    pub async fn get_last_verified_block(&mut self) -> QueryResult<BlockNumber> {
        let start = Instant::now();
        let result = OperationsSchema(self.0)
            .get_last_block_by_aggregated_action(AggregatedActionType::ExecuteBlocks, None)
            .await;
        metrics::histogram!("sql.chain.block.get_last_verified_block", start.elapsed());
        result
    }

    /// Returns the number of last block for which proof has been confirmed on Ethereum.
    pub async fn get_last_proven_confirmed_block(&mut self) -> QueryResult<BlockNumber> {
        let start = Instant::now();
        let result = OperationsSchema(self.0)
            .get_last_block_by_aggregated_action(
                AggregatedActionType::PublishProofBlocksOnchain,
                Some(true),
            )
            .await;
        metrics::histogram!(
            "sql.chain.block.get_last_proven_confirmed_block",
            start.elapsed()
        );
        result
    }

    /// Returns the number of last block for which executed operations has been confirmed on Ethereum .
    /// Essentially, it's number of last block for which updates were applied to the chain state.
    pub async fn get_last_verified_confirmed_block(&mut self) -> QueryResult<BlockNumber> {
        let start = Instant::now();
        let result = OperationsSchema(self.0)
            .get_last_block_by_aggregated_action(AggregatedActionType::ExecuteBlocks, Some(true))
            .await;
        metrics::histogram!(
            "sql.chain.block.get_last_verified_confirmed_block",
            start.elapsed()
        );
        result
    }

    pub async fn is_block_finalized(&mut self, block_number: BlockNumber) -> QueryResult<bool> {
        let last_finalized_block = self
            .0
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await?;
        Ok(block_number <= last_finalized_block)
    }

    pub async fn pending_block_chunks_left(&mut self) -> QueryResult<Option<usize>> {
        let start = Instant::now();
        let maybe_block_chunks = sqlx::query!(
            "SELECT chunks_left FROM pending_block
            LIMIT 1"
        )
        .fetch_optional(self.0.conn())
        .await?;
        metrics::histogram!("sql.chain.block.pending_block_chunks_left", start.elapsed());

        Ok(maybe_block_chunks.map(|val| val.chunks_left as usize))
    }

    /// Helper method for retrieving pending blocks from the database.
    async fn load_storage_pending_block(&mut self) -> QueryResult<Option<StoragePendingBlock>> {
        let start = Instant::now();
        let maybe_block = sqlx::query_as!(
            StoragePendingBlock,
            "SELECT * FROM pending_block
            ORDER BY number DESC
            LIMIT 1"
        )
        .fetch_optional(self.0.conn())
        .await?;
        metrics::histogram!(
            "sql.chain.block.load_storage_pending_block",
            start.elapsed()
        );

        Ok(maybe_block)
    }

    /// Retrieves the latest pending block from the database, if such is present.
    pub async fn load_pending_block(&mut self) -> QueryResult<Option<PendingBlock>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let pending_block_result = BlockSchema(&mut transaction)
            .load_storage_pending_block()
            .await?;

        let block = match pending_block_result {
            Some(block) => block,
            None => return Ok(None),
        };
        // Fill the block that's going to be returned with its operations.
        let executed_ops = BlockSchema(&mut transaction)
            .get_block_executed_ops(BlockNumber(block.number as u32))
            .await?;

        let mut success_operations = Vec::new();
        let mut failed_txs = Vec::new();
        for executed_op in executed_ops {
            match executed_op {
                ExecutedOperations::Tx(tx) if !tx.success => failed_txs.push(*tx),
                _ => success_operations.push(executed_op),
            }
        }

        let result = PendingBlock {
            number: BlockNumber(block.number as u32),
            chunks_left: block.chunks_left as usize,
            unprocessed_priority_op_before: block.unprocessed_priority_op_before as u64,
            pending_block_iteration: block.pending_block_iteration as usize,
            success_operations,
            failed_txs,
            timestamp: block.timestamp.unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("failed to get system time")
                    .as_secs() as i64
            }) as u64,
        };

        transaction.commit().await?;

        metrics::histogram!("sql.chain.block.load_pending_block", start.elapsed());
        Ok(Some(result))
    }

    /// Returns `true` if there is a stored pending block in the database.
    pub async fn pending_block_exists(&mut self) -> QueryResult<bool> {
        let start = Instant::now();
        let result = self.load_storage_pending_block().await?.is_some();

        metrics::histogram!("sql.chain.block.pending_block_exists", start.elapsed());
        Ok(result)
    }

    /// Stores given pending block into the database.
    pub async fn save_pending_block(&mut self, pending_block: PendingBlock) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let storage_block = StoragePendingBlock {
            number: (*pending_block.number).into(),
            chunks_left: pending_block.chunks_left as i64,
            unprocessed_priority_op_before: pending_block.unprocessed_priority_op_before as i64,
            pending_block_iteration: pending_block.pending_block_iteration as i64,
            previous_root_hash: Vec::new(), // Not used anywhere, left here for the backward compatibility.
            timestamp: Some(pending_block.timestamp as i64),
        };

        // Store the pending block header.
        sqlx::query!("
            INSERT INTO pending_block (number, chunks_left, unprocessed_priority_op_before, pending_block_iteration, timestamp)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (number)
            DO UPDATE
              SET chunks_left = $2, unprocessed_priority_op_before = $3, pending_block_iteration = $4, timestamp = $5
            ",
            storage_block.number, storage_block.chunks_left, storage_block.unprocessed_priority_op_before, storage_block.pending_block_iteration,
            storage_block.timestamp
        ).execute(transaction.conn())
        .await?;

        // Store the transactions from the block.
        let executed_transactions = pending_block
            .success_operations
            .into_iter()
            .chain(
                pending_block
                    .failed_txs
                    .into_iter()
                    .map(|tx| ExecutedOperations::Tx(Box::new(tx))),
            )
            .collect();
        BlockSchema(&mut transaction)
            .save_block_transactions(pending_block.number, executed_transactions)
            .await?;

        transaction.commit().await?;
        metrics::histogram!("sql.chain.block.save_pending_block", start.elapsed());

        Ok(())
    }

    /// Returns the number of rejected_txs in executed_txs
    pub async fn count_rejected_txs(&mut self) -> QueryResult<i64> {
        let start = Instant::now();
        let count = sqlx::query!(
            r#"SELECT count(*) as "count!" FROM executed_transactions WHERE success = false"#,
        )
        .fetch_one(self.0.conn())
        .await?
        .count;

        metrics::histogram!("sql.chain.block.count_rejected_txs", start.elapsed());
        Ok(count)
    }
    /// Returns the number of aggregated operations with the given `action_type` and `is_confirmed` status.
    pub async fn count_aggregated_operations(
        &mut self,
        aggregated_action_type: AggregatedActionType,
        is_confirmed: bool,
    ) -> QueryResult<i64> {
        let start = Instant::now();
        let count = sqlx::query!(
            r#"SELECT count(*) as "count!" FROM aggregate_operations WHERE action_type = $1 AND confirmed = $2"#,
            aggregated_action_type.to_string(),
            is_confirmed
        )
        .fetch_one(self.0.conn())
        .await?
        .count;

        metrics::histogram!("sql.chain.block.count_operations", start.elapsed());
        Ok(count)
    }

    /// Stores completed block into the database.
    ///
    /// This method assumes that `Block` was created from the corresponding `IncompleteBlock`
    /// object from the DB, and doesn't do any checks regarding that.
    pub async fn finish_incomplete_block(&mut self, block: Block) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let new_block = StorageBlock {
            number: i64::from(*block.block_number),
            root_hash: block.new_root_hash.to_bytes(),
            fee_account_id: i64::from(*block.fee_account),
            unprocessed_prior_op_before: block.processed_priority_ops.0 as i64,
            unprocessed_prior_op_after: block.processed_priority_ops.1 as i64,
            block_size: block.block_chunks_size as i64,
            commit_gas_limit: block.commit_gas_limit.as_u64() as i64,
            verify_gas_limit: block.verify_gas_limit.as_u64() as i64,
            commitment: block.block_commitment.as_bytes().to_vec(),
            timestamp: Some(block.timestamp as i64),
        };

        // Save new completed block.
        sqlx::query!("
            INSERT INTO blocks (number, root_hash, fee_account_id, unprocessed_prior_op_before, unprocessed_prior_op_after, block_size, commit_gas_limit, verify_gas_limit, commitment, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ",
            new_block.number, new_block.root_hash, new_block.fee_account_id, new_block.unprocessed_prior_op_before,
            new_block.unprocessed_prior_op_after, new_block.block_size, new_block.commit_gas_limit, new_block.verify_gas_limit,
            new_block.commitment, new_block.timestamp,
        ).execute(transaction.conn())
        .await?;

        // Remove incomplete block from which this block was created.
        sqlx::query!(
            "DELETE FROM incomplete_blocks WHERE number = $1",
            new_block.number
        )
        .execute(transaction.conn())
        .await?;

        transaction.commit().await?;

        metrics::histogram!("sql.chain.block.save_block", start.elapsed());
        Ok(())
    }

    /// Saves incomplete block to the database.
    ///
    /// This method **does not** save block transactions.
    /// They are expected to be saved prior, during processing of previous pending blocks.
    pub async fn save_incomplete_block(&mut self, block: &IncompleteBlock) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let number = i64::from(*block.block_number);
        let fee_account_id = i64::from(*block.fee_account);
        let unprocessed_prior_op_before = block.processed_priority_ops.0 as i64;
        let unprocessed_prior_op_after = block.processed_priority_ops.1 as i64;
        let block_size = block.block_chunks_size as i64;
        let commit_gas_limit = block.commit_gas_limit.as_u64() as i64;
        let verify_gas_limit = block.verify_gas_limit.as_u64() as i64;
        let timestamp = Some(block.timestamp as i64);

        let new_block = StorageIncompleteBlock {
            number,
            fee_account_id,
            unprocessed_prior_op_before,
            unprocessed_prior_op_after,
            block_size,
            commit_gas_limit,
            verify_gas_limit,
            timestamp,
        };

        // Remove pending block, as it's now sealed.
        // Note that the block is NOT completed yet: root hash calculation should still happen.
        sqlx::query!(
            "
            DELETE FROM pending_block WHERE number = $1
            ",
            new_block.number
        )
        .execute(transaction.conn())
        .await?;

        // Save this block.
        sqlx::query!("
            INSERT INTO incomplete_blocks (number, fee_account_id, unprocessed_prior_op_before, unprocessed_prior_op_after, block_size, commit_gas_limit, verify_gas_limit,  timestamp)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ",
            new_block.number, new_block.fee_account_id, new_block.unprocessed_prior_op_before,
            new_block.unprocessed_prior_op_after, new_block.block_size, new_block.commit_gas_limit, new_block.verify_gas_limit,
            new_block.timestamp,
        ).execute(transaction.conn())
        .await?;

        transaction.commit().await?;

        metrics::histogram!("sql.chain.block.save_incomplete_block", start.elapsed());
        Ok(())
    }

    /// This method saves block transactions, and then does both `save_incomplete_block`
    /// and `save_block` for a `Block` object.
    ///
    /// It is an alternative for calling two mentioned methods separately that has several use cases:
    /// - In some contexts, root hash for the block is known immediately (e.g. data restore).
    /// - In most DB/API tests, the process of block sealing doesn't really matter: these tests check the behavior
    ///   of blocks that are already stored in the DB, not *how* they are stored.
    pub async fn save_full_block(&mut self, block: Block) -> anyhow::Result<()> {
        let full_block = block.clone();
        let incomplete_block = IncompleteBlock::new(
            block.block_number,
            block.fee_account,
            block.block_transactions,
            block.processed_priority_ops,
            block.block_chunks_size,
            block.commit_gas_limit,
            block.verify_gas_limit,
            block.timestamp,
        );
        let mut transaction = self.0.start_transaction().await?;

        BlockSchema(&mut transaction)
            .save_block_transactions(
                full_block.block_number,
                full_block.block_transactions.clone(),
            )
            .await?;
        BlockSchema(&mut transaction)
            .save_incomplete_block(&incomplete_block)
            .await?;
        BlockSchema(&mut transaction)
            .finish_incomplete_block(full_block)
            .await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Returns the ID of the next expected priority operation.
    /// Performs a lookup in both incomplete and complete block tables.
    pub async fn next_expected_serial_id(&mut self) -> QueryResult<u64> {
        let start = Instant::now();

        let next_expected_serial_id = sqlx::query!(
            "SELECT GREATEST(
                (SELECT MAX(unprocessed_prior_op_after) FROM incomplete_blocks),
                (SELECT MAX(unprocessed_prior_op_after) FROM blocks)
            )",
        )
        .fetch_one(self.0.conn())
        .await?
        .greatest
        .map(|val| val as u64)
        .unwrap_or_default();

        metrics::histogram!("sql.chain.block.next_expected_serial_id", start.elapsed());
        Ok(next_expected_serial_id)
    }

    /// Returns the range of existing incomplete blocks.
    ///
    /// Returned range is *inclusive*, meaning that both returned blocks (if they were returned)
    /// exist in the database, and represent minimum and maximum existing blocks correspondingly.
    pub async fn incomplete_blocks_range(
        &mut self,
    ) -> QueryResult<Option<(BlockNumber, BlockNumber)>> {
        let start = Instant::now();

        let raw_numbers = sqlx::query!(
            "
                SELECT min(number), max(number)
                FROM incomplete_blocks
            ",
        )
        .fetch_one(self.0.conn())
        .await?;

        let block_numbers = match (raw_numbers.min, raw_numbers.max) {
            (Some(min), Some(max)) => Some((BlockNumber(min as u32), BlockNumber(max as u32))),
            (None, None) => None,
            _ => {
                panic!("Inconsistent results for min/max query: {:?}", raw_numbers);
            }
        };

        metrics::histogram!("sql.chain.block.incomplete_blocks_range", start.elapsed());
        Ok(block_numbers)
    }

    // Helper method for retrieving incomplete blocks from the database.
    async fn get_storage_incomplete_block(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Option<StorageIncompleteBlock>> {
        let start = Instant::now();
        let block = sqlx::query_as!(
            StorageIncompleteBlock,
            "SELECT * FROM incomplete_blocks WHERE number = $1",
            i64::from(*block)
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.block.get_storage_incomplete_block",
            start.elapsed()
        );

        Ok(block)
    }

    /// Given the block number, attempts to retrieve data to complete it from the database.
    /// Returns `None` if the block with provided number does not exist yet.
    ///
    /// Data to complete consists of `IncompleteBlock` object and the root hash of the previous block.
    pub async fn get_data_to_complete_block(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<(Option<IncompleteBlock>, Option<Fr>)> {
        let start = Instant::now();
        // Load block header.
        let stored_block =
            if let Some(block) = self.get_storage_incomplete_block(block_number).await? {
                block
            } else {
                return Ok((None, None));
            };

        // Load transactions for this block.
        let block_transactions = self.get_block_executed_ops(block_number).await?;

        // Return the obtained block in the expected format.
        let block = Some(IncompleteBlock::new(
            block_number,
            AccountId(stored_block.fee_account_id as u32),
            block_transactions,
            (
                stored_block.unprocessed_prior_op_before as u64,
                stored_block.unprocessed_prior_op_after as u64,
            ),
            stored_block.block_size as usize,
            U256::from(stored_block.commit_gas_limit as u64),
            U256::from(stored_block.verify_gas_limit as u64),
            stored_block.timestamp.unwrap_or_default() as u64,
        ));

        // Load previous block root hash.
        let prev_block_number = block_number - 1;
        let previous_root_hash = sqlx::query_as!(
            StorageRootHash,
            "SELECT root_hash FROM blocks WHERE number = $1",
            i64::from(*prev_block_number)
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|entry| FeConvert::from_bytes(&entry.root_hash).expect("Unparsable root hash"));

        metrics::histogram!(
            "sql.chain.block.get_data_to_complete_block",
            start.elapsed()
        );

        Ok((block, previous_root_hash))
    }

    pub async fn save_block_metadata(
        &mut self,
        block_number: BlockNumber,
        block_metadata: BlockMetadata,
    ) -> QueryResult<()> {
        let start = Instant::now();

        sqlx::query!(
            "
            INSERT INTO block_metadata (block_number, fast_processing)
            VALUES ($1, $2)
            ",
            i64::from(*block_number),
            block_metadata.fast_processing
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.block.save_block_metadata", start.elapsed());
        Ok(())
    }

    pub async fn save_genesis_block(&mut self, root_hash: Fr) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;

        let full_block = Block {
            block_number: BlockNumber(0),
            fee_account: AccountId(0),
            block_transactions: Vec::new(),
            processed_priority_ops: (0, 0),
            block_chunks_size: 0,
            commit_gas_limit: 0u32.into(),
            verify_gas_limit: 0u32.into(),
            timestamp: 0,
            new_root_hash: root_hash,
            block_commitment: H256::zero(),
        };

        BlockSchema(&mut transaction)
            .save_full_block(full_block)
            .await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Retrieves both L1 and L2 operations stored in the block for the given pagination query
    pub async fn get_block_transactions_page(
        &mut self,
        query: &PaginationQuery<BlockAndTxHash>,
    ) -> QueryResult<Option<Vec<Transaction>>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let tx_hash = match query.from.tx_hash.inner {
            Either::Left(tx_hash) => tx_hash,
            Either::Right(_) => {
                if let Some(tx_hash) = transaction
                    .chain()
                    .operations_ext_schema()
                    .get_block_last_tx_hash(query.from.block_number)
                    .await?
                {
                    tx_hash
                } else {
                    return Ok(Some(Vec::new()));
                }
            }
        };
        let sequence_number = transaction
            .chain()
            .operations_ext_schema()
            .get_tx_sequence_number_for_block(tx_hash, query.from.block_number)
            .await?;
        let block_txs = if let Some(sequence_number) = sequence_number {
            let raw_txs: Vec<TransactionItem> = match query.direction {
                PaginationDirection::Newer => {
                    sqlx::query_as!(
                        TransactionItem,
                        r#"
                                WITH transactions AS (
                                    SELECT
                                        sequence_number,
                                        tx_hash,
                                        tx as op,
                                        block_number,
                                        created_at,
                                        success,
                                        fail_reason,
                                        Null::bytea as eth_hash,
                                        Null::bigint as priority_op_serialid,
                                        block_index,
                                        batch_id
                                    FROM executed_transactions
                                    WHERE block_number = $1 AND sequence_number >= $2
                                ), priority_ops AS (
                                    SELECT
                                        sequence_number,
                                        tx_hash,
                                        operation as op,
                                        block_number,
                                        created_at,
                                        true as success,
                                        Null as fail_reason,
                                        eth_hash,
                                        priority_op_serialid,
                                        block_index,
                                        Null::bigint as batch_id
                                    FROM executed_priority_operations
                                    WHERE block_number = $1 AND sequence_number >= $2
                                ), everything AS (
                                    SELECT * FROM transactions
                                    UNION ALL
                                    SELECT * FROM priority_ops
                                )
                                SELECT
                                    sequence_number,
                                    tx_hash as "tx_hash!",
                                    block_number as "block_number!",
                                    block_index as "block_index?",
                                    op as "op!",
                                    created_at as "created_at!",
                                    success as "success!",
                                    fail_reason as "fail_reason?",
                                    eth_hash as "eth_hash?",
                                    priority_op_serialid as "priority_op_serialid?",
                                    batch_id as "batch_id?"
                                FROM everything
                                ORDER BY sequence_number ASC
                                LIMIT $3
                            "#,
                        i64::from(*query.from.block_number),
                        sequence_number,
                        i64::from(query.limit),
                    )
                    .fetch_all(transaction.conn())
                    .await?
                }
                PaginationDirection::Older => {
                    sqlx::query_as!(
                        TransactionItem,
                        r#"
                                WITH transactions AS (
                                    SELECT
                                        sequence_number,
                                        tx_hash,
                                        tx as op,
                                        block_number,
                                        created_at,
                                        success,
                                        fail_reason,
                                        Null::bytea as eth_hash,
                                        Null::bigint as priority_op_serialid,
                                        block_index,
                                        batch_id
                                    FROM executed_transactions
                                    WHERE block_number = $1 AND sequence_number <= $2
                                ), priority_ops AS (
                                    SELECT
                                        sequence_number,
                                        tx_hash,
                                        operation as op,
                                        block_number,
                                        created_at,
                                        true as success,
                                        Null as fail_reason,
                                        eth_hash,
                                        priority_op_serialid,
                                        block_index,
                                        Null::bigint as batch_id
                                    FROM executed_priority_operations
                                    WHERE block_number = $1 AND sequence_number <= $2
                                ), everything AS (
                                    SELECT * FROM transactions
                                    UNION ALL
                                    SELECT * FROM priority_ops
                                )
                                SELECT
                                    sequence_number,
                                    tx_hash as "tx_hash!",
                                    block_number as "block_number!",
                                    block_index as "block_index?",
                                    op as "op!",
                                    created_at as "created_at!",
                                    success as "success!",
                                    fail_reason as "fail_reason?",
                                    eth_hash as "eth_hash?",
                                    priority_op_serialid as "priority_op_serialid?",
                                    batch_id as "batch_id?"
                                FROM everything
                                ORDER BY sequence_number DESC 
                                LIMIT $3
                            "#,
                        i64::from(*query.from.block_number),
                        sequence_number,
                        i64::from(query.limit),
                    )
                    .fetch_all(transaction.conn())
                    .await?
                }
            };
            let is_block_finalized = transaction
                .chain()
                .block_schema()
                .is_block_finalized(query.from.block_number)
                .await?;
            let txs: Vec<Transaction> = raw_txs
                .into_iter()
                .map(|tx| TransactionItem::transaction_from_item(tx, is_block_finalized))
                .collect();
            Some(txs)
        } else {
            None
        };
        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.block.get_block_transactions_page",
            start.elapsed()
        );
        Ok(block_txs)
    }

    /// Returns count of both L1 and L2 operations stored in the block
    pub async fn get_block_transactions_count(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<u32> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let tx_count = sqlx::query!(
            r#"SELECT count(*) as "count!" FROM executed_transactions WHERE block_number = $1"#,
            i64::from(*block_number)
        )
        .fetch_one(transaction.conn())
        .await?
        .count;
        let priority_op_count = sqlx::query!(
            r#"SELECT count(*) as "count!" FROM executed_priority_operations WHERE block_number = $1"#,
            i64::from(*block_number)
        )
        .fetch_one(transaction.conn())
        .await?
        .count;
        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.block.get_block_transactions_count",
            start.elapsed()
        );
        Ok((tx_count + priority_op_count) as u32)
    }

    // Removes blocks with number greater than `last_block`
    pub async fn remove_blocks(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        // We should retrieve last committed block for which an aggregated operation was created.
        // Events for blocks are created by `eth_sender` after sending transaction to L1, so until then there is
        // no event and no need to create a `revert` one.
        // If aggregated operation was created, but was not sent to L1, that's also not a problem: event schema
        // ignores events for non-exist
        //
        // TODO (ZKS-856): `get_last_committed_block` returns the last block for which *an aggregated operation*
        // is created. Currently, it seems that events are created by eth sender after confirming an L1 transaction.
        // We need to check that if aggregated operation was created, but not sent or not confirmed, this will not
        // result in extra events being created.
        // Please, update the comment above after resolving this TODO to match the new logic.
        let last_committed_block = transaction
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await?;

        for block_number in *last_block..=*last_committed_block {
            transaction
                .event_schema()
                .store_block_event(BlockNumber(block_number), BlockStatus::Reverted)
                .await?;
        }

        sqlx::query!(
            "DELETE FROM incomplete_blocks WHERE number > $1",
            *last_block as i64
        )
        .execute(transaction.conn())
        .await?;

        sqlx::query!("DELETE FROM blocks WHERE number > $1", *last_block as i64)
            .execute(transaction.conn())
            .await?;

        sqlx::query!(
            "DELETE FROM block_metadata WHERE block_number > $1",
            *last_block as i64
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;
        metrics::histogram!("sql.chain.block.remove_blocks", start.elapsed());
        Ok(())
    }

    // Removes pending block
    pub async fn remove_pending_block(&mut self) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!("DELETE FROM pending_block")
            .execute(self.0.conn())
            .await?;

        metrics::histogram!("sql.chain.block.remove_pending_block", start.elapsed());
        Ok(())
    }

    pub async fn store_factories_for_block_withdraw_nfts(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let executed_txs: Vec<StoredExecutedTransaction> = sqlx::query_as!(
            StoredExecutedTransaction,
            "SELECT * FROM executed_transactions WHERE block_number BETWEEN $1 AND $2 AND success = true",
            i64::from(*from_block),
            i64::from(*to_block)
        )
        .fetch_all(transaction.conn())
        .await?;

        let mut token_ids = Vec::new();
        for executed_tx in executed_txs {
            if executed_tx.tx.get("type")
                == Some(&serde_json::Value::String("WithdrawNFT".to_string()))
            {
                token_ids.push(executed_tx.tx.get("token").unwrap().as_i64().unwrap() as i32);
            }
        }

        sqlx::query!(
            r#"
                INSERT INTO withdrawn_nfts_factories (token_id, factory_address)
                SELECT token_id, 
                    COALESCE(nft_factory.factory_address, server_config.nft_factory_addr) as factory_address
                FROM nft
                INNER JOIN server_config ON server_config.id = true
                LEFT JOIN nft_factory ON nft_factory.creator_id = nft.creator_account_id
                WHERE nft.token_id = ANY($1)
            "#,
            &token_ids
        )
        .execute(transaction.conn())
        .await?;
        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.block.store_factories_for_block_withdraw_nfts",
            start.elapsed()
        );
        Ok(())
    }

    pub async fn get_block_number_by_hash(
        &mut self,
        hash: &[u8],
    ) -> QueryResult<Option<BlockNumber>> {
        let start = Instant::now();
        let record = sqlx::query!("SELECT number FROM blocks where root_hash = $1", hash)
            .fetch_optional(self.0.conn())
            .await?;
        let block_number = record.map(|r| BlockNumber(r.number as u32));

        metrics::histogram!("sql.chain.block.get_block_number_by_hash", start.elapsed());
        Ok(block_number)
    }

    pub async fn get_block_transactions_hashes(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<Vec<Vec<u8>>> {
        let start = Instant::now();
        let records = sqlx::query!(
            r#"
                WITH transactions AS (
                    SELECT tx_hash, sequence_number
                    FROM executed_transactions
                    WHERE block_number = $1
                ), priority_ops AS (
                    SELECT tx_hash, sequence_number
                    FROM executed_priority_operations
                    WHERE block_number = $1
                ), everything AS (
                    SELECT * FROM transactions
                    UNION ALL
                    SELECT * FROM priority_ops
                )
                SELECT tx_hash as "tx_hash!"
                FROM everything
                ORDER BY sequence_number
            "#,
            i64::from(*block_number)
        )
        .fetch_all(self.0.conn())
        .await?;
        let hashes = records.into_iter().map(|record| record.tx_hash).collect();

        metrics::histogram!(
            "sql.chain.block.get_block_transactions_hashes",
            start.elapsed()
        );
        Ok(hashes)
    }
}
