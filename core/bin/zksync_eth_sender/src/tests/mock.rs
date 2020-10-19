//! Mocking utilities for tests.

// Built-in deps
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
// External uses
use futures::channel::mpsc;
use web3::contract::{tokens::Tokenize, Options};
use zksync_basic_types::{H256, U256};
// Workspace uses
use zksync_eth_client::SignedCallResult;
use zksync_types::{
    config_options::EthSenderOptions,
    ethereum::{ETHOperation, EthOpId, InsertedOperationResponse, OperationType},
    Action, Operation,
};
// Local uses
use super::ETHSender;
// use crate::eth_sender::database::DatabaseAccess;
use crate::eth_sender::ethereum_interface::EthereumInterface;
use crate::eth_sender::transactions::{ETHStats, ExecutedTxStatus};
use crate::eth_sender::ETHSenderRequest;
use crate::utils::current_zksync_info::CurrentZkSyncInfo;

const CHANNEL_CAPACITY: usize = 16;

/// Mock database is capable of recording all the incoming requests for the further analysis.
#[derive(Debug, Default)]
pub(in crate::eth_sender) struct MockDatabase {
    restore_state: VecDeque<ETHOperation>,
    unconfirmed_operations: RefCell<HashMap<i64, ETHOperation>>,
    confirmed_operations: RefCell<HashMap<i64, ETHOperation>>,
    nonce: Cell<i64>,
    gas_price_limit: Cell<U256>,
    pending_op_id: Cell<EthOpId>,
    stats: RefCell<ETHStats>,
}

impl MockDatabase {
    /// Creates a database with emulation of previously stored uncommitted requests.
    pub fn with_restorable_state(
        restore_state: impl IntoIterator<Item = ETHOperation>,
        stats: ETHStats,
    ) -> Self {
        let restore_state: VecDeque<_> = restore_state.into_iter().collect();
        let nonce = restore_state
            .iter()
            .fold(0, |acc, op| acc + op.used_tx_hashes.len());
        let pending_op_id = restore_state.len();

        let unconfirmed_operations: HashMap<i64, ETHOperation> =
            restore_state.iter().map(|op| (op.id, op.clone())).collect();

        let gas_price_limit: u64 =
            zksync_types::config_options::parse_env("ETH_GAS_PRICE_DEFAULT_LIMIT");

        Self {
            restore_state,
            nonce: Cell::new(nonce as i64),
            gas_price_limit: Cell::new(gas_price_limit.into()),
            pending_op_id: Cell::new(pending_op_id as EthOpId),
            stats: RefCell::new(stats),
            unconfirmed_operations: RefCell::new(unconfirmed_operations),
            ..Default::default()
        }
    }

    /// Ensures that the provided transaction is stored in the database and not confirmed yet.
    pub fn assert_stored(&self, tx: &ETHOperation) {
        assert_eq!(self.unconfirmed_operations.borrow().get(&tx.id), Some(tx));

        assert!(self.confirmed_operations.borrow().get(&tx.id).is_none());
    }

    /// Ensures that the provided transaction is stored as confirmed.
    pub fn assert_confirmed(&self, tx: &ETHOperation) {
        assert_eq!(self.confirmed_operations.borrow().get(&tx.id), Some(tx));

        assert!(self.unconfirmed_operations.borrow().get(&tx.id).is_none());
    }

    fn next_nonce(&self) -> Result<i64, anyhow::Error> {
        let old_value = self.nonce.get();
        let new_value = old_value + 1;
        self.nonce.set(new_value);

        Ok(old_value)
    }
}

impl DatabaseAccess for MockDatabase {
    fn restore_state(&self) -> Result<(VecDeque<ETHOperation>, Vec<Operation>), anyhow::Error> {
        Ok((self.restore_state.clone(), Vec::new()))
    }

