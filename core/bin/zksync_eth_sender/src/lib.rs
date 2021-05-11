//! `eth_sender` module is capable of synchronizing the operations
//! occurring in `ZKSync` with the Ethereum blockchain by creating
//! transactions from the operations, sending them and ensuring that
//! every transaction is executed successfully and confirmed.

// Built-in deps
use std::collections::VecDeque;
use std::time::{Duration, Instant};
// External uses
use anyhow::format_err;
use tokio::{task::JoinHandle, time};
use web3::{
    contract::Options,
    types::{TransactionReceipt, H256, U256},
};
// Workspace uses
use zksync_config::{ETHSenderConfig, ZkSyncConfig};
use zksync_eth_client::{EthereumGateway, SignedCallResult};
use zksync_storage::ConnectionPool;
use zksync_types::ethereum::ETHOperation;
// Local uses
use self::{
    database::{Database, DatabaseInterface},
    gas_adjuster::GasAdjuster,
    transactions::*,
    tx_queue::{TxData, TxQueue, TxQueueBuilder},
};
use zksync_types::{
    aggregated_operations::{AggregatedActionType, AggregatedOperation},
    gas_counter::GasCounter,
};

mod database;
mod gas_adjuster;
mod transactions;
mod tx_queue;

#[cfg(test)]
mod tests;

/// Wait this amount of time if we hit rate limit on infura https://infura.io/docs/ethereum/json-rpc/ratelimits
const RATE_LIMIT_BACKOFF_PERIOD: Duration = Duration::from_secs(30);
/// Rate limit error will contain this response code
const RATE_LIMIT_HTTP_CODE: &str = "429";

/// `TxCheckMode` enum determines the policy on the obtaining the tx status.
/// The latest sent transaction can be pending (we're still waiting for it),
/// but if there is more than one tx for some Ethereum operation, it means that we
/// already know that these transactions were considered stuck. Thus, lack of
/// response (either successful or unsuccessful) for any of the old txs means
/// that this transaction is still stuck.
#[derive(Debug, Clone, PartialEq)]
enum TxCheckMode {
    /// Mode for the latest sent tx (pending state is allowed).
    Latest,
    /// Mode for the latest sent tx (pending state is not allowed).
    Old,
}

/// `ETHSender` is a structure capable of anchoring
/// the ZKSync operations to the Ethereum blockchain.
///
/// # Description
///
/// The essential part of this structure is an event loop (which is supposed to be run
/// in a separate thread), which obtains the operations to commit through the channel,
/// and then commits them to the Ethereum, ensuring that all the transactions are
/// successfully included in blocks and executed.
///
/// Also `ETHSender` preserves the order of operations: it guarantees that operations
/// are committed in FIFO order, meaning that until the older operation of certain type (e.g.
/// `commit`) will always be committed before the newer one.
///
/// However, multiple transaction can be "in flight" at the same time, see "Concurrent transaction
/// sending" section for details.
///
/// # Transaction sending policy
///
/// The goal is to handle stuck transactions.
///
/// When we try to commit operation to ETH, we select nonce, gas price, sign
/// transaction and watch for its confirmations.
///
/// If transaction is not confirmed for a while, we increase the gas price and do the same, but we
/// keep the list of all sent transaction hashes for one particular operations, since we can't be
/// sure which one will be committed; thus we have to track all of them.
///
/// Note: make sure to save signed tx to db before sending it to ETH, this way we can be sure
/// that state is always recoverable.
///
/// # Concurrent transaction sending
///
/// `ETHSender` supports sending multiple transaction to the Ethereum at the same time.
/// This can be configured by the constructor `max_txs_in_flight` parameter. The order of
/// transaction is still guaranteed to be preserved, since every sent tx has the assigned nonce
/// which makes it impossible to get sent transactions committed out of order.
///
/// Internally order of the transaction is determined by the underlying `TxQueue`, which provides
/// transactions to send for `ETHSender` according to the following priority:
///
/// 1. Verify operations (only if the corresponding commit operation was sent)
/// 2. Withdraw operations (only if both commit/verify for the same block operations were sent).
/// 3. Commit operations.
///
/// # Failure policy
///
/// By default, `ETHSender` expects no transactions to fail, and thus upon a failure it will
/// report the incident to the log and then panic to prevent continue working in a probably
/// erroneous conditions. Failure handling policy is determined by a corresponding callback,
/// which can be changed if needed.
struct ETHSender<DB: DatabaseInterface> {
    /// Ongoing operations queue.
    ongoing_ops: VecDeque<ETHOperation>,
    /// Connection to the database.
    db: DB,
    /// Ethereum intermediator.
    ethereum: EthereumGateway,
    /// Queue for ordered transaction processing.
    tx_queue: TxQueue,
    /// Utility for managing the gas price for transactions.
    gas_adjuster: GasAdjuster<DB>,
    /// Settings for the `ETHSender`.
    options: ETHSenderConfig,
}

