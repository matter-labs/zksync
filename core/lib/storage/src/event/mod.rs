// Built-in uses
// External uses
use serde_json::Value;
// Workspace uses
use zksync_basic_types::{AccountId, BlockNumber};
use zksync_types::account::AccountUpdate;
// Local uses
use crate::{diff::StorageAccountDiff, QueryResult, StorageProcessor};
use records::EventType;
use types::{
    account::{
        AccountEvent, AccountStateChangeStatus, AccountStateChangeType, AccountUpdateDetails,
    },
    block::{BlockEvent, BlockStatus},
};

pub mod records;
pub mod types;

#[derive(Debug)]
pub struct EventSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

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
}
