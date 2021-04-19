// Built-in uses
// External uses
use serde_json::Value;
// Workspace uses
use zksync_basic_types::{AccountId, BlockNumber};
use zksync_types::{
    account::AccountUpdate, block::ExecutedOperations, priority_ops::ZkSyncPriorityOp,
};
// Local uses
use crate::{diff::StorageAccountDiff, QueryResult, StorageProcessor};
use records::{EventType, StoredEvent};
use types::{
    account::{
        AccountEvent, AccountStateChangeStatus, AccountStateChangeType, AccountUpdateDetails,
    },
    block::{BlockEvent, BlockStatus},
    transaction::TransactionEvent,
    ZkSyncEvent,
};

pub mod records;
pub mod types;

#[derive(Debug)]
pub struct EventSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

// TODO: add metrics.
impl<'a, 'c> EventSchema<'a, 'c> {
    async fn store_event_data(
        &mut self,
        event_type: EventType,
        event_data: Value,
    ) -> QueryResult<()> {
        sqlx::query!(
            "INSERT INTO events VALUES (DEFAULT, $1, $2, $3)",
            event_type as EventType,
            event_data,
            false
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    pub async fn get_unprocessed_events(&mut self) -> QueryResult<Vec<ZkSyncEvent>> {
        let events = sqlx::query_as!(
            StoredEvent,
            r#"
            UPDATE events SET is_processed = true 
            WHERE is_processed = false
            RETURNING id, event_type as "event_type!: EventType",
                event_data, is_processed
            "#
        )
        .fetch_all(self.0.conn())
        .await?
        .into_iter()
        .map(ZkSyncEvent::from)
        .collect();

        Ok(events)
    }

    pub async fn get_last_processed_event_id(&mut self) -> QueryResult<Option<i64>> {
        let id = sqlx::query!("SELECT MAX(id) FROM events WHERE is_processed = true")
            .fetch_one(self.0.conn())
            .await?
            .max;

        Ok(id)
    }

    pub async fn store_block_event(
        &mut self,
        status: BlockStatus,
        block_number: BlockNumber,
    ) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;

        let block_details = transaction
            .chain()
            .block_schema()
            .load_block_range(block_number, 1)
            .await?;

        let block_details = match block_details.into_iter().next() {
            Some(block_details) => block_details,
            None => return Ok(()),
        };

        let block_event = BlockEvent {
            status,
            block_details,
        };

        let event_data = serde_json::to_value(block_event).expect("couldn't serialize block event");

        transaction
            .event_schema()
            .store_event_data(EventType::Block, event_data)
            .await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn store_state_committed_event(
        &mut self,
        account_id: AccountId,
        account_update: &AccountUpdate,
    ) -> QueryResult<()> {
        // TODO: skip fee account?
        let account_update_details =
            AccountUpdateDetails::from_account_update(account_id, account_update);

        let update_type = AccountStateChangeType::from(account_update);
        let status = AccountStateChangeStatus::Committed;

        let account_event = AccountEvent {
            update_type,
            status,
            account_update_details,
        };

        let event_data =
            serde_json::to_value(account_event).expect("couldn't serialize account event");

        self.store_event_data(EventType::Account, event_data)
            .await?;

        Ok(())
    }

    pub async fn store_state_verified_event(
        &mut self,
        account_diff: &StorageAccountDiff,
    ) -> QueryResult<()> {
        // TODO: skip fee account?
        let account_update_details = AccountUpdateDetails::from(account_diff);

        let update_type = AccountStateChangeType::from(account_diff);
        let status = AccountStateChangeStatus::Finalized;

        let account_event = AccountEvent {
            update_type,
            status,
            account_update_details,
        };

        let event_data =
            serde_json::to_value(account_event).expect("couldn't serialize account event");

        self.store_event_data(EventType::Account, event_data)
            .await?;

        Ok(())
    }

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

    pub async fn store_transaction_event(
        &mut self,
        executed_operation: &ExecutedOperations,
        block: BlockNumber,
    ) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;

        let account_id = match transaction
            .event_schema()
            .account_id_from_op(executed_operation)
            .await
        {
            Ok(account_id) => account_id,
            _ => return Ok(()),
        };

        let transaction_event =
            TransactionEvent::from_executed_operation(executed_operation, block, account_id);

        let event_data =
            serde_json::to_value(transaction_event).expect("couldn't serialize account event");

        transaction
            .event_schema()
            .store_event_data(EventType::Transaction, event_data)
            .await?;
        transaction.commit().await?;

        Ok(())
    }
}