impl<DB: DatabaseInterface> ETHSender<DB> {
    pub async fn new(options: ETHSenderConfig, db: DB, ethereum: EthereumGateway) -> Self {
        let mut connection = db
            .acquire_connection()
            .await
            .expect("Unable to connect to DB");
        let mut transaction = connection
            .start_transaction()
            .await
            .expect("Unable create database transaction");

        db.restore_unprocessed_operations(&mut transaction)
            .await
            .expect("Can't restore unprocessed operations");

        let ongoing_ops = db
            .load_unconfirmed_operations(&mut transaction)
            .await
            .expect("Can't restore state");

        let operations_id = ongoing_ops
            .iter()
            .filter_map(|eth_op| eth_op.op.as_ref())
            .map(|aggregated_op| aggregated_op.0)
            .collect::<Vec<_>>();
        db.remove_unprocessed_operations(&mut transaction, operations_id)
            .await
            .expect("Failed remove unprocessed operations");

        let stats = db
            .load_stats(&mut transaction)
            .await
            .expect("Failed loading ETH operations stats");

        let tx_queue = TxQueueBuilder::new(options.sender.max_txs_in_flight as usize)
            .with_sent_pending_txs(ongoing_ops.len())
            .with_commit_operations_count(stats.last_committed_block)
            .with_verify_operations_count(stats.last_verified_block)
            .with_execute_operations_count(stats.last_executed_block)
            .build();

        let gas_adjuster = GasAdjuster::new(&db).await;

        transaction
            .commit()
            .await
            .expect("Failed commit database transaction");
        drop(connection);

        Self {
            ongoing_ops,
            db,
            ethereum,
            tx_queue,
            gas_adjuster,
            options,
        }
    }

    /// Main routine of `ETHSender`.
    pub async fn run(mut self) {
        loop {
            // We perform a loading routine every X seconds.
            tokio::time::delay_for(self.options.sender.tx_poll_period()).await;
            // If we received an error when loading a new operation, we can't do anything about it and should panic.
            if let Err(error) = self.load_new_operations().await {
                vlog::error!("Unable to restore operations from the database: {}", error);
                panic!("Unable to restore operations from the database: {}", error);
            }

            if self.options.sender.is_enabled {
                // ...and proceed them.
                self.proceed_next_operations().await;
                // Update the gas adjuster to maintain the up-to-date max gas price limit.
                self.gas_adjuster
                    .keep_updated(&self.ethereum, &self.db)
                    .await;
            }
        }
    }

    /// Gets the incoming operations from the database and adds them to the
    /// transactions queue.
    async fn load_new_operations(&mut self) -> anyhow::Result<()> {
        let start = Instant::now();
        let mut connection = self.db.acquire_connection().await?;
        let mut transaction = connection.start_transaction().await?;

        let new_operations = self.db.load_new_operations(&mut transaction).await?;

        if !new_operations.is_empty() {
            vlog::info!("Loaded {} new operations", new_operations.len());
        } else {
            vlog::debug!("No new operations are loaded from the database");
        }

        // let's mark the operations as successful processed.
        // So that next time you do not add them to the queue again.
        let operations_id = new_operations.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        self.db
            .remove_unprocessed_operations(&mut transaction, operations_id)
            .await?;

        transaction.commit().await?;
        drop(connection);

        for operation in new_operations {
            self.add_operation_to_queue(operation.clone())?;
        }

        metrics::histogram!("eth_sender.load_new_operations", start.elapsed());
        Ok(())
    }

