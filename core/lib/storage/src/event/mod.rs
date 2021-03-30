// Built-in uses
// External uses
use num::BigInt;
use serde_json::Value;
use sqlx::types::BigDecimal;
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
        let block_details = self
            .0
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

        self.store_event_data(EventType::Block, event_data).await?;

        Ok(())
    }

    pub async fn store_state_committed_event(
        &mut self,
        account_id: AccountId,
        account_update: &AccountUpdate,
    ) -> QueryResult<()> {
        let mut account_update_details = AccountUpdateDetails::new(account_id);
        match account_update {
            AccountUpdate::Create { address: _, nonce } => {
                account_update_details.nonce = i64::from(**nonce);
            }
            AccountUpdate::Delete { address: _, nonce } => {
                account_update_details.nonce = i64::from(**nonce);
            }
            AccountUpdate::UpdateBalance {
                old_nonce: _,
                new_nonce,
                balance_update,
            } => {
                account_update_details.nonce = i64::from(**new_nonce);
                account_update_details.token_id = Some(i32::from(*balance_update.0));
                let new_balance = BigDecimal::from(BigInt::from(balance_update.2.clone()));
                account_update_details.new_balance = Some(new_balance);
            }
            AccountUpdate::ChangePubKeyHash {
                old_pub_key_hash: _,
                new_pub_key_hash,
                old_nonce: _,
                new_nonce,
            } => {
                account_update_details.nonce = i64::from(**new_nonce);
                account_update_details.pub_key_hash = new_pub_key_hash.clone();
            }
        }

        if !matches!(account_update, AccountUpdate::ChangePubKeyHash { .. }) {
            let account = self
                .0
                .chain()
                .account_schema()
                .last_committed_state_for_account(account_id)
                .await?;
            if let Some(account) = account {
                account_update_details.pub_key_hash = account.pub_key_hash;
            }
        }

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
        let mut account_update_details = AccountUpdateDetails::from(account_diff);

        if !matches!(account_diff, StorageAccountDiff::ChangePubKey(_)) {
            let account = self
                .0
                .chain()
                .account_schema()
                .last_verified_state_for_account(AccountId(
                    account_update_details.account_id as u32,
                ))
                .await?;
            if let Some(account) = account {
                account_update_details.pub_key_hash = account.pub_key_hash;
            }
        }

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
