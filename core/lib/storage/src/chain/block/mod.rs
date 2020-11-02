// Built-in deps
// External imports
use zksync_basic_types::U256;
// Workspace imports
use zksync_crypto::convert::FeConvert;
use zksync_types::{block::PendingBlock, Action, ActionType, Operation};
use zksync_types::{
    block::{Block, ExecutedOperations},
    AccountId, BlockNumber, ZkSyncOp,
};
// Local imports
use self::records::{
    AccountTreeCache, BlockDetails, BlockTransactionItem, StorageBlock, StoragePendingBlock,
};
use crate::{
    chain::operations::{
        records::{
            NewExecutedPriorityOperation, NewExecutedTransaction, NewOperation,
            StoredExecutedPriorityOperation, StoredExecutedTransaction, StoredOperation,
        },
        OperationsSchema,
    },
    prover::ProverSchema,
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
    /// Executes an operation:
    /// 1. Stores the operation.
    /// 2. Stores the proof (if it isn't stored already) for the verify operation.
    pub async fn execute_operation(&mut self, op: Operation) -> QueryResult<Operation> {
        let mut transaction = self.0.start_transaction().await?;

        let block_number = op.block.block_number;

        match &op.action {
            Action::Commit => {
                BlockSchema(&mut transaction).save_block(op.block).await?;
            }
            Action::Verify { proof } => {
                let stored_proof = ProverSchema(&mut transaction)
                    .load_proof(block_number)
                    .await?;
                match stored_proof {
                    None => {
                        ProverSchema(&mut transaction)
                            .store_proof(block_number, proof)
                            .await?;
                    }
                    Some(_) => {}
                };
            }
        };

        let new_operation = NewOperation {
            block_number: i64::from(block_number),
            action_type: op.action.to_string(),
        };
        let stored: StoredOperation = OperationsSchema(&mut transaction)
            .store_operation(new_operation)
            .await?;
        let result = stored.into_op(&mut transaction).await;

        transaction.commit().await?;
        result
    }

    /// Given a block, stores its transactions in the database.
    pub async fn save_block_transactions(
        &mut self,
        block_number: u32,
        operations: Vec<ExecutedOperations>,
    ) -> QueryResult<()> {
        for block_tx in operations.into_iter() {
            match block_tx {
                ExecutedOperations::Tx(tx) => {
                    // Store the executed operation in the corresponding schema.
                    let new_tx = NewExecutedTransaction::prepare_stored_tx(*tx, block_number);
                    OperationsSchema(self.0).store_executed_tx(new_tx).await?;
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    // For priority operation we should only store it in the Operations schema.
                    let new_priority_op = NewExecutedPriorityOperation::prepare_stored_priority_op(
                        *prior_op,
                        block_number,
                    );
                    OperationsSchema(self.0)
                        .store_executed_priority_op(new_priority_op)
                        .await?;
                }
            }
        }
        Ok(())
    }

    async fn get_storage_block(&mut self, block: BlockNumber) -> QueryResult<Option<StorageBlock>> {
        let block = sqlx::query_as!(
            StorageBlock,
            "SELECT * FROM blocks WHERE number = $1",
            i64::from(block)
        )
        .fetch_optional(self.0.conn())
        .await?;

        Ok(block)
    }

    /// Given the block number, attempts to retrieve it from the database.
    /// Returns `None` if the block with provided number does not exist yet.
    pub async fn get_block(&mut self, block: BlockNumber) -> QueryResult<Option<Block>> {
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

        // Return the obtained block in the expected format.
        Ok(Some(Block::new(
            block,
            new_root_hash,
            stored_block.fee_account_id as AccountId,
            block_transactions,
            (
                stored_block.unprocessed_prior_op_before as u64,
                stored_block.unprocessed_prior_op_after as u64,
            ),
            stored_block.block_size as usize,
            U256::from(stored_block.commit_gas_limit as u64),
            U256::from(stored_block.verify_gas_limit as u64),
        )))
    }

    /// Same as `get_block_executed_ops`, but returns a vector of `ZkSyncOp` instead
    /// of `ExecutedOperations`.
    pub async fn get_block_operations(&mut self, block: BlockNumber) -> QueryResult<Vec<ZkSyncOp>> {
        let executed_ops = self.get_block_executed_ops(block).await?;
        Ok(executed_ops
            .into_iter()
            .filter_map(|exec_op| match exec_op {
                ExecutedOperations::Tx(tx) => tx.op,
                ExecutedOperations::PriorityOp(priorop) => Some(priorop.op),
            })
            .collect())
    }

    pub async fn get_block_transactions(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Vec<BlockTransactionItem>> {
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
            i64::from(block)
        )
        .fetch_all(self.0.conn())
        .await?;

        Ok(block_txs)
    }

    /// Given the block number, loads all the operations that were executed in that block.
    pub async fn get_block_executed_ops(
        &mut self,
        block: BlockNumber,
    ) -> QueryResult<Vec<ExecutedOperations>> {
        let mut executed_operations = Vec::new();

        // Load both executed transactions and executed priority operations
        // from the database.
        let (executed_ops, executed_priority_ops) = {
            let executed_ops = sqlx::query_as!(
                StoredExecutedTransaction,
                "SELECT * FROM executed_transactions WHERE block_number = $1",
                i64::from(block)
            )
            .fetch_all(self.0.conn())
            .await?;

            let executed_priority_ops = sqlx::query_as!(
                StoredExecutedPriorityOperation,
                "SELECT * FROM executed_priority_operations WHERE block_number = $1",
                i64::from(block)
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
                        u32::max_value()
                    }
                }
                ExecutedOperations::PriorityOp(op) => op.block_index,
            }
        });

        Ok(executed_operations)
    }

    /// Loads the block headers for the given amount of blocks.
    pub async fn load_block_range(
        &mut self,
        max_block: BlockNumber,
        limit: u32,
    ) -> QueryResult<Vec<BlockDetails>> {
        // This query does the following:
        // - joins the `operations` and `eth_tx_hashes` (using the intermediate `eth_ops_binding` table)
        //   tables to collect the data:
        //   block number, ethereum transaction hash, action type and action creation timestamp;
        // - joins the `blocks` table with result of the join twice: once for committed operations
        //   and verified operations;
        // - collects the {limit} blocks in the descending order with the data gathered above.
        let details = sqlx::query_as!(
            BlockDetails,
            r#"
            WITH eth_ops AS (
                SELECT DISTINCT ON (block_number, action_type)
                    operations.block_number,
                    eth_tx_hashes.tx_hash,
                    operations.action_type,
                    operations.created_at,
                    confirmed
                FROM operations
                    left join eth_ops_binding on eth_ops_binding.op_id = operations.id
                    left join eth_tx_hashes on eth_tx_hashes.eth_op_id = eth_ops_binding.eth_op_id
                ORDER BY block_number DESC, action_type, confirmed
            )
            SELECT
                blocks.number AS "block_number!",
                blocks.root_hash AS "new_state_root!",
                blocks.block_size AS "block_size!",
                committed.tx_hash AS "commit_tx_hash?",
                verified.tx_hash AS "verify_tx_hash?",
                committed.created_at AS "committed_at!",
                verified.created_at AS "verified_at?"
            FROM blocks
            INNER JOIN eth_ops committed ON
                committed.block_number = blocks.number AND committed.action_type = 'COMMIT' AND committed.confirmed = true
            LEFT JOIN eth_ops verified ON
                verified.block_number = blocks.number AND verified.action_type = 'VERIFY' AND verified.confirmed = true
            WHERE
                blocks.number <= $1
            ORDER BY blocks.number DESC
            LIMIT $2;
            "#,
            i64::from(max_block),
            i64::from(limit)
        ).fetch_all(self.0.conn())
        .await?;
        Ok(details)
    }

    /// Helper method for `find_block_by_height_or_hash`. It checks whether
    /// provided string can be interpreted like a hash, and if so, returns the
    /// hexadecimal string without prefix.
    fn try_parse_hex(&self, query: &str) -> Option<String> {
        const HASH_STRING_SIZE: usize = 32 * 2; // 32 bytes, 2 symbols per byte.

        if query.starts_with("0x") {
            Some(query[2..].into())
        } else if query.starts_with("sync-bl:") {
            Some(query[8..].into())
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
    pub async fn find_block_by_height_or_hash(&mut self, query: String) -> Option<BlockDetails> {
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
        sqlx::query_as!(
            BlockDetails,
            r#"
            WITH eth_ops AS (
                SELECT DISTINCT ON (block_number, action_type)
                    operations.block_number,
                    eth_tx_hashes.tx_hash,
                    operations.action_type,
                    operations.created_at,
                    confirmed
                FROM operations
                    left join eth_ops_binding on eth_ops_binding.op_id = operations.id
                    left join eth_tx_hashes on eth_tx_hashes.eth_op_id = eth_ops_binding.eth_op_id
                ORDER BY block_number desc, action_type, confirmed
            )
            SELECT
                blocks.number AS "block_number!",
                blocks.root_hash AS "new_state_root!",
                blocks.block_size AS "block_size!",
                committed.tx_hash AS "commit_tx_hash?",
                verified.tx_hash AS "verify_tx_hash?",
                committed.created_at AS "committed_at!",
                verified.created_at AS "verified_at?"
            FROM blocks
            INNER JOIN eth_ops committed ON
                committed.block_number = blocks.number AND committed.action_type = 'COMMIT' AND committed.confirmed = true
            LEFT JOIN eth_ops verified ON
                verified.block_number = blocks.number AND verified.action_type = 'VERIFY' AND verified.confirmed = true
            WHERE false
                OR committed.tx_hash = $1
                OR verified.tx_hash = $1
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
            .flatten()
    }

    pub async fn load_commit_op(&mut self, block_number: BlockNumber) -> Option<Operation> {
        let op = OperationsSchema(self.0)
            .get_operation(block_number, ActionType::COMMIT)
            .await;
        if let Some(stored_op) = op {
            stored_op.into_op(self.0).await.ok()
        } else {
            None
        }
    }

    pub async fn load_committed_block(&mut self, block_number: BlockNumber) -> Option<Block> {
        self.load_commit_op(block_number).await.map(|r| r.block)
    }

    /// Returns the number of last block
    pub async fn get_last_committed_block(&mut self) -> QueryResult<BlockNumber> {
        OperationsSchema(self.0)
            .get_last_block_by_action(ActionType::COMMIT, None)
            .await
    }

    /// Returns the number of last block for which proof has been created.
    ///
    /// Note: having a proof for the block doesn't mean that state was updated. Chain state
    /// is updated only after corresponding transaction is confirmed on the Ethereum blockchain.
    /// In order to see the last block with updated state, use `get_last_verified_confirmed_block` method.
    pub async fn get_last_verified_block(&mut self) -> QueryResult<BlockNumber> {
        OperationsSchema(self.0)
            .get_last_block_by_action(ActionType::VERIFY, None)
            .await
    }

    /// Returns the number of last block for which proof has been confirmed on Ethereum.
    /// Essentially, it's number of last block for which updates were applied to the chain state.
    pub async fn get_last_verified_confirmed_block(&mut self) -> QueryResult<BlockNumber> {
        OperationsSchema(self.0)
            .get_last_block_by_action(ActionType::VERIFY, Some(true))
            .await
    }

    async fn load_storage_pending_block(&mut self) -> QueryResult<Option<StoragePendingBlock>> {
        let maybe_block = sqlx::query_as!(
            StoragePendingBlock,
            "SELECT * FROM pending_block
            ORDER BY number DESC
            LIMIT 1"
        )
        .fetch_optional(self.0.conn())
        .await?;

        Ok(maybe_block)
    }

    pub async fn load_pending_block(&mut self) -> QueryResult<Option<PendingBlock>> {
        let mut transaction = self.0.start_transaction().await?;

        let pending_block_result = BlockSchema(&mut transaction)
            .load_storage_pending_block()
            .await?;

        let block = match pending_block_result {
            Some(block) => block,
            None => return Ok(None),
        };

        let executed_ops = BlockSchema(&mut transaction)
            .get_block_executed_ops(block.number as u32)
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
            number: block.number as u32,
            chunks_left: block.chunks_left as usize,
            unprocessed_priority_op_before: block.unprocessed_priority_op_before as u64,
            pending_block_iteration: block.pending_block_iteration as usize,
            success_operations,
            failed_txs,
        };

        transaction.commit().await?;

        Ok(Some(result))
    }

    /// Returns `true` if there is a stored pending block in the database.
    pub async fn pending_block_exists(&mut self) -> QueryResult<bool> {
        let result = self.load_storage_pending_block().await?.is_some();

        Ok(result)
    }

    pub async fn save_pending_block(&mut self, pending_block: PendingBlock) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;

        let storage_block = StoragePendingBlock {
            number: pending_block.number.into(),
            chunks_left: pending_block.chunks_left as i64,
            unprocessed_priority_op_before: pending_block.unprocessed_priority_op_before as i64,
            pending_block_iteration: pending_block.pending_block_iteration as i64,
        };

        // Store the pending block header.
        sqlx::query!("
            INSERT INTO pending_block (number, chunks_left, unprocessed_priority_op_before, pending_block_iteration)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (number)
            DO UPDATE
              SET chunks_left = $2, unprocessed_priority_op_before = $3, pending_block_iteration = $4
            ",
            storage_block.number, storage_block.chunks_left, storage_block.unprocessed_priority_op_before, storage_block.pending_block_iteration,
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

        Ok(())
    }

    pub async fn count_operations(
        &mut self,
        action_type: ActionType,
        is_confirmed: bool,
    ) -> QueryResult<i64> {
        let count = sqlx::query!(
            r#"SELECT count(*) as "count!" FROM operations WHERE action_type = $1 AND confirmed = $2"#,
            action_type.to_string(),
            is_confirmed
        )
        .fetch_one(self.0.conn())
        .await?
        .count;

        Ok(count)
    }

    pub(crate) async fn save_block(&mut self, block: Block) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;

        let number = i64::from(block.block_number);
        let root_hash = block.new_root_hash.to_bytes();
        let fee_account_id = i64::from(block.fee_account);
        let unprocessed_prior_op_before = block.processed_priority_ops.0 as i64;
        let unprocessed_prior_op_after = block.processed_priority_ops.1 as i64;
        let block_size = block.block_chunks_size as i64;
        let commit_gas_limit = block.commit_gas_limit.as_u64() as i64;
        let verify_gas_limit = block.verify_gas_limit.as_u64() as i64;

        BlockSchema(&mut transaction)
            .save_block_transactions(block.block_number, block.block_transactions)
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
            INSERT INTO blocks (number, root_hash, fee_account_id, unprocessed_prior_op_before, unprocessed_prior_op_after, block_size, commit_gas_limit, verify_gas_limit)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ",
            new_block.number, new_block.root_hash, new_block.fee_account_id, new_block.unprocessed_prior_op_before,
            new_block.unprocessed_prior_op_after, new_block.block_size, new_block.commit_gas_limit, new_block.verify_gas_limit,
        ).execute(transaction.conn())
        .await?;

        transaction.commit().await?;

        Ok(())
    }

    /// Stores account tree cache for a block
    pub async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        tree_cache: serde_json::Value,
    ) -> QueryResult<()> {
        if block == 0 {
            return Ok(());
        }

        let tree_cache_str =
            serde_json::to_string(&tree_cache).expect("Failed to serialize Account Tree Cache");
        sqlx::query!(
            "
            INSERT INTO account_tree_cache (block, tree_cache)
            VALUES ($1, $2)
            ",
            block as i64,
            tree_cache_str,
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    /// Gets stored account tree cache for a block
    pub async fn get_account_tree_cache(
        &mut self,
    ) -> QueryResult<Option<(BlockNumber, serde_json::Value)>> {
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

        Ok(account_tree_cache.map(|w| {
            (
                w.block as BlockNumber,
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
        let account_tree_cache = sqlx::query_as!(
            AccountTreeCache,
            "
            SELECT * FROM account_tree_cache
            WHERE block = $1
            ",
            block as i64
        )
        .fetch_optional(self.0.conn())
        .await?;

        Ok(account_tree_cache.map(|w| {
            serde_json::from_str(&w.tree_cache).expect("Failed to deserialize Account Tree Cache")
        }))
    }
}
