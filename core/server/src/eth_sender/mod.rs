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
use eth_client::SignedCallResult;
use models::config_options::{ConfigurationOptions, ThreadPanicNotify};
use models::node::config;
use models::{Action, ActionType, Operation};
use storage::ConnectionPool;
// Local uses
use self::database::{Database, DatabaseAccess};
use self::ethereum_interface::{EthereumHttpClient, EthereumInterface};
use self::transactions::*;

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
    /// Unconfirmed operations queue.
    unconfirmed_ops: VecDeque<OperationETHState>,
    /// Connection to the database.
    db: DB,
    /// Ethereum intermediator.
    ethereum: ETH,
    /// Channel for receiving operations to commit.
    rx_for_eth: mpsc::Receiver<Operation>,
    /// Channel to notify about committed operations.
    op_notify: mpsc::Sender<Operation>,
}

impl<ETH: EthereumInterface, DB: DatabaseAccess> ETHSender<ETH, DB> {
    pub fn new(
        db: DB,
        ethereum: ETH,
        rx_for_eth: mpsc::Receiver<Operation>,
        op_notify: mpsc::Sender<Operation>,
    ) -> Self {
        let unconfirmed_ops = db
            .restore_state()
            .expect("Failed loading unconfirmed operations from the storage");

        Self {
            ethereum,
            unconfirmed_ops,
            db,
            rx_for_eth,
            op_notify,
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
            self.proceed_next_operation();
        }
    }

    /// Obtains all the available operations to commit through the channel
    /// and stores them within self for further processing.
    fn retrieve_operations(&mut self) {
        while let Ok(Some(operation)) = self.rx_for_eth.try_next() {
            self.unconfirmed_ops.push_back(OperationETHState {
                operation,
                txs: Vec::new(),
            });
        }
    }

    fn proceed_next_operation(&mut self) {
        // Commit the next operation (if any).
        if let Some(current_op) = self.unconfirmed_ops.pop_front() {
            self.try_commit(current_op);
        }
    }

    /// Attempts to commit the provided operation to the Ethereum blockchain.
    ///
    /// The strategy is the following:
    /// - First we check the transactions associated with the operation.
    ///   If there are none, we create and send one, storing it locally. No more
    ///   processing at this step; we need to wait.
    ///   If there are some transactions, we check their state. If one of them
    ///   is committed and has enough approvals, we're all good.
    ///   Otherwise, we check if the last pending transaction is "stuck", meaning
    ///   that it is not being included in a block for a decent amount of time. If
    ///   so, we create a new transaction (with increased gas) and send it.
    /// - If there was no confirmation of a transaction in a previous step, we return
    ///   the operation to the beginning of the unprocessed operations queue. We will
    ///   check it again after some time.
    /// - If transaction was confirmed, there may be two possible outcomes:
    ///   1. Transaction is executed successfully. Desirable outcome, in which we
    ///      consider the commitment completed and notify about it through the channel.
    ///   2. Transaction erred. This should never happen, but if so, such an incident is
    ///      reported according to the chosen failure report policy.
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

                if operation.operation.action.get_type() == ActionType::VERIFY {
                    // We notify about verify only when commit is confirmed on the Ethereum.
                    self.op_notify
                        .try_send(operation.operation)
                        .map_err(|e| warn!("Failed notify about verify op confirmation: {}", e))
                        .unwrap_or_default();

                    // Complete pending withdrawals after each verify.
                    self.call_complete_withdrawals()
                        .map_err(|e| {
                            warn!("Error: {}", e);
                        })
                        .unwrap_or_default();
                }
            }
            OperationCommitment::Pending => {
                // Retry the operation again the next time.
                self.unconfirmed_ops.push_front(operation);
            }
        }
    }

    /// Checks the state of the operation commitment, choosing the necessary action to perform.
    /// Initially this method sends the first transaction to the Ethereum blockchain.
    /// Within next invocations for the same operation, state of sent transaction is checked.
    /// If transaction(s) will be pending yet, this method won't do anything.
    /// If one of transactions will be successfully confirmed on chain, the commitment will be considered
    /// finished.
    /// In case of stuck transaction, another transaction with increased gas limit will be sent.
    /// In case of transaction failure, it will be reported and processed according to failure handling
    /// policy.
    fn perform_commitment_step(
        &mut self,
        op: &mut OperationETHState,
    ) -> Result<OperationCommitment, failure::Error> {
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

        // Reaching this point will mean that either there were no transactions to process,
        // or the latest transaction got stuck.
        // Either way we should create a new transaction (the approach is the same,
        // `sign_new_tx` will adapt its logic based on `last_stuck_tx`).
        let deadline_block = self.get_deadline_block(current_block);
        let new_tx = self.sign_new_tx(&op.operation, deadline_block, last_stuck_tx)?;
        // New transaction should be persisted in the DB *before* sending it.
        self.db.save_unconfirmed_operation(&new_tx)?;

        op.txs.push(new_tx.clone());
        info!(
            "Sending tx for op, op_id: {} tx_hash: {:#x}",
            new_tx.op_id, new_tx.signed_tx.hash
        );
        self.ethereum.send_tx(&new_tx.signed_tx)?;

        Ok(OperationCommitment::Pending)
    }

    /// Handles a transaction execution failure by reporting the issue to the log
    /// and terminating the node.
    fn failure_handler(&self, receipt: &TransactionReceipt) -> ! {
        info!(
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
    fn sign_new_tx(
        &self,
        op: &Operation,
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

        let signed_tx = self.sign_operation_tx(op, tx_options)?;
        Ok(TransactionETHState {
            op_id: op.id.unwrap(),
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

    /// Creates a signed transaction according to the operation action.
    fn sign_operation_tx(
        &self,
        op: &Operation,
        tx_options: Options,
    ) -> Result<SignedCallResult, failure::Error> {
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
                self.ethereum.sign_call_tx(
                    "commitBlock",
                    (
                        u64::from(op.block.block_number),
                        u64::from(op.block.fee_account),
                        root,
                        public_data,
                        witness_data.0,
                        witness_data.1,
                    ),
                    tx_options,
                )
            }
            Action::Verify { proof } => {
                // function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof) external {
                self.ethereum.sign_call_tx(
                    "verifyBlock",
                    (u64::from(op.block.block_number), *proof.clone()),
                    tx_options,
                )
            }
        }
    }

    fn call_complete_withdrawals(&self) -> Result<(), failure::Error> {
        // function completeWithdrawals(uint32 _n) external {
        let mut options = Options::default();
        let nonce = self.db.next_nonce()?;
        options.nonce = Some(nonce.into());

        let tx = self
            .ethereum
            .sign_call_tx(
                "completeWithdrawals",
                config::MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL,
                options,
            )
            .map_err(|e| failure::format_err!("completeWithdrawals: {}", e))?;
        info!("Sending completeWithdrawals tx with hash: {:#?}", tx.hash);
        self.ethereum.send_tx(&tx)
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