    fn save_new_eth_tx(
        &self,
        op_type: OperationType,
        op: Option<Operation>,
        deadline_block: i64,
        used_gas_price: U256,
        encoded_tx_data: Vec<u8>,
    ) -> Result<InsertedOperationResponse, anyhow::Error> {
        let id = self.pending_op_id.get();
        let new_id = id + 1;
        self.pending_op_id.set(new_id);

        let nonce = self.next_nonce()?;

        // Store with the assigned ID.
        let state = ETHOperation {
            id,
            op_type,
            op,
            nonce: nonce.into(),
            last_deadline_block: deadline_block as u64,
            last_used_gas_price: used_gas_price,
            used_tx_hashes: vec![],
            encoded_tx_data,
            confirmed: false,
            final_hash: None,
        };

        self.unconfirmed_operations.borrow_mut().insert(id, state);

        let response = InsertedOperationResponse {
            id,
            nonce: nonce.into(),
        };

        Ok(response)
    }

    /// Adds a tx hash entry associated with some Ethereum operation to the database.
    fn add_hash_entry(&self, eth_op_id: i64, hash: &H256) -> Result<(), anyhow::Error> {
        assert!(
            self.unconfirmed_operations
                .borrow()
                .contains_key(&eth_op_id),
            "Attempt to update tx that is not unconfirmed"
        );

        let mut ops = self.unconfirmed_operations.borrow_mut();
        let mut op = ops[&eth_op_id].clone();
        op.used_tx_hashes.push(*hash);
        ops.insert(eth_op_id, op);

        Ok(())
    }

    fn update_eth_tx(
        &self,
        eth_op_id: EthOpId,
        new_deadline_block: i64,
        new_gas_value: U256,
    ) -> Result<(), anyhow::Error> {
        assert!(
            self.unconfirmed_operations
                .borrow()
                .contains_key(&eth_op_id),
            "Attempt to update tx that is not unconfirmed"
        );

        let mut ops = self.unconfirmed_operations.borrow_mut();
        let mut op = ops[&eth_op_id].clone();
        op.last_deadline_block = new_deadline_block as u64;
        op.last_used_gas_price = new_gas_value;
        ops.insert(eth_op_id, op);

        Ok(())
    }

    fn confirm_operation(&self, hash: &H256) -> Result<(), anyhow::Error> {
        let mut unconfirmed_operations = self.unconfirmed_operations.borrow_mut();
        let mut op_idx: Option<i64> = None;
        for operation in unconfirmed_operations.values_mut() {
            if operation.used_tx_hashes.contains(hash) {
                operation.confirmed = true;
                operation.final_hash = Some(*hash);
                op_idx = Some(operation.id);
                break;
            }
        }

        assert!(
            op_idx.is_some(),
            "Request to confirm operation that was not stored"
        );
        let op_idx = op_idx.unwrap();

        let operation = unconfirmed_operations.remove(&op_idx).unwrap();
        self.confirmed_operations
            .borrow_mut()
            .insert(op_idx, operation);

        Ok(())
    }

    fn load_gas_price_limit(&self) -> Result<U256, anyhow::Error> {
        Ok(self.gas_price_limit.get())
    }

    fn update_gas_price_limit(&self, value: U256) -> Result<(), anyhow::Error> {
        self.gas_price_limit.set(value);

        Ok(())
    }

    fn load_stats(&self) -> Result<ETHStats, anyhow::Error> {
        Ok(self.stats.borrow().clone())
    }

    fn transaction<F, T>(&self, f: F) -> Result<T, anyhow::Error>
    where
        F: FnOnce() -> Result<T, anyhow::Error>,
    {
        f()
    }
}

/// Mock Ethereum client is capable of recording all the incoming requests for the further analysis.
#[derive(Debug)]
pub(in crate::eth_sender) struct MockEthereum {
    pub block_number: u64,
    pub gas_price: U256,
    pub tx_statuses: RefCell<HashMap<H256, ExecutedTxStatus>>,
    pub sent_txs: RefCell<HashMap<H256, SignedCallResult>>,
}