    /// This method does two main things:
    ///
    /// 1. Pops all the available transactions from the `TxQueue` and sends them.
    /// 2. Sifts all the ongoing operations, filtering the completed ones and
    ///   managing the rest (e.g. by sending a supplement txs for stuck operations).
    async fn proceed_next_operations(&mut self) {
        let start = Instant::now();
        // Queue for storing all the operations that were not finished at this iteration.
        let mut new_ongoing_ops = VecDeque::new();

        while let Some(tx) = self.tx_queue.pop_front() {
            if let Err(e) = self.initialize_operation(tx.clone()).await {
                Self::process_error(e).await;
                // Return the unperformed operation to the queue, since failing the
                // operation initialization means that it was not stored in the database.
                if let Err(err_message) = self.tx_queue.return_popped(tx) {
                    panic!(
                        "Failed return previous sent operation to the queue: {}",
                        err_message
                    );
                }
            }
        }

        // Commit the next operations (if any).
        while let Some(mut current_op) = self.ongoing_ops.pop_front() {
            // We perform a commitment step here. In case of error, we suppose that this is some
            // network issue which won't appear the next time, so we report the situation to the
            // log and consider the operation pending (meaning that we won't process it on this
            // step, but will try to do so on the next one).
            let commitment = match self.perform_commitment_step(&mut current_op).await {
                Ok(commitment) => commitment,
                Err(e) => {
                    Self::process_error(e).await;
                    OperationCommitment::Pending
                }
            };

            match commitment {
                OperationCommitment::Committed => {
                    // Free a slot for the next tx in the queue.
                    self.tx_queue.report_commitment();
                }
                OperationCommitment::Pending => {
                    // Poll this operation on the next iteration.
                    new_ongoing_ops.push_back(current_op);
                }
            }
        }

        assert!(
            self.ongoing_ops.is_empty(),
            "Ongoing ops queue should be empty after draining"
        );

        // Store the ongoing operations for the next round.
        self.ongoing_ops = new_ongoing_ops;
        metrics::histogram!("eth_sender.proceed_next_operations", start.elapsed());
    }

    async fn process_error(err: anyhow::Error) {
        vlog::warn!("Error while trying to complete uncommitted op: {}", err);
        if err.to_string().contains(RATE_LIMIT_HTTP_CODE) {
            vlog::warn!(
                "Received rate limit response, waiting for {}s",
                RATE_LIMIT_BACKOFF_PERIOD.as_secs()
            );
            // This metric is needed to track how much time is spent in backoff mode
            // and trigger grafana alerts
            metrics::histogram!("eth_sender.backoff_mode", RATE_LIMIT_BACKOFF_PERIOD);
            time::delay_for(RATE_LIMIT_BACKOFF_PERIOD).await;
        }
    }

