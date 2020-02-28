//! Mocking utilities for tests.

// Built-in deps
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
// External uses
use futures::channel::mpsc;
use web3::contract::{tokens::Tokenize, Options};
use web3::types::{H256, U256};
// Workspace uses
use eth_client::SignedCallResult;
use models::{Action, Operation};
// Local uses
use super::ETHSender;
use crate::eth_sender::database::DatabaseAccess;
use crate::eth_sender::ethereum_interface::EthereumInterface;
use crate::eth_sender::transactions::{ExecutedTxStatus, OperationETHState, TransactionETHState};

const CHANNEL_CAPACITY: usize = 16;

/// Mock database is capable of recording all the incoming requests for the further analysis.
#[derive(Debug, Default)]
pub(super) struct MockDatabase {
    restore_state: VecDeque<OperationETHState>,
    unconfirmed_operations: RefCell<HashMap<H256, TransactionETHState>>,
    confirmed_operations: RefCell<HashMap<H256, TransactionETHState>>,
}

impl MockDatabase {
    /// Creates a database with emulation of previously stored uncommitted requests.
    pub fn with_restorable_state(
        restore_state: impl IntoIterator<Item = OperationETHState>,
    ) -> Self {
        Self {
            restore_state: restore_state.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Ensures that the provided transaction is stored in the database and not confirmed yet.
    pub fn assert_stored(&self, tx: &TransactionETHState) {
        assert_eq!(
            self.unconfirmed_operations.borrow().get(&tx.signed_tx.hash),
            Some(tx)
        );

        assert!(self
            .confirmed_operations
            .borrow()
            .get(&tx.signed_tx.hash)
            .is_none());
    }

    /// Ensures that the provided transaction is not stored in the database.
    pub fn assert_not_stored(&self, tx: &TransactionETHState) {
        assert!(self
            .confirmed_operations
            .borrow()
            .get(&tx.signed_tx.hash)
            .is_none());

        assert!(self
            .unconfirmed_operations
            .borrow()
            .get(&tx.signed_tx.hash)
            .is_none());
    }

    /// Ensures that the provided transaction is stored as confirmed.
    pub fn assert_confirmed(&self, tx: &TransactionETHState) {
        assert_eq!(
            self.confirmed_operations.borrow().get(&tx.signed_tx.hash),
            Some(tx)
        );

        assert!(self
            .unconfirmed_operations
            .borrow()
            .get(&tx.signed_tx.hash)
            .is_none());
    }
}

impl DatabaseAccess for MockDatabase {
    fn restore_state(&self) -> Result<VecDeque<OperationETHState>, failure::Error> {
        Ok(self.restore_state.clone())
    }

    fn save_unconfirmed_operation(&self, tx: &TransactionETHState) -> Result<(), failure::Error> {
        self.unconfirmed_operations
            .borrow_mut()
            .insert(tx.signed_tx.hash, tx.clone());

        Ok(())
    }

    fn confirm_operation(&self, hash: &H256) -> Result<(), failure::Error> {
        let mut unconfirmed_operations = self.unconfirmed_operations.borrow_mut();
        assert!(
            unconfirmed_operations.contains_key(hash),
            "Request to confirm operation that was not stored"
        );

        let operation = unconfirmed_operations.remove(hash).unwrap();
        self.confirmed_operations
            .borrow_mut()
            .insert(*hash, operation);

        Ok(())
    }
}

/// Mock Ethereum client is capable of recording all the incoming requests for the further analysis.
#[derive(Debug)]
pub(super) struct MockEthereum {
    pub block_number: u64,
    pub nonce: U256,
    pub gas_price: U256,
    pub tx_statuses: RefCell<HashMap<H256, ExecutedTxStatus>>,
    pub sent_txs: RefCell<HashMap<H256, SignedCallResult>>,
}

impl Default for MockEthereum {
    fn default() -> Self {
        Self {
            block_number: 1,
            nonce: Default::default(),
            gas_price: 100.into(),
            tx_statuses: Default::default(),
            sent_txs: Default::default(),
        }
    }
}

impl MockEthereum {
    /// A fake `sha256` hasher, which calculates an `std::hash` instead.
    /// This is done for simplicity and it's also much faster.
    pub fn fake_sha256(data: &[u8]) -> H256 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        hasher.write(data);

        let result = hasher.finish();

        H256::from_low_u64_ne(result)
    }

    /// Checks that there was a request to send the provided transaction.
    pub fn assert_sent(&self, tx: &TransactionETHState) {
        assert_eq!(
            self.sent_txs.borrow().get(&tx.signed_tx.hash),
            Some(&tx.signed_tx)
        );
    }

    /// Checks that there was a request to send a transaction with the provided hash.
    pub fn assert_sent_by_hash(&self, hash: &H256) {
        assert!(
            self.sent_txs.borrow().get(hash).is_some(),
            format!("Transaction with hash {:?} was not sent", hash),
        );
    }

    /// Adds an response for the sent transaction for `ETHSender` to receive.
    pub fn add_execution(&mut self, tx: &TransactionETHState, status: &ExecutedTxStatus) {
        self.tx_statuses
            .borrow_mut()
            .insert(tx.signed_tx.hash, status.clone());
    }

    /// Increments the blocks by a provided `confirmations` and marks the sent transaction
    /// as a success.
    pub fn add_successfull_execution(&mut self, tx: &TransactionETHState, confirmations: u64) {
        self.block_number += confirmations;
        self.nonce += 1.into();

        let status = ExecutedTxStatus {
            confirmations,
            success: true,
            receipt: None,
        };
        self.tx_statuses
            .borrow_mut()
            .insert(tx.signed_tx.hash, status);
    }

    /// Same as `add_successfull_execution`, but marks the transaction as a failure.
    pub fn add_failed_execution(&mut self, tx: &TransactionETHState, confirmations: u64) {
        self.block_number += confirmations;
        self.nonce += 1.into();

        let status = ExecutedTxStatus {
            confirmations,
            success: false,
            receipt: Some(Default::default()),
        };
        self.tx_statuses
            .borrow_mut()
            .insert(tx.signed_tx.hash, status);
    }

    /// Replicates the `ETHCLient::sign_operation_tx` method for testing.
    pub fn create_signed_tx_replica(&self, op: &Operation) -> SignedCallResult {
        match &op.action {
            Action::Commit => {
                let root = op.block.get_eth_encoded_root();
                let public_data = op.block.get_eth_public_data();
                let witness_data = op.block.get_eth_witness_data();
                self.sign_call_tx(
                    "commitBlock",
                    (
                        u64::from(op.block.block_number),
                        u64::from(op.block.fee_account),
                        root,
                        public_data,
                        witness_data.0,
                        witness_data.1,
                    ),
                    Options::default(),
                )
                .unwrap()
            }
            Action::Verify { proof } => self
                .sign_call_tx(
                    "verifyBlock",
                    (u64::from(op.block.block_number), *proof.clone()),
                    Options::default(),
                )
                .unwrap(),
        }
    }
}

impl EthereumInterface for MockEthereum {
    fn get_tx_status(&self, hash: &H256) -> Result<Option<ExecutedTxStatus>, failure::Error> {
        Ok(self.tx_statuses.borrow().get(hash).cloned())
    }

