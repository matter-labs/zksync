//! Mocking utilities for tests.

// Built-in deps
use crate::database::DatabaseInterface;
use crate::ethereum_interface::FailureInfo;
use crate::EthSenderOptions;
use std::collections::{BTreeMap, HashMap, VecDeque};
use tokio::sync::RwLock;
// External uses
use web3::contract::{tokens::Tokenize, Options};
use zksync_basic_types::{H256, U256};
// Workspace uses
use zksync_eth_client::SignedCallResult;
use zksync_storage::StorageProcessor;
use zksync_types::{
    ethereum::{ETHOperation, EthOpId, InsertedOperationResponse, OperationType},
    Action, Operation,
};
// Local uses
use super::ETHSender;

use crate::ethereum_interface::EthereumInterface;
use crate::transactions::{ETHStats, ExecutedTxStatus};

/// Mock database is capable of recording all the incoming requests for the further analysis.
#[derive(Debug, Default)]
pub(in crate) struct MockDatabase {
    restore_state: VecDeque<ETHOperation>,
    unconfirmed_operations: RwLock<BTreeMap<i64, ETHOperation>>,
    unprocessed_operations: RwLock<BTreeMap<i64, Operation>>,
    confirmed_operations: RwLock<BTreeMap<i64, ETHOperation>>,
    nonce: RwLock<i64>,
    gas_price_limit: RwLock<U256>,
    pending_op_id: RwLock<EthOpId>,
    stats: RwLock<ETHStats>,
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

        let unconfirmed_operations: BTreeMap<i64, ETHOperation> =
            restore_state.iter().map(|op| (op.id, op.clone())).collect();

        let gas_price_limit: u64 = zksync_utils::parse_env("ETH_GAS_PRICE_DEFAULT_LIMIT");

        Self {
            restore_state,
            nonce: RwLock::new(nonce as i64),
            gas_price_limit: RwLock::new(gas_price_limit.into()),
            pending_op_id: RwLock::new(pending_op_id as EthOpId),
            stats: RwLock::new(stats),
            unconfirmed_operations: RwLock::new(unconfirmed_operations),
            ..Default::default()
        }
    }

    pub async fn update_gas_price_limit(&self, value: U256) -> anyhow::Result<()> {
        let mut gas_price_limit = self.gas_price_limit.write().await;
        (*gas_price_limit) = value;

        Ok(())
    }

    /// Simulates the operation of OperationsSchema, creates a new operation in the database.
    pub async fn send_operation(&mut self, op: Operation) -> anyhow::Result<()> {
        let nonce = op.id.expect("Nonce must be set for every tx");

        self.unprocessed_operations.write().await.insert(nonce, op);

        Ok(())
    }

    /// Ensures that the provided transaction is stored in the database and not confirmed yet.
    pub async fn assert_stored(&self, tx: &ETHOperation) {
        assert_eq!(
            self.unconfirmed_operations.read().await.get(&tx.id),
            Some(tx)
        );

        assert!(self.confirmed_operations.read().await.get(&tx.id).is_none());
    }

    /// Ensures that the provided transaction is stored as confirmed.
    pub async fn assert_confirmed(&self, tx: &ETHOperation) {
        assert_eq!(self.confirmed_operations.read().await.get(&tx.id), Some(tx));

        assert!(self
            .unconfirmed_operations
            .read()
            .await
            .get(&tx.id)
            .is_none());
    }

    async fn next_nonce(&self) -> anyhow::Result<i64> {
        let old_value = *(self.nonce.read().await);
        let mut new_value = self.nonce.write().await;
        *new_value = old_value + 1;

        Ok(old_value)
    }
}