    /// Stores the new operation in the database and sends the corresponding transaction.
    async fn initialize_operation(&mut self, tx: TxData) -> anyhow::Result<()> {
        let current_block = self.ethereum.block_number().await?;
        let deadline_block = self.get_deadline_block(current_block.as_u64());
        let gas_price = self
            .gas_adjuster
            .get_gas_price(&self.ethereum, None)
            .await?;

        let mut connection = self.db.acquire_connection().await?;
        let mut transaction = connection.start_transaction().await?;

        // let (new_op, signed_tx) = self.db.transaction(|| {
        let (new_op, signed_tx) = {
            // First, we should store the operation in the database and obtain the assigned
            // operation ID and nonce. Without them we won't be able to sign the transaction.
            let assigned_data = self
                .db
                .save_new_eth_tx(
                    &mut transaction,
                    tx.op_type,
                    Some(tx.operation.clone()),
                    deadline_block as i64,
                    gas_price,
                    tx.raw.clone(),
                )
                .await?;

            let mut new_op = ETHOperation {
                id: assigned_data.id,
                op_type: tx.op_type,
                op: Some(tx.operation),
                nonce: assigned_data.nonce,
                last_deadline_block: deadline_block,
                last_used_gas_price: gas_price,
                used_tx_hashes: vec![], // No hash yet, will be added below.
                encoded_tx_data: tx.raw,
                confirmed: false,
                final_hash: None,
            };

            // Sign the transaction.
            let signed_tx = Self::sign_new_tx(&self.ethereum, &new_op).await?;

            // With signed tx, update the hash in the operation entry and in the db.
            new_op.used_tx_hashes.push(signed_tx.hash);
            self.db
                .add_hash_entry(&mut transaction, new_op.id, &signed_tx.hash)
                .await?;

            (new_op, signed_tx)
        };

        // We should store the operation as `ongoing` **before** sending it as well,
        // so if sending will fail, we won't forget about it.
        self.ongoing_ops.push_back(new_op.clone());

        // After storing all the tx data in the database, we can finally send the tx.
        vlog::info!(
            "Sending new tx: [ETH Operation <id: {}, type: {:?}>. ETH tx: {}. ZKSync operation: {}]",
            new_op.id, new_op.op_type, self.eth_tx_description(&signed_tx), self.zksync_operation_description(&new_op),
        );
        if let Err(e) = self.ethereum.send_raw_tx(signed_tx.raw_tx).await {
            // Sending tx error is not critical: this will result in transaction being considered stuck,
            // and resent. We can't do anything about this failure either, since it's most probably is not
            // related to the node logic, so we just log this error and pretend to have this operation
            // processed.
            vlog::warn!("Error while sending the operation: {}", e);
        }

        transaction.commit().await?;

        Ok(())
    }

    /// Helper method to obtain the string representation of the Ethereum transaction.
    /// Intended to be used for log entries.
    fn eth_tx_description(&self, tx: &SignedCallResult) -> String {
        // Gas price in gwei (wei / 10^9).
        let gas_price = tx.gas_price / (1_000_000_000);
        format!(
            "<hash: {:#x}; gas price: {} gwei; nonce: {}>",
            tx.hash, gas_price, tx.nonce
        )
    }

    /// Helper method to obtain the string representation of the zkSync operation.
    /// Intended to be used for log entries.
    fn zksync_operation_description(&self, operation: &ETHOperation) -> String {
        if let Some((id, op)) = &operation.op {
            let (first_block, last_block) = op.get_block_range();
            format!(
                "<id {}; action: {}; blocks: {}-{}>",
                id,
                op.get_action_type().to_string(),
                first_block,
                last_block
            )
        } else {
            "<not applicable>".into()
        }
    }

