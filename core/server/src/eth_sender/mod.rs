//! `eth_sender` module is capable of synchronizing the operations
//! occurring in `ZKSync` with the Ethereum blockchain by creating
//! transactions from the operations, sending them and ensuring that
//! every transaction is executed successfully and confirmed.

// Built-in deps
use std::collections::VecDeque;
// External uses
use futures::channel::mpsc;
use tokio::runtime::Runtime;
use tokio::time;
use web3::contract::Options;
use web3::types::{TransactionReceipt, H256};
// Workspace uses
use eth_client::SignedCallResult;
use models::{
    config_options::{ConfigurationOptions, EthSenderOptions, ThreadPanicNotify},
    ethereum::{ETHOperation, OperationType},
    node::config,
    Action, Operation,
};
use storage::ConnectionPool;
// Local uses
use self::{
    database::{Database, DatabaseAccess},
    ethereum_interface::{EthereumHttpClient, EthereumInterface},
    gas_adjuster::GasAdjuster,
    transactions::*,
    tx_queue::{TxData, TxQueue, TxQueueBuilder},
};

mod database;
mod ethereum_interface;
mod gas_adjuster;
mod transactions;
mod tx_queue;

#[cfg(test)]
mod tests;

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
struct ETHSender<ETH: EthereumInterface, DB: DatabaseAccess> {
    /// Ongoing operations queue.
    ongoing_ops: VecDeque<ETHOperation>,
    /// Connection to the database.
    db: DB,
    /// Ethereum intermediator.
    ethereum: ETH,
    /// Channel for receiving operations to commit.
    rx_for_eth: mpsc::Receiver<Operation>,
    /// Channel to notify about committed operations.
    op_notify: mpsc::Sender<Operation>,
    /// Queue for ordered transaction processing.
    tx_queue: TxQueue,
    /// Utility for managing the gas price for transactions.
    gas_adjuster: GasAdjuster<ETH, DB>,
    /// Settings for the `ETHSender`.
    options: EthSenderOptions,
}

impl<ETH: EthereumInterface, DB: DatabaseAccess> ETHSender<ETH, DB> {
    pub fn new(
        options: EthSenderOptions,
        db: DB,
        ethereum: ETH,
        rx_for_eth: mpsc::Receiver<Operation>,
        op_notify: mpsc::Sender<Operation>,
    ) -> Self {
        let (ongoing_ops, unprocessed_ops) = db.restore_state().expect("Can't restore state");

        let stats = db
            .load_stats()
            .expect("Failed loading ETH operations stats");

        let tx_queue = TxQueueBuilder::new(options.max_txs_in_flight as usize)
            .with_sent_pending_txs(ongoing_ops.len())
            .with_commit_operations_count(stats.commit_ops)
            .with_verify_operations_count(stats.verify_ops)
            .with_withdraw_operations_count(stats.withdraw_ops)
            .build();

        let gas_adjuster = GasAdjuster::new(&db);

        let mut sender = Self {
            ethereum,
            ongoing_ops,
            db,
            rx_for_eth,
            op_notify,
            tx_queue,
            gas_adjuster,
            options,
        };

        // Add all the unprocessed operations to the queue.
        for operation in unprocessed_ops {
            info!(
                "Adding unprocessed ZKSync operation <id {}; action: {}; block: {}> to queue",
                operation.id.expect("ID must be set"),
                operation.action.to_string(),
                operation.block.block_number
            );
            sender.add_operation_to_queue(operation);
        }

        sender
    }

    /// Main routine of `ETHSender`.
    pub async fn run(mut self) {
        let mut timer = time::interval(self.options.tx_poll_period);

        loop {
            // Update the incoming operations.
            self.retrieve_operations();
            timer.tick().await;

            // ...and proceed them.
            self.proceed_next_operations();

            // Update the gas adjuster to maintain the up-to-date max gas price limit.
            self.gas_adjuster.keep_updated(&self.db);
        }
    }

    /// Gets the incoming operations from the channel and adds them to the
    /// transactions queue.
    fn retrieve_operations(&mut self) {
        while let Ok(Some(operation)) = self.rx_for_eth.try_next() {
            info!(
                "Adding ZKSync operation <id {}; action: {}; block: {}> to queue",
                operation.id.expect("ID must be set"),
                operation.action.to_string(),
                operation.block.block_number
            );
            self.add_operation_to_queue(operation);
        }
    }

