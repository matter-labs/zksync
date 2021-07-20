//! Mocking utilities for tests.

// Built-in deps
use std::collections::VecDeque;
use std::convert::TryFrom;
// External uses
use tokio::sync::RwLock;
use web3::contract::Options;
use zksync_basic_types::{BlockNumber, H256, U256};
// Workspace uses
use zksync_config::configs::eth_sender::{ETHSenderConfig, GasLimit, Sender};
use zksync_eth_client::EthereumGateway;
use zksync_storage::{ethereum::records::ETHParams, StorageProcessor};
use zksync_types::aggregated_operations::{AggregatedActionType, AggregatedOperation};
use zksync_types::ethereum::{ETHOperation, EthOpId, InsertedOperationResponse};
// Local uses
use super::ETHSender;
use crate::database::DatabaseInterface;
use crate::transactions::ETHStats;
use zksync_eth_client::clients::mock::MockEthereum;

/// Mock database is capable of recording all the incoming requests for the further analysis.
#[derive(Debug)]
pub(in crate) struct MockDatabase {
    eth_operations: RwLock<Vec<ETHOperation>>,
    aggregated_operations: RwLock<Vec<(i64, AggregatedOperation)>>,
    unprocessed_operations: RwLock<Vec<(i64, AggregatedOperation)>>,
    eth_parameters: RwLock<ETHParams>,
}

impl MockDatabase {
    /// Creates a database with emulation of previously stored uncommitted requests.
    pub fn with_restorable_state(
        eth_operations: Vec<ETHOperation>,
        aggregated_operations: Vec<(i64, AggregatedOperation)>,
        unprocessed_operations: Vec<(i64, AggregatedOperation)>,
        eth_parameters: ETHParams,
    ) -> Self {
        Self {
            eth_operations: RwLock::new(eth_operations),
            aggregated_operations: RwLock::new(aggregated_operations),
            unprocessed_operations: RwLock::new(unprocessed_operations),
            eth_parameters: RwLock::new(eth_parameters),
        }
    }

    pub async fn update_gas_price_limit(&self, value: i64) -> anyhow::Result<()> {
        let mut eth_parameters = self.eth_parameters.write().await;
        eth_parameters.gas_price_limit = value;

        Ok(())
    }

    /// Simulates the operation of OperationsSchema, creates a new operation in the database.
    pub async fn send_aggregated_operation(
        &mut self,
        aggregated_operation: (i64, AggregatedOperation),
    ) -> anyhow::Result<()> {
        self.unprocessed_operations
            .write()
            .await
            .push(aggregated_operation.clone());
        self.aggregated_operations
            .write()
            .await
            .push(aggregated_operation);

        Ok(())
    }

    /// Ensures that the provided transaction is stored in the database and not confirmed yet.
    pub async fn assert_stored(&self, tx: &ETHOperation) {
        let eth_operations = self.eth_operations.read().await;
        let is_stored = eth_operations
            .iter()
            .any(|eth_op| eth_op.id == tx.id && !eth_op.confirmed);

        assert!(is_stored);
    }

    /// Ensures that the provided transaction is stored as confirmed.
    pub async fn assert_confirmed(&self, tx: &ETHOperation) {
        let eth_operations = self.eth_operations.read().await;
        let is_confirmed = eth_operations
            .iter()
            .any(|eth_op| eth_op.id == tx.id && eth_op.confirmed);

        assert!(is_confirmed);
    }

    /// Returns the stored average gas price.
    pub async fn average_gas_price(&self) -> U256 {
        let eth_parameters = self.eth_parameters.read().await;

        U256::from(eth_parameters.average_gas_price.unwrap_or_default() as u64)
    }
}