    /// Handles the ongoing operation by checking its state and doing the following:
    /// - If the transaction is either pending or completed, stops the execution (as
    ///   there is nothing to do with the operation yet).
    /// - If the transaction is stuck, sends a supplement transaction for it.
    /// - If the transaction is failed, handles the failure according to the failure
    ///   processing policy.
    async fn perform_commitment_step(
        &mut self,
        op: &mut ETHOperation,
    ) -> anyhow::Result<OperationCommitment> {
        let start = Instant::now();
        assert!(
            !op.used_tx_hashes.is_empty(),
            "OperationETHState should have at least one transaction"
        );

        let current_block = self.ethereum.block_number().await?;

        // Check statuses of existing transactions.
        // Go through every transaction in a loop. We will exit this method early
        // if there will be discovered a pending or successfully committed transaction.
        for (idx, tx_hash) in op.used_tx_hashes.iter().enumerate() {
            let mode = if idx == op.used_tx_hashes.len() - 1 {
                TxCheckMode::Latest
            } else {
                TxCheckMode::Old
            };

            match self
                .check_transaction_state(mode, op, *tx_hash, current_block.as_u64())
                .await?
            {
                TxCheckOutcome::Pending => {
                    // Transaction is pending, nothing to do yet.
                    return Ok(OperationCommitment::Pending);
                }
                TxCheckOutcome::Committed => {
                    let mut connection = self.db.acquire_connection().await?;
                    let mut transaction = connection.start_transaction().await?;

                    // While transactions are sent in order, has to be processed in order due to nonce,
                    // and checked for commitment also in the same order, we still must check that previous
                    // operation was confirmed.
                    //
                    // Consider the following scenario:
                    // 1. Two Verify operations are sent to the Ethereum and included into one block.
                    // 2. We start checking sent operations in a loop.
                    // 3. First operation is considered pending, due to not having enough confirmations.
                    // 4. After check, a new Ethereum block is created.
                    // 5. Later in the loop we check the second Verify operation, and it's considered committed.
                    // 6. State is updated according to operation Verify#2.
                    // 7. On the next round, Verify#1 is also considered confirmed.
                    // 8. State is updated according to operation Verify#1, and likely some data is overwritten.
                    //
                    // For commit operations consequences aren't that drastic, but still it's not correct to confirm
                    // operations out of order.
                    if !self
                        .db
                        .is_previous_operation_confirmed(&mut transaction, &op)
                        .await?
                    {
                        vlog::info!("ETH Operation <id: {}> is confirmed ahead of time, considering it pending for now", op.id);
                        return Ok(OperationCommitment::Pending);
                    }

                    vlog::info!(
                        "Confirmed: [ETH Operation <id: {}, type: {:?}>. Tx hash: <{:#x}>. ZKSync operation: {}]",
                        op.id, op.op_type, tx_hash, self.zksync_operation_description(op),
                    );
                    self.db
                        .confirm_operation(&mut transaction, tx_hash, op)
                        .await?;
                    transaction.commit().await?;
                    return Ok(OperationCommitment::Committed);
                }
                TxCheckOutcome::Stuck => {
                    // We do nothing for a stuck transaction. If this will be
                    // the last entry of the list, a new tx will be sent.
                }
                TxCheckOutcome::Failed(receipt) => {
                    vlog::warn!(
                        "ETH transaction failed: tx: {:#x}, op_type: {:?}, op: {:?}; tx_receipt: {:#?} ",
                        tx_hash,
                        op.op_type,
                        op.op,
                        receipt,
                    );
                    // Process the failure according to the chosen policy.
                    self.failure_handler(&receipt).await;
                }
            }
        }

        // Reaching this point will mean that the latest transaction got stuck.
        // We should create another tx based on it, and send it.
        let deadline_block = self.get_deadline_block(current_block.as_u64());
        // Raw tx contents are the same for every transaction, so we just
        // create a new one from the old one with updated parameters.
        let new_tx = self.create_supplement_tx(deadline_block, op).await?;
        // New transaction should be persisted in the DB *before* sending it.

        let mut connection = self.db.acquire_connection().await?;
        let mut transaction = connection.start_transaction().await?;
        self.db
            .update_eth_tx(
                &mut transaction,
                op.id,
                deadline_block as i64,
                new_tx.gas_price,
            )
            .await?;
        self.db
            .add_hash_entry(&mut transaction, op.id, &new_tx.hash)
            .await?;

        vlog::info!(
            "Stuck tx processing: sending tx for op, eth_op_id: {}; ETH tx: {}",
            op.id,
            self.eth_tx_description(&new_tx),
        );
        self.ethereum.send_raw_tx(new_tx.raw_tx).await?;
        transaction.commit().await?;

        metrics::histogram!("eth_sender.perform_commitment_step", start.elapsed());
        Ok(OperationCommitment::Pending)
    }

    /// Handles a transaction execution failure by reporting the issue to the log
    /// and terminating the node.
    async fn failure_handler(&self, receipt: &TransactionReceipt) -> ! {
        vlog::error!(
            "Ethereum transaction unexpectedly failed. Receipt: {:#?}",
            receipt
        );
        if let Ok(Some(reason)) = self.ethereum.failure_reason(receipt.transaction_hash).await {
            vlog::error!("Failure reason for Ethereum tx: {:#?}", reason);
        } else {
            vlog::error!("Unable to receive failure reason for Ethereum tx");
        }
        panic!("Cannot operate after unexpected TX failure");
    }

