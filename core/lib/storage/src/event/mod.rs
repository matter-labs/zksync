// Built-in uses
use std::time::Instant;
// External uses
use serde_json::Value;
// Workspace uses
use zksync_basic_types::{AccountId, BlockNumber};
use zksync_types::event::{
    account::{
        AccountEvent, AccountStateChangeStatus, AccountStateChangeType, AccountUpdateDetails,
    },
    block::{BlockEvent, BlockStatus},
    transaction::TransactionEvent,
    EventId, ZkSyncEvent,
};
use zksync_types::{
    account::AccountUpdate, block::ExecutedOperations, priority_ops::ZkSyncPriorityOp,
};
// Local uses
use crate::{diff::StorageAccountDiff, QueryResult, StorageProcessor};
use records::StoredEvent;

pub mod records;

pub use records::{get_event_type, EventType};

/// Schema for persisting events that happen in the zkSync network.
///
/// All events are serialized into JSON and stored in a single `events` table.
/// On every insert into this table a special PostgreSQL channel gets notified
/// about it.
///
/// Note, that all events should be created solely by other `storage` methods
/// and they are persisted in the database forever.
#[derive(Debug)]
pub struct EventSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> EventSchema<'a, 'c> {
    /// Store new serialized event in the database. This method is private
    /// since the type safety is only guaranteed by the correctness of `event_type`
    /// parameter.
    async fn store_event_data(
        &mut self,
        block_number: BlockNumber,
        event_type: EventType,
        event_data: Value,
    ) -> QueryResult<()> {
        let start = Instant::now();
        // Note, that the id can happen not to be continuous,
        // sequences are always incremented ignoring
        // the fact whether the transaction is committed or reverted.
        sqlx::query!(
            "INSERT INTO events VALUES (DEFAULT, $1, $2, $3)",
            i64::from(*block_number),
            event_type as EventType,
            event_data
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.event.store_event_data", start.elapsed());
        Ok(())
    }

    /// Load all events from the database with the `id` greater than `from`.
    pub async fn fetch_new_events(&mut self, from: EventId) -> QueryResult<Vec<ZkSyncEvent>> {
        let start = Instant::now();
        let events = sqlx::query_as!(
            StoredEvent,
            r#"
            SELECT
                id,
                block_number,
                event_type as "event_type!: EventType",
                event_data
            FROM events WHERE id > $1
            ORDER BY id ASC
            "#,
            *from as i64
        )
        .fetch_all(self.0.conn())
        .await?
        .into_iter()
        .map(ZkSyncEvent::from)
        .collect();

        metrics::histogram!("sql.event.fetch_new_events", start.elapsed());
        Ok(events)
    }

    /// Load the id of the latest event in the database.
    /// Returns `None` if the `events` table is empty.
    pub async fn get_last_event_id(&mut self) -> QueryResult<Option<EventId>> {
        let start = Instant::now();
        let id = sqlx::query!("SELECT MAX(id) as max FROM events")
            .fetch_one(self.0.conn())
            .await?
            .max
            .map(|id| EventId(id as u64));

        metrics::histogram!("sql.event.get_last_event_id", start.elapsed());
        Ok(id)
    }

    /// Create new block event and store it in the database.
    /// This method relies on the `load_block_range` which may return `None`
    /// if there're no Ethereum transactions featuring this block (`Committed` or `Executed`).
    /// In such cases, it silently returns `Ok`.
    pub async fn store_block_event(
        &mut self,
        status: BlockStatus,
        block_number: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let block_details = transaction
            .chain()
            .block_schema()
            .load_block_range(block_number, 1)
            .await?;
        // If there're no block details for the given block number,
        // ignore the event. Since the `eth_sender` is currently
        // responsible for confirming Ethereum operations in the database,
        // failing here will make it think there's some kind of network error.
        let block_details = match block_details.into_iter().next() {
            Some(block_details) => block_details,
            None => {
                vlog::warn!(
                    "Couldn't create block event, no block details found in the database. \
                    Block number: {}, status: {:?}",
                    *block_number,
                    status
                );
                return Ok(());
            }
        };

        let block_event = BlockEvent {
            status,
            block_details: block_details.into(),
        };

        let event_data = serde_json::to_value(block_event).expect("couldn't serialize block event");

        transaction
            .event_schema()
            .store_event_data(block_number, EventType::Block, event_data)
            .await?;
        transaction.commit().await?;

        metrics::histogram!("sql.event.store_block_event", start.elapsed());
        Ok(())
    }

    /// Create new account event with the `Committed` status
    /// and store it in the database.
    pub async fn store_state_committed_event(
        &mut self,
        block_number: BlockNumber,
        account_id: AccountId,
        account_update: &AccountUpdate,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let account_update_details =
            AccountUpdateDetails::from_account_update(account_id, account_update);

        let update_type = AccountStateChangeType::from(account_update);
        let status = AccountStateChangeStatus::Committed;

        let account_event = AccountEvent {
            update_type,
            status,
            update_details: account_update_details,
        };

        let event_data =
            serde_json::to_value(account_event).expect("couldn't serialize account event");

        self.store_event_data(block_number, EventType::Account, event_data)
            .await?;

        metrics::histogram!("sql.event.store_state_committed_event", start.elapsed());
        Ok(())
    }

    /// Create new account event with the `Finalized` status
    /// and store it in the database.
    pub async fn store_state_verified_event(
        &mut self,
        block_number: BlockNumber,
        account_diff: &StorageAccountDiff,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let account_update_details = AccountUpdateDetails::from(account_diff);

        let update_type = AccountStateChangeType::from(account_diff);
        let status = AccountStateChangeStatus::Finalized;

        let account_event = AccountEvent {
            update_type,
            status,
            update_details: account_update_details,
        };

        let event_data =
            serde_json::to_value(account_event).expect("couldn't serialize account event");

        self.store_event_data(block_number, EventType::Account, event_data)
            .await?;

        metrics::histogram!("sql.event.store_state_verified_event", start.elapsed());
        Ok(())
    }

    /// Returns the account affected by the operation.
    /// In case of `Deposit` we have to query the database in order
    /// to get the id, at this point the account creation should be already
    /// committed in storage.
    async fn account_id_from_op(
        &mut self,
        executed_operation: &ExecutedOperations,
    ) -> QueryResult<AccountId> {
        let priority_op = match executed_operation {
            ExecutedOperations::Tx(tx) => {
                return tx.signed_tx.tx.account_id().map_err(anyhow::Error::from)
            }
            ExecutedOperations::PriorityOp(priority_op) => priority_op,
        };
        match &priority_op.priority_op.data {
            ZkSyncPriorityOp::Deposit(deposit) => self
                .0
                .chain()
                .account_schema()
                .account_id_by_address(deposit.to)
                .await?
                .ok_or_else(|| anyhow::Error::msg("Account doesn't exist")),
            ZkSyncPriorityOp::FullExit(full_exit) => Ok(full_exit.account_id),
        }
    }

    /// Create new transaction event and store it in the database.
    pub async fn store_transaction_event(
        &mut self,
        executed_operation: &ExecutedOperations,
        block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        // Like in the case of block events, we return `Ok`
        // if we didn't manage to fetch the account id for the given
        // operation.
        let account_id = match transaction
            .event_schema()
            .account_id_from_op(executed_operation)
            .await
        {
            Ok(account_id) => account_id,
            _ => {
                vlog::warn!(
                    "Couldn't create transaction event, no account id exists \
                    in the database. Operation: {:?}",
                    executed_operation
                );
                return Ok(());
            }
        };

        let transaction_event =
            TransactionEvent::from_executed_operation(executed_operation, block, account_id);

        let event_data =
            serde_json::to_value(transaction_event).expect("couldn't serialize transaction event");

        transaction
            .event_schema()
            .store_event_data(block, EventType::Transaction, event_data)
            .await?;
        transaction.commit().await?;

        metrics::histogram!("sql.event.store_transaction_event", start.elapsed());
        Ok(())
    }
}
