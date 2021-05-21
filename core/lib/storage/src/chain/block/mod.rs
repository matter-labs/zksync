// Built-in deps
use std::time::{Instant, SystemTime, UNIX_EPOCH};
// External imports
// Workspace imports
use zksync_basic_types::{H256, U256};
use zksync_crypto::convert::FeConvert;
use zksync_types::{
    aggregated_operations::AggregatedActionType,
    block::{Block, BlockMetadata, ExecutedOperations, PendingBlock},
    event::block::BlockStatus,
    AccountId, BlockNumber, Fr, ZkSyncOp,
};
// Local imports
use self::records::{
    AccountTreeCache, BlockTransactionItem, StorageBlock, StorageBlockDetails,
    StorageBlockMetadata, StoragePendingBlock,
};
use crate::{
    chain::account::records::EthAccountType,
    chain::operations::{
        records::{
            NewExecutedPriorityOperation, NewExecutedTransaction, StoredExecutedPriorityOperation,
            StoredExecutedTransaction,
        },
        OperationsSchema,
    },
    QueryResult, StorageProcessor,
};

mod conversion;
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
                        transaction
                            .chain()
                            .account_schema()
                            .set_account_type(tx.account_id, account_type)
                            .await?;
                    }
                    // Store the executed operation in the corresponding schema.
                    let new_tx = NewExecutedTransaction::prepare_stored_tx(*tx, block_number);
                    transaction
                        .chain()
                        .operations_schema()
                        .store_executed_tx(new_tx)
                        .await?;
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    // For priority operation we should only store it in the Operations schema.
                    let new_priority_op = NewExecutedPriorityOperation::prepare_stored_priority_op(
                        *prior_op,
                        block_number,
                    );
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
    async fn get_storage_block(&mut self, block: BlockNumber) -> QueryResult<Option<StorageBlock>> {
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
                        success,
                        fail_reason,
                        created_at
                    FROM executed_transactions
                    WHERE block_number = $1
                ), priority_ops AS (
                    SELECT
                        '0x' || encode(eth_hash, 'hex') as tx_hash,
                        operation as op,
                        block_number,
                        true as success,
                        Null as fail_reason,
                        created_at
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
                    success as "success?",
                    fail_reason as "fail_reason?",
                    created_at as "created_at!"
                FROM everything
                ORDER BY created_at DESC
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
            .filter_map(|stored_exec| stored_exec.into_executed_tx().ok())
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

    /// Loads the block headers for the given amount of blocks.
    pub async fn load_block_range(
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
            )
            ,aggr_exec as (
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

    /// Helper method for `find_block_by_height_or_hash`. It checks whether
    /// provided string can be interpreted like a hash, and if so, returns the
    /// hexadecimal string without prefix.
    fn try_parse_hex(&self, query: &str) -> Option<String> {
        const HASH_STRING_SIZE: usize = 32 * 2; // 32 bytes, 2 symbols per byte.

        if let Some(query) = query.strip_prefix("0x") {
            Some(query.into())
        } else if let Some(query) = query.strip_prefix("sync-bl:") {
            Some(query.into())
        } else if query.len() == HASH_STRING_SIZE && hex::decode(query).is_ok() {
            Some(query.into())
        } else {
            None
        }
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
        let hash_bytes = if let Some(hex_query) = self.try_parse_hex(&query) {
            // It must be a hexadecimal value, so unwrap is safe.
            hex::decode(hex_query).unwrap()
        } else {
            // Not a hash, provide an empty vector.
            vec![]
        };

        // If the input can be interpreted as integer, add the block number lookup part.
        let block_number = if let Ok(int_query) = query.parse::<i64>() {
            // let block_lookup = format!("or blocks.number = {}", int_query);
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
            )
            ,aggr_exec as (
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

        let previous_block_root_hash = H256::from_slice(&block.previous_root_hash);

        let result = PendingBlock {
            number: BlockNumber(block.number as u32),
            chunks_left: block.chunks_left as usize,
            unprocessed_priority_op_before: block.unprocessed_priority_op_before as u64,
            pending_block_iteration: block.pending_block_iteration as usize,
            success_operations,
            failed_txs,
            previous_block_root_hash,
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
            previous_root_hash: pending_block.previous_block_root_hash.as_bytes().to_vec(),
            timestamp: Some(pending_block.timestamp as i64),
        };

        // Store the pending block header.
        sqlx::query!("
            INSERT INTO pending_block (number, chunks_left, unprocessed_priority_op_before, pending_block_iteration, previous_root_hash, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (number)
            DO UPDATE
              SET chunks_left = $2, unprocessed_priority_op_before = $3, pending_block_iteration = $4, previous_root_hash = $5, timestamp = $6
            ",
            storage_block.number, storage_block.chunks_left, storage_block.unprocessed_priority_op_before, storage_block.pending_block_iteration, storage_block.previous_root_hash,
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
        metrics::histogram!("sql.chain.block.load_pending_block", start.elapsed());

        Ok(())
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

    pub async fn save_block(&mut self, block: Block) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let number = i64::from(*block.block_number);
        let root_hash = block.new_root_hash.to_bytes();
        let fee_account_id = i64::from(*block.fee_account);
        let unprocessed_prior_op_before = block.processed_priority_ops.0 as i64;
        let unprocessed_prior_op_after = block.processed_priority_ops.1 as i64;
        let block_size = block.block_chunks_size as i64;
        let commit_gas_limit = block.commit_gas_limit.as_u64() as i64;
        let verify_gas_limit = block.verify_gas_limit.as_u64() as i64;
        let commitment = block.block_commitment.as_bytes().to_vec();
        let timestamp = Some(block.timestamp as i64);

        BlockSchema(&mut transaction)
            .save_block_transactions(block.block_number, block.block_transactions)
            .await?;

        // Notify about rejected transactions right away without waiting for the block commit.
        transaction
            .event_schema()
            .store_rejected_transaction_event(block.block_number)
            .await?;

        let new_block = StorageBlock {
            number,
            root_hash,
            fee_account_id,
            unprocessed_prior_op_before,
            unprocessed_prior_op_after,
            block_size,
            commit_gas_limit,
            verify_gas_limit,
            commitment,
            timestamp,
        };

        // Remove pending block (as it's now completed).
        sqlx::query!(
            "
            DELETE FROM pending_block WHERE number = $1
            ",
            new_block.number
        )
        .execute(transaction.conn())
        .await?;

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

        transaction.commit().await?;

        metrics::histogram!("sql.chain.block.save_block", start.elapsed());
        Ok(())
    }

    // This method does not have metrics, since it is used only for the
    // migration for the nft regenesis.
    // Remove this function once the regenesis is complete and the tool is not
    // needed anymore: ZKS-663
    pub async fn change_block_root_hash(
        &mut self,
        block_number: BlockNumber,
        new_root_hash: Fr,
    ) -> QueryResult<()> {
        let root_hash_bytes = new_root_hash.to_bytes();
        sqlx::query!(
            "UPDATE blocks
                SET root_hash = $1
                WHERE number = $2",
            root_hash_bytes,
            *block_number as i64
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
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

    /// Stores account tree cache for a block
    pub async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        tree_cache: serde_json::Value,
    ) -> QueryResult<()> {
        let start = Instant::now();
        if *block == 0 {
            return Ok(());
        }

        let tree_cache_str =
            serde_json::to_string(&tree_cache).expect("Failed to serialize Account Tree Cache");
        sqlx::query!(
            "
            INSERT INTO account_tree_cache (block, tree_cache)
            VALUES ($1, $2)
            ",
            *block as i64,
            tree_cache_str,
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.block.store_account_tree_cache", start.elapsed());
        Ok(())
    }

    // This method does not have metrics, since it is used only for the
    // migration for the nft regenesis.
    // Remove this function once the regenesis is complete and the tool is not
    // needed anymore: ZKS-663
    pub async fn reset_account_tree_cache(&mut self, block_number: BlockNumber) -> QueryResult<()> {
        sqlx::query!(
            "
            DELETE FROM account_tree_cache 
            WHERE block = $1
            ",
            *block_number as u32
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    /// Gets stored account tree cache for a block
    pub async fn get_account_tree_cache(
        &mut self,
    ) -> QueryResult<Option<(BlockNumber, serde_json::Value)>> {
        let start = Instant::now();
        let account_tree_cache = sqlx::query_as!(
            AccountTreeCache,
            "
            SELECT * FROM account_tree_cache
            ORDER BY block DESC
            LIMIT 1
            ",
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.block.get_account_tree_cache", start.elapsed());
        Ok(account_tree_cache.map(|w| {
            (
                BlockNumber(w.block as u32),
                serde_json::from_str(&w.tree_cache)
                    .expect("Failed to deserialize Account Tree Cache"),
            )
        }))
    }

    /// Gets stored account tree cache for a block
    pub async fn get_account_tree_cache_block(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Option<serde_json::Value>> {
        let start = Instant::now();
        let account_tree_cache = sqlx::query_as!(
            AccountTreeCache,
            "
            SELECT * FROM account_tree_cache
            WHERE block = $1
            ",
            *block as i64
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.block.get_account_tree_cache_block",
            start.elapsed()
        );
        Ok(account_tree_cache.map(|w| {
            serde_json::from_str(&w.tree_cache).expect("Failed to deserialize Account Tree Cache")
        }))
    }

    pub async fn save_genesis_block(&mut self, root_hash: Fr) -> QueryResult<()> {
        let block = Block {
            block_number: BlockNumber(0),
            new_root_hash: root_hash,
            fee_account: AccountId(0),
            block_transactions: Vec::new(),
            processed_priority_ops: (0, 0),
            block_chunks_size: 0,
            commit_gas_limit: 0u32.into(),
            verify_gas_limit: 0u32.into(),
            block_commitment: H256::zero(),
            timestamp: 0,
        };

        Ok(self.save_block(block).await?)
    }

    // Removes blocks with number greater than `last_block`
    pub async fn remove_blocks(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

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

        sqlx::query!("DELETE FROM blocks WHERE number > $1", *last_block as i64)
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

    // Removes account tree cache for blocks with number greater than `last_block`
    pub async fn remove_account_tree_cache(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM account_tree_cache WHERE block > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.block.remove_account_tree_cache", start.elapsed());
        Ok(())
    }
}