    /// Helper method encapsulating the logic of determining the next deadline block.
    fn get_deadline_block(&self, current_block: u64) -> u64 {
        current_block + self.options.sender.expected_wait_time_block
    }

    /// Looks up for a transaction state on the Ethereum chain
    /// and reduces it to the simpler `TxCheckOutcome` report.
    async fn check_transaction_state(
        &self,
        mode: TxCheckMode,
        op: &ETHOperation,
        tx_hash: H256,
        current_block: u64,
    ) -> anyhow::Result<TxCheckOutcome> {
        let status = self.ethereum.get_tx_status(tx_hash).await?;

        let outcome = match status {
            // Successful execution.
            Some(status) if status.success => {
                // Check if transaction has enough confirmations.
                if status.confirmations >= self.options.sender.wait_confirmations {
                    TxCheckOutcome::Committed
                } else {
                    TxCheckOutcome::Pending
                }
            }
            // Non-successful execution, report the failure with details.
            Some(status) => {
                // Check if transaction has enough confirmations.
                if status.confirmations >= self.options.sender.wait_confirmations {
                    assert!(
                        status.receipt.is_some(),
                        "Receipt should exist for a failed transaction"
                    );

                    TxCheckOutcome::Failed(Box::new(status.receipt.unwrap()))
                } else {
                    TxCheckOutcome::Pending
                }
            }
            // Stuck transaction.
            None if op.is_stuck(current_block) => TxCheckOutcome::Stuck,
            // No status yet. If this is a latest transaction, it's pending.
            // For an old tx it means that it's still stuck.
            None => match mode {
                TxCheckMode::Latest => TxCheckOutcome::Pending,
                TxCheckMode::Old => TxCheckOutcome::Stuck,
            },
        };

        Ok(outcome)
    }

    /// Creates a new Ethereum operation.
    async fn sign_new_tx(
        ethereum: &EthereumGateway,
        op: &ETHOperation,
    ) -> anyhow::Result<SignedCallResult> {
        let tx_options = {
            // We set the gas limit for commit / verify operations as pre-calculated estimation.
            // This estimation is a higher bound based on a pre-calculated cost of every operation in the block.
            let gas_limit = Self::gas_limit_for_op(op);

            assert!(
                gas_limit > 0.into(),
                "Proposed gas limit for operation is 0; operation: {:?}",
                op
            );

            vlog::info!(
                "Gas limit for <ETH Operation id: {}> is {}",
                op.id,
                gas_limit
            );

            Options {
                nonce: Some(op.nonce),
                gas_price: Some(op.last_used_gas_price),
                gas: Some(gas_limit),
                ..Default::default()
            }
        };

        let signed_tx = ethereum
            .sign_prepared_tx(op.encoded_tx_data.clone(), tx_options)
            .await?;

        Ok(signed_tx)
    }

    /// Calculates the gas limit for transaction to be send, depending on the type of operation.
    fn gas_limit_for_op(op: &ETHOperation) -> U256 {
        let (_, op) = op
            .op
            .as_ref()
            .expect("Operation not found - can't compute gas limit");
        match op {
            AggregatedOperation::CommitBlocks(commit) => {
                GasCounter::commit_gas_limit_aggregated(&commit.blocks)
            }
            AggregatedOperation::ExecuteBlocks(execute) => {
                GasCounter::execute_gas_limit_aggregated(&execute.blocks)
            }
            AggregatedOperation::PublishProofBlocksOnchain(_) => {
                U256::from(GasCounter::BASE_PROOF_BLOCKS_TX_COST)
            }
            AggregatedOperation::CreateProofBlocks(_) => {
                panic!("Can't compute gas limit for CreateProofBlocks")
            }
        }
    }