    fn block_number(&self) -> Result<u64, failure::Error> {
        Ok(self.block_number)
    }

    fn gas_price(&self) -> Result<U256, failure::Error> {
        Ok(self.gas_price)
    }

    fn current_nonce(&self) -> Result<U256, failure::Error> {
        Ok(self.nonce)
    }

    fn send_tx(&self, signed_tx: &SignedCallResult) -> Result<(), failure::Error> {
        self.sent_txs
            .borrow_mut()
            .insert(signed_tx.hash, signed_tx.clone());

        Ok(())
    }

    fn sign_call_tx<P: Tokenize>(
        &self,
        _func: &str,
        params: P,
        options: Options,
    ) -> Result<SignedCallResult, failure::Error> {
        let raw_tx = ethabi::encode(params.into_tokens().as_ref());
        let hash = Self::fake_sha256(raw_tx.as_ref()); // Okay for test purposes.

        Ok(SignedCallResult {
            raw_tx,
            gas_price: options.gas_price.unwrap_or(self.gas_price),
            nonce: options.nonce.unwrap_or(self.nonce),
            hash,
        })
    }
}

/// Creates a default `ETHSender` with mock Ethereum connection/database and no operations in DB.
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(super) fn default_eth_sender() -> (
    ETHSender<MockEthereum, MockDatabase>,
    mpsc::Sender<Operation>,
    mpsc::Receiver<Operation>,
) {
    restored_eth_sender(Vec::new())
}

/// Creates an `ETHSender` with mock Ethereum connection/database and restores its state "from DB".
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(super) fn restored_eth_sender(
    restore_state: impl IntoIterator<Item = OperationETHState>,
) -> (
    ETHSender<MockEthereum, MockDatabase>,
    mpsc::Sender<Operation>,
    mpsc::Receiver<Operation>,
) {
    let ethereum = MockEthereum::default();
    let db = MockDatabase::with_restorable_state(restore_state);

    let (operation_sender, operation_receiver) = mpsc::channel(CHANNEL_CAPACITY);
    let (notify_sender, notify_receiver) = mpsc::channel(CHANNEL_CAPACITY);

    (
        ETHSender::new(db, ethereum, operation_receiver, notify_sender),
        operation_sender,
        notify_receiver,
    )
}