#[async_trait::async_trait]
impl DatabaseInterface for MockDatabase {
    /// Creates a new database connection, used as a stub
    /// and nothing will be sent through this connection.
    async fn acquire_connection(&self) -> anyhow::Result<StorageProcessor<'_>> {
        StorageProcessor::establish_connection().await
    }

    /// Returns all unprocessed operations and then deletes them.
    async fn load_new_operations(
        &self,
        _connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<Vec<Operation>> {
        let unprocessed_operations = self
            .unprocessed_operations
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();

        self.unprocessed_operations.write().await.clear();

        Ok(unprocessed_operations)
    }

    async fn update_gas_price_params(
        &self,
        _connection: &mut StorageProcessor<'_>,
        gas_price_limit: U256,
        _average_gas_price: U256,
    ) -> anyhow::Result<()> {
        let mut new_gas_price_limit = self.gas_price_limit.write().await;
        *new_gas_price_limit = gas_price_limit;

        Ok(())
    }

    async fn restore_state(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<(VecDeque<ETHOperation>, Vec<Operation>)> {
        Ok((
            self.restore_state.clone(),
            self.load_new_operations(connection).await?,
        ))
    }

    async fn save_new_eth_tx(
        &self,
        _connection: &mut StorageProcessor<'_>,
        op_type: OperationType,
        op: Option<Operation>,
        deadline_block: i64,
        used_gas_price: U256,
        encoded_tx_data: Vec<u8>,
    ) -> anyhow::Result<InsertedOperationResponse> {
        let id = *(self.pending_op_id.read().await);
        let mut pending_op_id = self.pending_op_id.write().await;
        *pending_op_id = id + 1;

        let nonce = self.next_nonce().await?;

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

        self.unconfirmed_operations.write().await.insert(id, state);

        let response = InsertedOperationResponse {
            id,
            nonce: nonce.into(),
        };

        Ok(response)
    }

    /// Adds a tx hash entry associated with some Ethereum operation to the database.
    async fn add_hash_entry(
        &self,
        _connection: &mut StorageProcessor<'_>,
        eth_op_id: i64,
        hash: &H256,
    ) -> anyhow::Result<()> {
        assert!(
            self.unconfirmed_operations
                .read()
                .await
                .contains_key(&eth_op_id),
            "Attempt to update tx that is not unconfirmed"
        );

        let mut ops = self.unconfirmed_operations.write().await;
        let mut op = ops[&eth_op_id].clone();
        op.used_tx_hashes.push(*hash);
        ops.insert(eth_op_id, op);

        Ok(())
    }

    async fn update_eth_tx(
        &self,
        _connection: &mut StorageProcessor<'_>,
        eth_op_id: EthOpId,
        new_deadline_block: i64,
        new_gas_value: U256,
    ) -> anyhow::Result<()> {
        assert!(
            self.unconfirmed_operations
                .read()
                .await
                .contains_key(&eth_op_id),
            "Attempt to update tx that is not unconfirmed"
        );

        let mut ops = self.unconfirmed_operations.write().await;
        let mut op = ops[&eth_op_id].clone();
        op.last_deadline_block = new_deadline_block as u64;
        op.last_used_gas_price = new_gas_value;
        ops.insert(eth_op_id, op);

        Ok(())
    }

    async fn confirm_operation(
        &self,
        _connection: &mut StorageProcessor<'_>,
        hash: &H256,
        _op: &ETHOperation,
    ) -> anyhow::Result<()> {
        let mut unconfirmed_operations = self.unconfirmed_operations.write().await;
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
            .write()
            .await
            .insert(op_idx, operation);

        Ok(())
    }

    async fn load_gas_price_limit(
        &self,
        _connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<U256> {
        Ok(*self.gas_price_limit.read().await)
    }

    async fn load_stats(&self, _connection: &mut StorageProcessor<'_>) -> anyhow::Result<ETHStats> {
        Ok(self.stats.read().await.clone())
    }

    async fn is_previous_operation_confirmed(
        &self,
        _connection: &mut StorageProcessor<'_>,
        op: &ETHOperation,
    ) -> anyhow::Result<bool> {
        let confirmed = match op.op_type {
            OperationType::Commit | OperationType::Verify => {
                let op = op.op.as_ref().unwrap();
                // We're checking previous block, so for the edge case of first block we can say that it was confirmed.
                let block_to_check = if op.block.block_number > 1 {
                    op.block.block_number - 1
                } else {
                    return Ok(true);
                };

                let confirmed_operations = self.confirmed_operations.read().await.clone();
                let maybe_operation = confirmed_operations.get(&(block_to_check as i64));

                let operation = match maybe_operation {
                    Some(op) => op,
                    None => return Ok(false),
                };

                operation.confirmed
            }
            OperationType::Withdraw => {
                // Withdrawals aren't actually sequential, so we don't really care.
                true
            }
        };

        Ok(confirmed)
    }
}

/// Mock Ethereum client is capable of recording all the incoming requests for the further analysis.
#[derive(Debug)]
pub(in crate) struct MockEthereum {
    pub block_number: u64,
    pub gas_price: U256,
    pub tx_statuses: RwLock<HashMap<H256, ExecutedTxStatus>>,
    pub sent_txs: RwLock<HashMap<H256, SignedCallResult>>,
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
    pub async fn assert_sent(&self, hash: &H256) {
        assert!(
            self.sent_txs.read().await.get(hash).is_some(),
            format!("Transaction with hash {:?} was not sent", hash),
        );
    }

    /// Adds an response for the sent transaction for `ETHSender` to receive.
    pub async fn add_execution(&mut self, hash: &H256, status: &ExecutedTxStatus) {
        self.tx_statuses.write().await.insert(*hash, status.clone());
    }

    /// Increments the blocks by a provided `confirmations` and marks the sent transaction
    /// as a success.
    pub async fn add_successfull_execution(&mut self, tx_hash: H256, confirmations: u64) {
        self.block_number += confirmations;

        let status = ExecutedTxStatus {
            confirmations,
            success: true,
            receipt: None,
        };
        self.tx_statuses.write().await.insert(tx_hash, status);
    }

    /// Same as `add_successfull_execution`, but marks the transaction as a failure.
    pub async fn add_failed_execution(&mut self, hash: &H256, confirmations: u64) {
        self.block_number += confirmations;

        let status = ExecutedTxStatus {
            confirmations,
            success: false,
            receipt: Some(Default::default()),
        };
        self.tx_statuses.write().await.insert(*hash, status);
    }
}

#[async_trait::async_trait]
impl EthereumInterface for MockEthereum {
    async fn get_tx_status(&self, hash: &H256) -> anyhow::Result<Option<ExecutedTxStatus>> {
        Ok(self.tx_statuses.read().await.get(hash).cloned())
    }

    async fn block_number(&self) -> anyhow::Result<u64> {
        Ok(self.block_number)
    }

    async fn gas_price(&self) -> anyhow::Result<U256> {
        Ok(self.gas_price)
    }

    async fn send_tx(&self, signed_tx: &SignedCallResult) -> anyhow::Result<()> {
        self.sent_txs
            .write()
            .await
            .insert(signed_tx.hash, signed_tx.clone());

        Ok(())
    }

    fn encode_tx_data<P: Tokenize>(&self, _func: &str, params: P) -> Vec<u8> {
        ethabi::encode(params.into_tokens().as_ref())
    }

    async fn sign_prepared_tx(
        &self,
        raw_tx: Vec<u8>,
        options: Options,
    ) -> anyhow::Result<SignedCallResult> {
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

    async fn failure_reason(&self, _tx_hash: H256) -> Option<FailureInfo> {
        None
    }
}

/// Creates a default `ETHSender` with mock Ethereum connection/database and no operations in DB.
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(in crate) async fn default_eth_sender() -> ETHSender<MockEthereum, MockDatabase> {
    build_eth_sender(1, Vec::new(), Default::default()).await
}

/// Creates an `ETHSender` with mock Ethereum connection/database and no operations in DB
/// which supports multiple transactions in flight.
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(in crate) async fn concurrent_eth_sender(
    max_txs_in_flight: u64,
) -> ETHSender<MockEthereum, MockDatabase> {
    build_eth_sender(max_txs_in_flight, Vec::new(), Default::default()).await
}

/// Creates an `ETHSender` with mock Ethereum connection/database and restores its state "from DB".
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(in crate) async fn restored_eth_sender(
    restore_state: impl IntoIterator<Item = ETHOperation>,
    stats: ETHStats,
) -> ETHSender<MockEthereum, MockDatabase> {
    const MAX_TXS_IN_FLIGHT: u64 = 1;

    build_eth_sender(MAX_TXS_IN_FLIGHT, restore_state, stats).await
}

/// Helper method for configurable creation of `ETHSender`.
async fn build_eth_sender(
    max_txs_in_flight: u64,
    restore_state: impl IntoIterator<Item = ETHOperation>,
    stats: ETHStats,
) -> ETHSender<MockEthereum, MockDatabase> {
    let ethereum = MockEthereum::default();
    let db = MockDatabase::with_restorable_state(restore_state, stats);

    let options = EthSenderOptions {
        max_txs_in_flight,
        expected_wait_time_block: super::EXPECTED_WAIT_TIME_BLOCKS,
        wait_confirmations: super::WAIT_CONFIRMATIONS,
        tx_poll_period: Default::default(),
        is_enabled: true,
    };

    ETHSender::new(options, db, ethereum).await
}

/// Behaves the same as `ETHSender::sign_new_tx`, but does not affect nonce.
/// This method should be used to create expected tx copies which won't affect
/// the internal `ETHSender` state.
pub(in crate) async fn create_signed_tx(
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
        .await
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
pub(in crate) async fn create_signed_withdraw_tx(
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
        .await
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