    /// Creates a new transaction for the existing Ethereum operation.
    /// This method is used to create supplement transactions instead of the stuck one.
    async fn create_supplement_tx(
        &mut self,
        deadline_block: u64,
        stuck_tx: &mut ETHOperation,
    ) -> anyhow::Result<SignedCallResult> {
        let tx_options = self.tx_options_from_stuck_tx(stuck_tx).await?;

        let raw_tx = stuck_tx.encoded_tx_data.clone();
        let signed_tx = self.ethereum.sign_prepared_tx(raw_tx, tx_options).await?;

        stuck_tx.last_deadline_block = deadline_block;
        stuck_tx.last_used_gas_price = signed_tx.gas_price;
        stuck_tx.used_tx_hashes.push(signed_tx.hash);

        Ok(signed_tx)
    }

    /// Creates a new tx options from a stuck transaction, with updated gas amount
    /// and nonce.
    async fn tx_options_from_stuck_tx(
        &mut self,
        stuck_tx: &ETHOperation,
    ) -> anyhow::Result<Options> {
        let old_tx_gas_price = stuck_tx.last_used_gas_price;

        let new_gas_price = self
            .gas_adjuster
            .get_gas_price(&self.ethereum, Some(old_tx_gas_price))
            .await?;
        let nonce = stuck_tx.nonce;
        let gas_limit = Self::gas_limit_for_op(stuck_tx);

        assert!(
            gas_limit > 0.into(),
            "Proposed gas limit for (stuck) operation is 0; operation: {:?}",
            stuck_tx
        );

        vlog::info!(
            "Replacing tx: hash: {:#x}, old_gas: {}, new_gas: {}, used nonce: {}, gas limit: {}",
            stuck_tx.used_tx_hashes.last().unwrap(),
            old_tx_gas_price,
            new_gas_price,
            nonce,
            gas_limit,
        );

        Ok(Options::with(move |opt| {
            opt.gas_price = Some(new_gas_price);
            opt.nonce = Some(nonce);
            opt.gas = Some(gas_limit);
        }))
    }

    /// Encodes the operation data to the Ethereum tx payload (not signs it!).
    fn operation_to_raw_tx(&self, op: &AggregatedOperation) -> Vec<u8> {
        match op {
            AggregatedOperation::CommitBlocks(operation) => {
                let args = operation.get_eth_tx_args();
                self.ethereum
                    .encode_tx_data("commitBlocks", args.as_slice())
            }
            AggregatedOperation::CreateProofBlocks(..) => {
                panic!("Eth sender should ignore CreateProofBlocks");
            } // not for eth sender
            AggregatedOperation::PublishProofBlocksOnchain(operation) => {
                let args = operation.get_eth_tx_args();
                self.ethereum.encode_tx_data("proveBlocks", args.as_slice())
            }
            AggregatedOperation::ExecuteBlocks(operation) => {
                let args = operation.get_eth_tx_args();
                self.ethereum
                    .encode_tx_data("executeBlocks", args.as_slice())
            }
        }
    }

    /// Encodes the zkSync operation to the tx payload and adds it to the queue.
    fn add_operation_to_queue(&mut self, op: (i64, AggregatedOperation)) -> anyhow::Result<()> {
        let raw_tx = self.operation_to_raw_tx(&op.1);
        let tx_data = TxData::from_operation(op, raw_tx);

        match tx_data.op_type {
            AggregatedActionType::CommitBlocks => self.tx_queue.add_commit_operation(tx_data)?,
            AggregatedActionType::PublishProofBlocksOnchain => {
                self.tx_queue.add_verify_operation(tx_data)?
            }
            AggregatedActionType::ExecuteBlocks => self.tx_queue.add_execute_operation(tx_data)?,
            AggregatedActionType::CreateProofBlocks => {
                return Err(format_err!(
                    "Can't add CreateProofBlocks operation to transaction queue"
                ));
            }
        }

        Ok(())
    }
}

#[must_use]
pub fn run_eth_sender(
    pool: ConnectionPool,
    eth_gateway: EthereumGateway,
    options: ZkSyncConfig,
) -> JoinHandle<()> {
    let db = Database::new(pool);

    tokio::spawn(async move {
        let eth_sender = ETHSender::new(options.eth_sender, db, eth_gateway).await;

        eth_sender.run().await
    })
}
