//! `eth_sender` module is capable of synchronizing the operations
//! occurring in `ZKSync` with the Ethereum blockchain by creating
//! transactions from the operations, sending them and ensuring that
//! every transaction is executed successfully and confirmed.

// Built-in deps
use std::collections::VecDeque;
use std::time::Duration;
// External uses
use futures::channel::mpsc;
use tokio::runtime::Runtime;
use tokio::time;
use web3::contract::Options;
use web3::types::{TransactionReceipt, U256};
// Workspace uses
use models::config_options::{ConfigurationOptions, ThreadPanicNotify};
use models::node::config;
use models::{Action, ActionType, Operation};
use storage::ConnectionPool;
// Local uses
use self::database::{Database, DatabaseAccess};
use self::ethereum_interface::{EthereumHttpClient, EthereumInterface};
use self::transactions::*;
use self::tx_queue::{TxData, TxQueue, TxQueueBuilder};

mod database;
mod ethereum_interface;
mod transactions;
mod tx_queue;

#[cfg(test)]
mod tests;

const EXPECTED_WAIT_TIME_BLOCKS: u64 = 30;
const TX_POLL_PERIOD: Duration = Duration::from_secs(5);
const WAIT_CONFIRMATIONS: u64 = 1;

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
/// are committed in FIFO order, meaning that until the older operation is committed
/// and has enough confirmations, no other operations will be committed.
///
/// # Transaction sending policy
///
/// The goal is to handle stuck transactions.
///
/// When we try to commit operation to ETH, we select nonce, gas price, sign
/// transaction and watch for its confirmations.
///
/// If transaction is not confirmed for a while, we increase the gas price and do the same, but we
/// keep the list of all sent transactions for one particular operations, since we can't be
/// sure which one will be committed; thus we have to track all of them.
///
/// Note: make sure to save signed tx to db before sending it to ETH, this way we can be sure
/// that state is always recoverable.
///
/// # Failure policy
///
/// By default, `ETHSender` expects no transactions to fail, and thus upon a failure it will
/// report the incident to the log and then panic to prevent continue working in a probably
/// erroneous conditions. Failure handling policy is determined by a corresponding callback,
/// which can be changed if needed.
struct ETHSender<ETH: EthereumInterface, DB: DatabaseAccess> {
    /// Ongoing operations queue.
    ongoing_ops: VecDeque<OperationETHState>,
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
}

impl<ETH: EthereumInterface, DB: DatabaseAccess> ETHSender<ETH, DB> {
    pub fn new(
        db: DB,
        ethereum: ETH,
        rx_for_eth: mpsc::Receiver<Operation>,
        op_notify: mpsc::Sender<Operation>,
    ) -> Self {
        const MAX_TXS_IN_FLIGHT: usize = 1; // TODO: Should be configurable.

        let ongoing_ops = db
            .restore_state()
            .expect("Failed loading unconfirmed operations from the storage");

        let stats = db
            .load_stats()
            .expect("Failed loading ETH operations stats");

        let tx_queue = TxQueueBuilder::new(MAX_TXS_IN_FLIGHT)
            .with_sent_pending_txs(ongoing_ops.len())
            .with_commit_operations_count(stats.commit_ops)
            .with_verify_operations_count(stats.verify_ops)
            .with_withdraw_operations_count(stats.withdraw_ops)
            .build();

        Self {
            ethereum,
            ongoing_ops,
            db,
            rx_for_eth,
            op_notify,
            tx_queue,
        }
    }

    /// Main routine of `ETHSender`.
    pub async fn run(mut self) {
        let mut timer = time::interval(TX_POLL_PERIOD);

        loop {
            // Update the incoming operations.
            self.retrieve_operations();
            timer.tick().await;

            // ...and proceed them.
            self.proceed_next_operations();
        }
    }

    fn retrieve_operations(&mut self) {
        while let Ok(Some(operation)) = self.rx_for_eth.try_next() {
            self.add_operation_to_queue(operation);
        }
    }

    fn proceed_next_operations(&mut self) {
        while let Some(tx) = self.tx_queue.pop_front() {
            self.initialize_operation(tx).unwrap_or_else(|e| {
                warn!("Error while trying to complete uncommitted op: {}", e);
            });
        }

        // Commit the next operation (if any).
        // TODO: should not be `if let`, but rather `while let`.
        if let Some(current_op) = self.ongoing_ops.pop_front() {
            self.try_commit(current_op);
        }
    }