    /// This method does two main things:
    ///
    /// 1. Pops all the available transactions from the `TxQueue` and sends them.
    /// 2. Sifts all the ongoing operations, filtering the completed ones and
    ///   managing the rest (e.g. by sending a supplement txs for stuck operations).
    fn proceed_next_operations(&mut self) {
        // Queue for storing all the operations that were not finished at this iteration.
        let mut new_ongoing_ops = VecDeque::new();

        while let Some(tx) = self.tx_queue.pop_front() {
            self.initialize_operation(tx.clone()).unwrap_or_else(|e| {
                warn!("Error while trying to complete uncommitted op: {}", e);

                // Return the unperformed operation to the queue, since failing the
                // operation initialization means that it was not stored in the database.
                self.tx_queue.return_popped(tx);
            });
        }

        // Commit the next operations (if any).
        while let Some(mut current_op) = self.ongoing_ops.pop_front() {
            // We perform a commitment step here. In case of error, we suppose that this is some
            // network issue which won't appear the next time, so we report the situation to the
            // log and consider the operation pending (meaning that we won't process it on this
            // step, but will try to do so on the next one).
            let commitment = self
                .perform_commitment_step(&mut current_op)
                .map_err(|e| {
                    warn!("Error while trying to complete uncommitted op: {}", e);
                })
                .unwrap_or(OperationCommitment::Pending);

            match commitment {
                OperationCommitment::Committed => {
                    // Free a slot for the next tx in the queue.
                    self.tx_queue.report_commitment();

                    if current_op.is_verify() {
                        // We notify about verify only when it's confirmed on the Ethereum.
                        self.op_notify
                            .try_send(current_op.op.expect("Should be verify operation"))
                            .map_err(|e| warn!("Failed notify about verify op confirmation: {}", e))
                            .unwrap_or_default();

                        // Complete pending withdrawals after each verify.
                        self.add_complete_withdrawals_to_queue();
                    }
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
    }

    /// Stores the new operation in the database and sends the corresponding transaction.
    fn initialize_operation(&mut self, tx: TxData) -> Result<(), failure::Error> {
        let current_block = self.ethereum.block_number()?;
        let deadline_block = self.get_deadline_block(current_block);
        let gas_price = self.gas_adjuster.get_gas_price(&self.ethereum, None)?;

        let (new_op, signed_tx) = self.db.transaction(|| {
            // First, we should store the operation in the database and obtain the assigned
            // operation ID and nonce. Without them we won't be able to sign the transaction.
            let assigned_data = self.db.save_new_eth_tx(
                tx.op_type,
                tx.operation.clone(),
                deadline_block as i64,
                gas_price,
                tx.raw.clone(),
            )?;

            let mut new_op = ETHOperation {
                id: assigned_data.id,
                op_type: tx.op_type,
                op: tx.operation,
                nonce: assigned_data.nonce,
                last_deadline_block: deadline_block,
                last_used_gas_price: gas_price,
                used_tx_hashes: vec![], // No hash yet, will be added below.
                encoded_tx_data: tx.raw,
                confirmed: false,
                final_hash: None,
            };

            // Sign the transaction.
            let signed_tx = Self::sign_new_tx(&self.ethereum, &new_op)?;

            // With signed tx, update the hash in the operation entry and in the db.
            new_op.used_tx_hashes.push(signed_tx.hash);
            self.db.add_hash_entry(new_op.id, &signed_tx.hash)?;

            Ok((new_op, signed_tx))
        })?;

        // We should store the operation as `ongoing` **before** sending it as well,
        // so if sending will fail, we won't forget about it.
        self.ongoing_ops.push_back(new_op.clone());

        // After storing all the tx data in the database, we can finally send the tx.
        info!(
            "Sending new tx: [ETH Operation <id: {}, type: {:?}>. ETH tx: {}. ZKSync operation: {}]",
            new_op.id, new_op.op_type, self.eth_tx_description(&signed_tx), self.zksync_operation_description(&new_op),
        );
        self.ethereum.send_tx(&signed_tx).unwrap_or_else(|e| {
            // Sending tx error is not critical: this will result in transaction being considered stuck,
            // and resent. We can't do anything about this failure either, since it's most probably is not
            // related to the node logic, so we just log this error and pretend to have this operation
            // processed.
            warn!("Error while sending the operation: {}", e);
        });

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

    /// Helper method to obtain the string representation of the ZK Sync operation.
    /// Intended to be used for log entries.
    fn zksync_operation_description(&self, operation: &ETHOperation) -> String {
        if let Some(op) = &operation.op {
            format!(
                "<id {}; action: {}; block: {}>",
                op.id.expect("ID must be set"),
                op.action.to_string(),
                op.block.block_number
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
    fn perform_commitment_step(
        &mut self,
        op: &mut ETHOperation,
    ) -> Result<OperationCommitment, failure::Error> {
        assert!(
            !op.used_tx_hashes.is_empty(),
            "OperationETHState should have at least one transaction"
        );

        let current_block = self.ethereum.block_number()?;

        // Check statuses of existing transactions.
        // Go through every transaction in a loop. We will exit this method early
        // if there will be discovered a pending or successfully committed transaction.
        for (idx, tx_hash) in op.used_tx_hashes.iter().enumerate() {
            let mode = if idx == op.used_tx_hashes.len() - 1 {
                TxCheckMode::Latest
            } else {
                TxCheckMode::Old
            };

            match self.check_transaction_state(mode, op, tx_hash, current_block)? {
                TxCheckOutcome::Pending => {
                    // Transaction is pending, nothing to do yet.
                    return Ok(OperationCommitment::Pending);
                }
                TxCheckOutcome::Committed => {
                    info!(
                        "Confirmed: [ETH Operation <id: {}, type: {:?}>. Tx hash: <{:#x}>. ZKSync operation: {}]",
                        op.id, op.op_type, tx_hash, self.zksync_operation_description(op),
                    );
                    self.db.confirm_operation(tx_hash)?;
                    return Ok(OperationCommitment::Committed);
                }
                TxCheckOutcome::Stuck => {
                    // We do nothing for a stuck transaction. If this will be
                    // the last entry of the list, a new tx will be sent.
                }
                TxCheckOutcome::Failed(receipt) => {
                    warn!(
                        "ETH transaction failed: tx: {:#x}, op_type: {:?}, op: {:?}; tx_receipt: {:#?} ",
                        tx_hash,
                        op.op_type,
                        op.op,
                        receipt,
                    );
                    // Process the failure according to the chosen policy.
                    self.failure_handler(&receipt);
                }
            }
        }

        // Reaching this point will mean that the latest transaction got stuck.
        // We should create another tx based on it, and send it.
        let deadline_block = self.get_deadline_block(current_block);
        // Raw tx contents are the same for every transaction, so we just
        // create a new one from the old one with updated parameters.
        let new_tx = self.create_supplement_tx(deadline_block, op)?;
        // New transaction should be persisted in the DB *before* sending it.
        self.db.transaction(|| {
            self.db
                .update_eth_tx(op.id, deadline_block as i64, new_tx.gas_price)?;
            self.db.add_hash_entry(op.id, &new_tx.hash)?;
            Ok(())
        })?;

        info!(
            "Stuck tx processing: sending tx for op, eth_op_id: {}; ETH tx: {}",
            op.id,
            self.eth_tx_description(&new_tx),
        );
        self.ethereum.send_tx(&new_tx)?;

        Ok(OperationCommitment::Pending)
    }

    /// Handles a transaction execution failure by reporting the issue to the log
    /// and terminating the node.
    fn failure_handler(&self, receipt: &TransactionReceipt) -> ! {
        error!(
            "Ethereum transaction unexpectedly failed. Receipt: {:#?}",
            receipt
        );
        panic!("Cannot operate after unexpected TX failure");
    }

    /// Helper method encapsulating the logic of determining the next deadline block.
    fn get_deadline_block(&self, current_block: u64) -> u64 {
        current_block + self.options.expected_wait_time_block
    }

    /// Looks up for a transaction state on the Ethereum chain
    /// and reduces it to the simpler `TxCheckOutcome` report.
    fn check_transaction_state(
        &self,
        mode: TxCheckMode,
        op: &ETHOperation,
        tx_hash: &H256,
        current_block: u64,
    ) -> Result<TxCheckOutcome, failure::Error> {
        let status = self.ethereum.get_tx_status(tx_hash)?;

        let outcome = match status {
            // Successful execution.
            Some(status) if status.success => {
                // Check if transaction has enough confirmations.
                if status.confirmations >= self.options.wait_confirmations {
                    TxCheckOutcome::Committed
                } else {
                    TxCheckOutcome::Pending
                }
            }
            // Non-successful execution.
            Some(status) => {
                // Transaction failed, report the failure with details.

                // TODO check confirmations for fail
                assert!(
                    status.receipt.is_some(),
                    "Receipt should exist for a failed transaction"
                );
                TxCheckOutcome::Failed(Box::new(status.receipt.unwrap()))
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
    fn sign_new_tx(ethereum: &ETH, op: &ETHOperation) -> Result<SignedCallResult, failure::Error> {
        let tx_options = {
            let mut options = Options::default();
            options.nonce = Some(op.nonce);
            options.gas_price = Some(op.last_used_gas_price);
            options
        };

        let signed_tx = ethereum.sign_prepared_tx(op.encoded_tx_data.clone(), tx_options)?;

        Ok(signed_tx)
    }

    /// Creates a new transaction for the existing Ethereum operation.
    /// This method is used to create supplement transactions instead of the stuck one.
    fn create_supplement_tx(
        &mut self,
        deadline_block: u64,
        stuck_tx: &mut ETHOperation,
    ) -> Result<SignedCallResult, failure::Error> {
        let tx_options = self.tx_options_from_stuck_tx(stuck_tx)?;

        let raw_tx = stuck_tx.encoded_tx_data.clone();
        let signed_tx = self.ethereum.sign_prepared_tx(raw_tx, tx_options)?;

        stuck_tx.last_deadline_block = deadline_block;
        stuck_tx.last_used_gas_price = signed_tx.gas_price;
        stuck_tx.used_tx_hashes.push(signed_tx.hash.clone());

        Ok(signed_tx)
    }

    /// Creates a new tx options from a stuck transaction, with updated gas amount
    /// and nonce.
    fn tx_options_from_stuck_tx(
        &mut self,
        stuck_tx: &ETHOperation,
    ) -> Result<Options, failure::Error> {
        let old_tx_gas_price = stuck_tx.last_used_gas_price;

        let new_gas_price = self
            .gas_adjuster
            .get_gas_price(&self.ethereum, Some(old_tx_gas_price))?;
        let nonce = stuck_tx.nonce;

        info!(
            "Replacing tx: hash: {:#x}, old_gas: {}, new_gas: {}, used nonce: {}",
            stuck_tx.used_tx_hashes.last().unwrap(),
            old_tx_gas_price,
            new_gas_price,
            nonce
        );

        Ok(Options::with(move |opt| {
            opt.gas_price = Some(new_gas_price);
            opt.nonce = Some(nonce);
        }))
    }

    /// Encodes the operation data to the Ethereum tx payload (not signs it!).
    fn operation_to_raw_tx(&self, op: &Operation) -> Vec<u8> {
        match &op.action {
            Action::Commit => {
                let root = op.block.get_eth_encoded_root();

                let public_data = op.block.get_eth_public_data();
                debug!(
                    "public_data for block_number {}: {}",
                    op.block.block_number,
                    hex::encode(&public_data)
                );

                let witness_data = op.block.get_eth_witness_data();
                debug!(
                    "witness_data for block {}: {}, {:?}",
                    op.block.block_number,
                    hex::encode(&witness_data.0),
                    &witness_data.1
                );

                // function commitBlock(uint32 _blockNumber, uint24 _feeAccount, bytes32 _newRoot, bytes calldata _publicData)
                self.ethereum.encode_tx_data(
                    "commitBlock",
                    (
                        u64::from(op.block.block_number),
                        u64::from(op.block.fee_account),
                        root,
                        public_data,
                        witness_data.0,
                        witness_data.1,
                    ),
                )
            }
            Action::Verify { proof } => {
                // function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof) external {
                let block_number = op.block.block_number;
                self.ethereum.encode_tx_data(
                    "verifyBlock",
                    (u64::from(block_number), proof.proof.clone()),
                )
            }
        }
    }

    /// Encodes the ZK Sync operation to the tx payload and adds it to the queue.
    fn add_operation_to_queue(&mut self, op: Operation) {
        let raw_tx = self.operation_to_raw_tx(&op);

        match &op.action {
            Action::Commit => {
                self.tx_queue.add_commit_operation(TxData::from_operation(
                    OperationType::Commit,
                    op,
                    raw_tx,
                ));
            }
            Action::Verify { .. } => {
                let block_number = op.block.block_number;

                self.tx_queue.add_verify_operation(
                    block_number as usize,
                    TxData::from_operation(OperationType::Verify, op, raw_tx),
                );
            }
        }
    }

    /// The same as `add_operation_to_queue`, but for the withdraw operation.
    fn add_complete_withdrawals_to_queue(&mut self) {
        // function completeWithdrawals(uint32 _n) external {
        let raw_tx = self.ethereum.encode_tx_data(
            "completeWithdrawals",
            config::MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL,
        );

        info!("Adding withdraw operation to queue");

        self.tx_queue
            .add_withdraw_operation(TxData::from_raw(OperationType::Withdraw, raw_tx));
    }
}

pub fn start_eth_sender(
    pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    op_notify_sender: mpsc::Sender<Operation>,
    send_requst_receiver: mpsc::Receiver<Operation>,
    config_options: ConfigurationOptions,
) {
    std::thread::Builder::new()
        .name("eth_sender".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);

            let ethereum =
                EthereumHttpClient::new(&config_options).expect("Ethereum client creation failed");

            let db = Database::new(pool);

            let eth_sender_options = EthSenderOptions::from_env();

            let mut runtime = Runtime::new().expect("eth-sender-runtime");
            let eth_sender = ETHSender::new(
                eth_sender_options,
                db,
                ethereum,
                send_requst_receiver,
                op_notify_sender,
            );
            runtime.block_on(eth_sender.run());
        })
        .expect("Eth sender thread");
}