impl Default for MockEthereum {
    fn default() -> Self {
        Self {
            block_number: 1,
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
    pub fn assert_sent(&self, hash: &H256) {
        assert!(
            self.sent_txs.borrow().get(hash).is_some(),
            format!("Transaction with hash {:?} was not sent", hash),
        );
    }

    /// Adds an response for the sent transaction for `ETHSender` to receive.
    pub fn add_execution(&mut self, hash: &H256, status: &ExecutedTxStatus) {
        self.tx_statuses.borrow_mut().insert(*hash, status.clone());
    }

    /// Increments the blocks by a provided `confirmations` and marks the sent transaction
    /// as a success.
    pub fn add_successfull_execution(&mut self, tx_hash: H256, confirmations: u64) {
        self.block_number += confirmations;

        let status = ExecutedTxStatus {
            confirmations,
            success: true,
            receipt: None,
        };
        self.tx_statuses.borrow_mut().insert(tx_hash, status);
    }

    /// Same as `add_successfull_execution`, but marks the transaction as a failure.
    pub fn add_failed_execution(&mut self, hash: &H256, confirmations: u64) {
        self.block_number += confirmations;

        let status = ExecutedTxStatus {
            confirmations,
            success: false,
            receipt: Some(Default::default()),
        };
        self.tx_statuses.borrow_mut().insert(*hash, status);
    }
}

impl EthereumInterface for MockEthereum {
    fn get_tx_status(&self, hash: &H256) -> Result<Option<ExecutedTxStatus>, anyhow::Error> {
        Ok(self.tx_statuses.borrow().get(hash).cloned())
    }

    fn block_number(&self) -> Result<u64, anyhow::Error> {
        Ok(self.block_number)
    }

    fn gas_price(&self) -> Result<U256, anyhow::Error> {
        Ok(self.gas_price)
    }

    fn send_tx(&self, signed_tx: &SignedCallResult) -> Result<(), anyhow::Error> {
        self.sent_txs
            .borrow_mut()
            .insert(signed_tx.hash, signed_tx.clone());

        Ok(())
    }

    fn encode_tx_data<P: Tokenize>(&self, _func: &str, params: P) -> Vec<u8> {
        ethabi::encode(params.into_tokens().as_ref())
    }

    fn sign_prepared_tx(
        &self,
        raw_tx: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        let gas_price = options.gas_price.unwrap_or(self.gas_price);
        let nonce = options.nonce.expect("Nonce must be set for every tx");

        // Nonce and gas_price are appended to distinguish the same transactions
        // with different gas by their hash in tests.
        let mut data_for_hash = raw_tx.clone();
        data_for_hash.append(&mut ethabi::encode(gas_price.into_tokens().as_ref()));
        data_for_hash.append(&mut ethabi::encode(nonce.into_tokens().as_ref()));
        let hash = Self::fake_sha256(data_for_hash.as_ref()); // Okay for test purposes.

        Ok(SignedCallResult {
            raw_tx,
            gas_price,
            nonce,
            hash,
        })
    }
}

/// Creates a default `ETHSender` with mock Ethereum connection/database and no operations in DB.
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(in crate::eth_sender) fn default_eth_sender() -> (
    ETHSender<MockEthereum, MockDatabase>,
    mpsc::Sender<ETHSenderRequest>,
    mpsc::Receiver<Operation>,
) {
    build_eth_sender(1, Vec::new(), Default::default())
}

/// Creates an `ETHSender` with mock Ethereum connection/database and no operations in DB
/// which supports multiple transactions in flight.
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(in crate::eth_sender) fn concurrent_eth_sender(
    max_txs_in_flight: u64,
) -> (
    ETHSender<MockEthereum, MockDatabase>,
    mpsc::Sender<ETHSenderRequest>,
    mpsc::Receiver<Operation>,
) {
    build_eth_sender(max_txs_in_flight, Vec::new(), Default::default())
}

/// Creates an `ETHSender` with mock Ethereum connection/database and restores its state "from DB".
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(in crate::eth_sender) fn restored_eth_sender(
    restore_state: impl IntoIterator<Item = ETHOperation>,
    stats: ETHStats,
) -> (
    ETHSender<MockEthereum, MockDatabase>,
    mpsc::Sender<ETHSenderRequest>,
    mpsc::Receiver<Operation>,
) {
    const MAX_TXS_IN_FLIGHT: u64 = 1;

    build_eth_sender(MAX_TXS_IN_FLIGHT, restore_state, stats)
}

/// Helper method for configurable creation of `ETHSender`.
fn build_eth_sender(
    max_txs_in_flight: u64,
    restore_state: impl IntoIterator<Item = ETHOperation>,
    stats: ETHStats,
) -> (
    ETHSender<MockEthereum, MockDatabase>,
    mpsc::Sender<ETHSenderRequest>,
    mpsc::Receiver<Operation>,
) {
    let ethereum = MockEthereum::default();
    let db = MockDatabase::with_restorable_state(restore_state, stats);

    let (operation_sender, operation_receiver) = mpsc::channel(CHANNEL_CAPACITY);
    let (notify_sender, notify_receiver) = mpsc::channel(CHANNEL_CAPACITY);

    let options = EthSenderOptions {
        max_txs_in_flight,
        expected_wait_time_block: super::EXPECTED_WAIT_TIME_BLOCKS,
        wait_confirmations: super::WAIT_CONFIRMATIONS,
        tx_poll_period: Default::default(),
        is_enabled: true,
    };

    let current_zksync_info = CurrentZkSyncInfo::with_block_number(0);
    let eth_sender = ETHSender::new(
        options,
        db,
        ethereum,
        operation_receiver,
        notify_sender,
        current_zksync_info,
    );

    (eth_sender, operation_sender, notify_receiver)
}

/// Behaves the same as `ETHSender::sign_new_tx`, but does not affect nonce.
/// This method should be used to create expected tx copies which won't affect
/// the internal `ETHSender` state.
pub(in crate::eth_sender) fn create_signed_tx(
    id: i64,
    eth_sender: &ETHSender<MockEthereum, MockDatabase>,
    operation: &Operation,
    deadline_block: u64,
    nonce: i64,
) -> ETHOperation {
    let mut options = Options::default();
    options.nonce = Some(nonce.into());

    let raw_tx = eth_sender.operation_to_raw_tx(&operation);
    let signed_tx = eth_sender
        .ethereum
        .sign_prepared_tx(raw_tx.clone(), options)
        .unwrap();

    let op_type = match operation.action {
        Action::Commit => OperationType::Commit,
        Action::Verify { .. } => OperationType::Verify,
    };

    ETHOperation {
        id,
        op_type,
        op: Some(operation.clone()),
        nonce: signed_tx.nonce,
        last_deadline_block: deadline_block,
        last_used_gas_price: signed_tx.gas_price,
        used_tx_hashes: vec![signed_tx.hash],
        encoded_tx_data: raw_tx,
        confirmed: false,
        final_hash: None,
    }
}

/// Creates an `ETHOperation` object for a withdraw operation.
pub(in crate::eth_sender) fn create_signed_withdraw_tx(
    id: i64,
    eth_sender: &ETHSender<MockEthereum, MockDatabase>,
    deadline_block: u64,
    nonce: i64,
) -> ETHOperation {
    let mut options = Options::default();
    options.nonce = Some(nonce.into());

    let raw_tx = eth_sender.ethereum.encode_tx_data(
        "completeWithdrawals",
        zksync_types::config::MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL,
    );
    let signed_tx = eth_sender
        .ethereum
        .sign_prepared_tx(raw_tx.clone(), options)
        .unwrap();

    let op_type = OperationType::Withdraw;

    ETHOperation {
        id,
        op_type,
        op: None,
        nonce: signed_tx.nonce,
        last_deadline_block: deadline_block,
        last_used_gas_price: signed_tx.gas_price,
        used_tx_hashes: vec![signed_tx.hash],
        encoded_tx_data: raw_tx,
        confirmed: false,
        final_hash: None,
    }
}