    fn initialize_operation(&mut self, tx: TxData) -> Result<(), failure::Error> {
        let current_block = self.ethereum.block_number()?;
        let deadline_block = self.get_deadline_block(current_block);

        if let Some(operation) = tx.operation {
            let mut eth_op = OperationETHState {
                operation,
                txs: Vec::new(),
            };

            let new_tx =
                self.sign_raw_tx(eth_op.operation.id.unwrap(), tx.raw, deadline_block, None)?;

            self.db.save_unconfirmed_operation(&new_tx)?;
            self.db.report_created_operation(
                self.operation_type_for_action(&eth_op.operation.action),
            )?;

            eth_op.txs.push(new_tx.clone());
            info!(
                "Sending tx for op, op_id: {} tx_hash: {:#x}, nonce: {}",
                new_tx.op_id, new_tx.signed_tx.hash, new_tx.signed_tx.nonce,
            );
            self.ethereum.send_tx(&new_tx.signed_tx)?;

            self.ongoing_ops.push_back(eth_op);
        } else {
            let mut options = Options::default();
            let nonce = self.db.next_nonce()?;
            options.nonce = Some(nonce.into());

            let tx = self
                .ethereum
                .sign_prepared_tx(tx.raw, options)
                .map_err(|e| failure::format_err!("Failed to sign a prepared tx: {}", e))?;

            // TODO: Operations w/o `Operation` field (e.g. withdrawals) should be stored to the DB as well.
            // self.db
            //     .report_created_operation(self.operation_type_for_action(&op.operation.action))?;

            info!("Sending tx with hash: {:#?}", tx.hash);
            self.ethereum.send_tx(&tx)?;
        }

        Ok(())
    }

    fn try_commit(&mut self, mut operation: OperationETHState) {
        // Check the transactions associated with the operation, and send a new one if required.

        // We perform a commitment step here. In case of error, we suppose that this is some
        // network issue which won't appear the next time, so we report the situation to the
        // log and consider the operation pending (meaning that we won't process it on this
        // step, but will try to do so on the next one).
        let result = self
            .perform_commitment_step(&mut operation)
            .map_err(|e| {
                warn!("Error while trying to complete uncommitted op: {}", e);
            })
            .unwrap_or(OperationCommitment::Pending);

        // Check if we've completed the commitment.
        match result {
            OperationCommitment::Committed => {
                info!(
                    "Operation {}, {}  block: {}, confirmed on ETH",
                    operation.operation.id.unwrap(),
                    operation.operation.action.to_string(),
                    operation.operation.block.block_number,
                );

                // Free a slot for the next tx in the queue.
                self.tx_queue.report_commitment();

                if operation.operation.action.get_type() == ActionType::VERIFY {
                    // We notify about verify only when commit is confirmed on the Ethereum.
                    self.op_notify
                        .try_send(operation.operation)
                        .map_err(|e| warn!("Failed notify about verify op confirmation: {}", e))
                        .unwrap_or_default();

                    // Complete pending withdrawals after each verify.
                    self.add_complete_withdrawals_to_queue();
                }
            }
            OperationCommitment::Pending => {
                // Retry the operation again the next time.
                self.ongoing_ops.push_front(operation);
            }
        }
    }

    fn perform_commitment_step(
        &mut self,
        op: &mut OperationETHState,
    ) -> Result<OperationCommitment, failure::Error> {
        assert!(
            !op.txs.is_empty(),
            "OperationETHState should have at least one transaction"
        );

        let current_block = self.ethereum.block_number()?;

        // Check statuses of existing transactions.
        let mut last_stuck_tx: Option<&TransactionETHState> = None;

        // Go through every transaction in a loop. We will exit this method early
        // if there will be discovered a pending or successfully committed transaction.
        for tx in &op.txs {
            match self.check_transaction_state(tx, current_block)? {
                TxCheckOutcome::Pending => {
                    // Transaction is pending, nothing to do yet.
                    return Ok(OperationCommitment::Pending);
                }
                TxCheckOutcome::Committed => {
                    info!(
                        "Operation {}, {}  block: {}, committed, tx: {:#x}",
                        op.operation.id.unwrap(),
                        op.operation.action.to_string(),
                        op.operation.block.block_number,
                        tx.signed_tx.hash,
                    );
                    self.db.confirm_operation(&tx.signed_tx.hash)?;
                    return Ok(OperationCommitment::Committed);
                }
                TxCheckOutcome::Stuck => {
                    // Update the last stuck transaction. If we won't exit the loop early,
                    // it will be used to create a new transaction with higher gas limit.
                    last_stuck_tx = Some(tx);
                }
                TxCheckOutcome::Failed(receipt) => {
                    warn!(
                        "ETH transaction failed: tx: {:#x}, operation_id: {}; tx_receipt: {:#?} ",
                        tx.signed_tx.hash,
                        op.operation.id.unwrap(),
                        receipt,
                    );
                    // Process the failure according to the chosen policy.
                    self.failure_handler(&receipt);
                }
            }
        }

        // Reaching this point will mean that the latest transaction got stuck.
        // We should create another tx based on it, and send it.
        assert!(
            last_stuck_tx.is_some(),
            "Loop didn't exit without a stuck tx"
        );
        let deadline_block = self.get_deadline_block(current_block);
        // Raw tx contents are the same for every transaction, so we just
        // clone them from the first tx.
        let raw_tx = op.txs[0].signed_tx.raw_tx.clone();
        let new_tx = self.sign_raw_tx(
            op.operation.id.unwrap(),
            raw_tx,
            deadline_block,
            last_stuck_tx,
        )?;
        // New transaction should be persisted in the DB *before* sending it.
        self.db.save_unconfirmed_operation(&new_tx)?;
        // Since we're processing the stuck operation, no need to invoke `report_created_operation`.

        op.txs.push(new_tx.clone());
        info!(
            "Stuck tx processing: sending tx for op, op_id: {} tx_hash: {:#x}, nonce: {}",
            new_tx.op_id, new_tx.signed_tx.hash, new_tx.signed_tx.nonce,
        );
        self.ethereum.send_tx(&new_tx.signed_tx)?;

        Ok(OperationCommitment::Pending)
    }