#[async_trait::async_trait]
impl DatabaseInterface for MockDatabase {
    /// Creates a new database connection, used as a stub
    /// and nothing will be sent through this connection.
    async fn acquire_connection(&self) -> anyhow::Result<StorageProcessor<'_>> {
        StorageProcessor::establish_connection().await
    }

    /// Returns all unprocessed operations.
    async fn load_new_operations(
        &self,
        _connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<Vec<(i64, AggregatedOperation)>> {
        let unprocessed_operations = self
            .unprocessed_operations
            .read()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();

        Ok(unprocessed_operations)
    }

    /// Remove the unprocessed operations from the database.
    async fn remove_unprocessed_operations(
        &self,
        _connection: &mut StorageProcessor<'_>,
        operations_id: Vec<i64>,
    ) -> anyhow::Result<()> {
        let mut old_unprocessed_operations = self.unprocessed_operations.write().await;

        let mut new_unprocessed_operations = Vec::new();
        for operation in old_unprocessed_operations.iter() {
            if !operations_id.iter().any(|id| &operation.0 == id) {
                new_unprocessed_operations.push(operation.clone());
            }
        }
        *old_unprocessed_operations = new_unprocessed_operations;

        Ok(())
    }

    async fn update_gas_price_params(
        &self,
        _connection: &mut StorageProcessor<'_>,
        gas_price_limit: U256,
        average_gas_price: U256,
    ) -> anyhow::Result<()> {
        let mut eth_parameters = self.eth_parameters.write().await;
        eth_parameters.gas_price_limit =
            i64::try_from(gas_price_limit).expect("Can't convert U256 to i64");
        eth_parameters.average_gas_price =
            Some(i64::try_from(average_gas_price).expect("Can't convert U256 to i64"));

        Ok(())
    }

    async fn restore_unprocessed_operations(
        &self,
        _connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<()> {
        let aggregated_operations = self.aggregated_operations.read().await;
        let eth_operations = self.eth_operations.read().await;
        let mut unprocessed_operations = self.unprocessed_operations.write().await;

        let mut new_unprocessed_operations = Vec::new();

        for operation in aggregated_operations.iter() {
            let is_operation_in_queue = unprocessed_operations
                .iter()
                .any(|unprocessed_operation| unprocessed_operation.0 == operation.0);
            let is_operation_send_to_ethereum = eth_operations
                .iter()
                .any(|ethereum_operation| ethereum_operation.op.as_ref().unwrap().0 == operation.0);
            if !is_operation_in_queue && !is_operation_send_to_ethereum {
                new_unprocessed_operations.push(operation.clone());
            }
        }

        unprocessed_operations.extend(new_unprocessed_operations);

        Ok(())
    }

    async fn load_unconfirmed_operations(
        &self,
        _connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<VecDeque<ETHOperation>> {
        let unconfirmed_operations = self
            .eth_operations
            .read()
            .await
            .iter()
            .cloned()
            .filter(|eth_op| !eth_op.confirmed)
            .collect();

        Ok(unconfirmed_operations)
    }

    async fn save_new_eth_tx(
        &self,
        _connection: &mut StorageProcessor<'_>,
        op_type: AggregatedActionType,
        op: Option<(i64, AggregatedOperation)>,
        deadline_block: i64,
        used_gas_price: U256,
        encoded_tx_data: Vec<u8>,
    ) -> anyhow::Result<InsertedOperationResponse> {
        let mut eth_operations = self.eth_operations.write().await;
        let id = eth_operations.len() as i64;
        let nonce = eth_operations.len();

        // Store with the assigned ID.
        let eth_operation = ETHOperation {
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

        eth_operations.push(eth_operation);

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
        let mut eth_operations = self.eth_operations.write().await;
        let eth_op = eth_operations
            .iter_mut()
            .find(|eth_op| eth_op.id == eth_op_id && !eth_op.confirmed);

        if let Some(eth_op) = eth_op {
            eth_op.used_tx_hashes.push(*hash);
        } else {
            panic!("Attempt to update tx that is not unconfirmed");
        }

        Ok(())
    }

    async fn update_eth_tx(
        &self,
        _connection: &mut StorageProcessor<'_>,
        eth_op_id: EthOpId,
        new_deadline_block: i64,
        new_gas_value: U256,
    ) -> anyhow::Result<()> {
        let mut eth_operations = self.eth_operations.write().await;
        let eth_op = eth_operations
            .iter_mut()
            .find(|eth_op| eth_op.id == eth_op_id && !eth_op.confirmed);

        if let Some(eth_op) = eth_op {
            eth_op.last_deadline_block = new_deadline_block as u64;
            eth_op.last_used_gas_price = new_gas_value;
        } else {
            panic!("Attempt to update tx that is not unconfirmed");
        }

        Ok(())
    }

    async fn confirm_operation(
        &self,
        _connection: &mut StorageProcessor<'_>,
        hash: &H256,
        _op: &ETHOperation,
    ) -> anyhow::Result<()> {
        let mut eth_operations = self.eth_operations.write().await;
        let mut op_idx: Option<i64> = None;
        for operation in eth_operations.iter_mut() {
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

        Ok(())
    }

    async fn load_gas_price_limit(
        &self,
        _connection: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<U256> {
        let eth_parameters = self.eth_parameters.read().await;
        let gas_price_limit = eth_parameters.gas_price_limit.into();

        Ok(gas_price_limit)
    }

    async fn load_stats(&self, _connection: &mut StorageProcessor<'_>) -> anyhow::Result<ETHStats> {
        let eth_parameters = self.eth_parameters.read().await;
        let eth_stats = ETHStats {
            last_committed_block: eth_parameters.last_committed_block as usize,
            last_verified_block: eth_parameters.last_verified_block as usize,
            last_executed_block: eth_parameters.last_executed_block as usize,
        };

        Ok(eth_stats)
    }

    async fn is_previous_operation_confirmed(
        &self,
        _connection: &mut StorageProcessor<'_>,
        op: &ETHOperation,
    ) -> anyhow::Result<bool> {
        let confirmed = {
            let op = op.op.as_ref().unwrap();
            // We're checking previous block, so for the edge case of first block we can say that previous operation was confirmed.
            let (first_block, _) = op.1.get_block_range();
            if first_block == BlockNumber(1) {
                return Ok(true);
            }

            let eth_operations = self.eth_operations.read().await.clone();

            // Consider an operation that affects sequential blocks.
            let maybe_operation = eth_operations.iter().find(|(eth_operation)| {
                let op_block_range = eth_operation.op.as_ref().unwrap().1.get_block_range();

                op_block_range.1 == first_block - 1
            });

            let operation = match maybe_operation {
                Some(op) => op,
                None => return Ok(false),
            };

            operation.confirmed
        };

        Ok(confirmed)
    }
}

/// Creates a default `ETHParams` for use by mock `ETHSender` .
pub(in crate) fn default_eth_parameters() -> ETHParams {
    ETHParams {
        id: true,
        nonce: 0,
        gas_price_limit: 400000000000,
        average_gas_price: None,
        last_committed_block: 0,
        last_verified_block: 0,
        last_executed_block: 0,
    }
}

/// Creates a default `ETHSender` with mock Ethereum connection/database and no operations in DB.
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(in crate) async fn default_eth_sender() -> ETHSender<MockDatabase> {
    build_eth_sender(
        1,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        default_eth_parameters(),
    )
    .await
}

/// Creates an `ETHSender` with mock Ethereum connection/database and no operations in DB
/// which supports multiple transactions in flight.
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(in crate) async fn concurrent_eth_sender(max_txs_in_flight: u64) -> ETHSender<MockDatabase> {
    build_eth_sender(
        max_txs_in_flight,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        default_eth_parameters(),
    )
    .await
}

/// Creates an `ETHSender` with mock Ethereum connection/database and restores its state "from DB".
/// Returns the `ETHSender` itself along with communication channels to interact with it.
pub(in crate) async fn restored_eth_sender(
    eth_operations: Vec<ETHOperation>,
    aggregated_operations: Vec<(i64, AggregatedOperation)>,
    unprocessed_operations: Vec<(i64, AggregatedOperation)>,
    eth_parameters: ETHParams,
) -> ETHSender<MockDatabase> {
    const MAX_TXS_IN_FLIGHT: u64 = 1;

    build_eth_sender(
        MAX_TXS_IN_FLIGHT,
        eth_operations,
        aggregated_operations,
        unprocessed_operations,
        eth_parameters,
    )
    .await
}

/// Helper method for configurable creation of `ETHSender`.
async fn build_eth_sender(
    max_txs_in_flight: u64,
    eth_operations: Vec<ETHOperation>,
    aggregated_operations: Vec<(i64, AggregatedOperation)>,
    unprocessed_operations: Vec<(i64, AggregatedOperation)>,
    eth_parameters: ETHParams,
) -> ETHSender<MockDatabase> {
    let ethereum = EthereumGateway::Mock(MockEthereum::default());
    let db = MockDatabase::with_restorable_state(
        eth_operations,
        aggregated_operations,
        unprocessed_operations,
        eth_parameters,
    );

    let options = ETHSenderConfig {
        sender: Sender {
            max_txs_in_flight,
            expected_wait_time_block: super::EXPECTED_WAIT_TIME_BLOCKS,
            wait_confirmations: super::WAIT_CONFIRMATIONS,
            tx_poll_period: 0,
            is_enabled: true,
            operator_commit_eth_addr: Default::default(),
            operator_private_key: Default::default(),
        },
        gas_price_limit: GasLimit {
            default: 1000,
            sample_interval: 15,
            update_interval: 15,
            scale_factor: 1.0f64,
        },
    };

    ETHSender::new(options, db, ethereum).await
}

/// Behaves the same as `ETHSender::sign_new_tx`, but does not affect nonce.
/// This method should be used to create expected tx copies which won't affect
/// the internal `ETHSender` state.
pub(in crate) async fn create_signed_tx(
    id: i64,
    eth_sender: &ETHSender<MockDatabase>,
    aggregated_operation: (i64, AggregatedOperation),
    deadline_block: u64,
    nonce: i64,
) -> ETHOperation {
    let options = Options {
        nonce: Some(nonce.into()),
        ..Default::default()
    };

    let raw_tx = eth_sender.operation_to_raw_tx(&aggregated_operation.1);
    let signed_tx = eth_sender
        .ethereum
        .sign_prepared_tx(raw_tx.clone(), options)
        .await
        .unwrap();

    let op_type = aggregated_operation.1.get_action_type();

    ETHOperation {
        id,
        op_type,
        op: Some(aggregated_operation.clone()),
        nonce: signed_tx.nonce,
        last_deadline_block: deadline_block,
        last_used_gas_price: signed_tx.gas_price,
        used_tx_hashes: vec![signed_tx.hash],
        encoded_tx_data: raw_tx,
        confirmed: false,
        final_hash: None,
    }
}