    fn operation_type_for_action(&self, action: &Action) -> OperationType {
        match action {
            Action::Commit => OperationType::Commit,
            Action::Verify { .. } => OperationType::Verify,
        }
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
        current_block + EXPECTED_WAIT_TIME_BLOCKS
    }

    /// Looks up for a transaction state on the Ethereum chain
    /// and reduces it to the simpler `TxCheckOutcome` report.
    fn check_transaction_state(
        &self,
        tx: &TransactionETHState,
        current_block: u64,
    ) -> Result<TxCheckOutcome, failure::Error> {
        let status = self.ethereum.get_tx_status(&tx.signed_tx.hash)?;

        let outcome = match status {
            // Successful execution.
            Some(status) if status.success => {
                // Check if transaction has enough confirmations.
                if status.confirmations >= WAIT_CONFIRMATIONS {
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
            None if tx.is_stuck(current_block) => TxCheckOutcome::Stuck,
            // No status and not stuck yet, thus considered pending.
            None => TxCheckOutcome::Pending,
        };

        Ok(outcome)
    }

    /// Creates a new transaction. If stuck tx is provided, the new transaction will be
    /// and updated version of it; otherwise a brand new transaction will be created.
    fn sign_raw_tx(
        &self,
        op_id: i64,
        raw_tx: Vec<u8>,
        deadline_block: u64,
        stuck_tx: Option<&TransactionETHState>,
    ) -> Result<TransactionETHState, failure::Error> {
        let tx_options = if let Some(stuck_tx) = stuck_tx {
            self.tx_options_from_stuck_tx(stuck_tx)?
        } else {
            let mut options = Options::default();
            let nonce = self.db.next_nonce()?;
            options.nonce = Some(nonce.into());
            options
        };

        let signed_tx = self.ethereum.sign_prepared_tx(raw_tx, tx_options)?;
        Ok(TransactionETHState {
            op_id,
            deadline_block,
            signed_tx,
        })
    }

    // Calculates a new gas amount for the replacement of the stuck tx.
    // Replacement price should be at least 10% higher, we make it 15% higher.
    fn scale_gas(&self, old_tx_gas_price: U256) -> Result<U256, failure::Error> {
        let network_price = self.ethereum.gas_price()?;
        let replacement_price = (old_tx_gas_price * U256::from(115)) / U256::from(100);
        Ok(std::cmp::max(network_price, replacement_price))
    }

    /// Creates a new tx options from a stuck transaction, with updated gas amount
    /// and nonce.
    fn tx_options_from_stuck_tx(
        &self,
        stuck_tx: &TransactionETHState,
    ) -> Result<Options, failure::Error> {
        let old_tx_gas_price =
            U256::from_dec_str(&stuck_tx.signed_tx.gas_price.to_string()).unwrap();

        let new_gas_price = self.scale_gas(old_tx_gas_price)?;
        let nonce = stuck_tx.signed_tx.nonce;

        info!(
            "Replacing tx: hash: {:#x}, old_gas: {}, new_gas: {}, used nonce: {}",
            stuck_tx.signed_tx.hash, old_tx_gas_price, new_gas_price, nonce
        );

        Ok(Options::with(move |opt| {
            opt.gas_price = Some(new_gas_price);
            opt.nonce = Some(nonce);
        }))
    }

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
                self.ethereum
                    .encode_tx_data("verifyBlock", (u64::from(block_number), *proof.clone()))
            }
        }
    }

    fn add_operation_to_queue(&mut self, op: Operation) {
        let raw_tx = self.operation_to_raw_tx(&op);

        match &op.action {
            Action::Commit => {
                self.tx_queue
                    .add_commit_operation(TxData::from_operation(op, raw_tx));
            }
            Action::Verify { proof } => {
                let block_number = op.block.block_number;

                self.tx_queue.add_verify_operation(
                    block_number as usize,
                    TxData::from_operation(op, raw_tx),
                );
            }
        }
    }

    fn add_complete_withdrawals_to_queue(&mut self) {
        // function completeWithdrawals(uint32 _n) external {
        let raw_tx = self.ethereum.encode_tx_data(
            "completeWithdrawals",
            config::MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL,
        );

        self.tx_queue
            .add_withdraw_operation(TxData::from_raw(raw_tx));
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

            let mut runtime = Runtime::new().expect("eth-sender-runtime");
            let eth_sender = ETHSender::new(db, ethereum, send_requst_receiver, op_notify_sender);
            runtime.block_on(eth_sender.run());
        })
        .expect("Eth sender thread");
}
