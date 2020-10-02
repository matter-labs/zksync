// Built-in deps
// External imports
use sqlx::Acquire;
use zksync_basic_types::Address;
// Workspace imports
use zksync_types::{Account, AccountId, AccountUpdates};
// Local imports
use self::records::*;
use crate::diff::StorageAccountDiff;
use crate::{QueryResult, StorageProcessor};

pub mod records;
mod restore_account;
mod stored_state;

pub(crate) use self::restore_account::restore_account;
pub use self::stored_state::StoredAccountState;

/// Account schema contains interfaces to interact with the stored
/// ZKSync accounts.
#[derive(Debug)]
pub struct AccountSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> AccountSchema<'a, 'c> {
    /// Obtains both committed and verified state for the account by its address.
    pub async fn account_state_by_address(
        &mut self,
        address: &Address,
    ) -> QueryResult<StoredAccountState> {
        // Find the account in `account_creates` table.
        let mut results = sqlx::query_as!(
            StorageAccountCreation,
            "
                SELECT * FROM account_creates
                WHERE address = $1 AND is_create = $2
                ORDER BY block_number desc
                LIMIT 1
            ",
            address.as_bytes(),
            true
        )
        .fetch_all(self.0.conn())
        .await?;

        assert!(results.len() <= 1, "LIMIT 1 is in query");
        let account_create_record = results.pop();

        // If account wasn't found, we return no state for it.
        // Otherwise we obtain the account ID for the state lookup.
        let account_id = if let Some(account_create_record) = account_create_record {
            account_create_record.account_id as AccountId
        } else {
            return Ok(StoredAccountState {
                committed: None,
                verified: None,
            });
        };

        // Load committed & verified states, and return them.
        let committed = self
            .last_committed_state_for_account(account_id)
            .await?
            .map(|a| (account_id, a));
        let verified = self
            .last_verified_state_for_account(account_id)
            .await?
            .map(|a| (account_id, a));
        Ok(StoredAccountState {
            committed,
            verified,
        })
    }

    /// Loads the last committed (e.g. just added but no necessarily verified) state for
    /// account given its ID.
    pub async fn last_committed_state_for_account(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<Option<Account>> {
        let mut transaction = self.0.start_transaction().await?;

        // Get the last certain state of the account.
        // Note that `account` can be `None` here (if it wasn't verified yet), since
        // we will update the committed changes below.
        let (last_block, account) = AccountSchema(&mut transaction)
            .get_account_and_last_block(account_id)
            .await?;

        let account_balance_diff = sqlx::query_as!(
            StorageAccountUpdate,
            "
                SELECT * FROM account_balance_updates
                WHERE account_id = $1 AND block_number > $2
            ",
            i64::from(account_id),
            last_block
        )
        .fetch_all(transaction.conn())
        .await?;

        let account_creation_diff = sqlx::query_as!(
            StorageAccountCreation,
            "
                SELECT * FROM account_creates
                WHERE account_id = $1 AND block_number > $2
            ",
            i64::from(account_id),
            last_block
        )
        .fetch_all(transaction.conn())
        .await?;

        let account_pubkey_diff = sqlx::query_as!(
            StorageAccountPubkeyUpdate,
            "
                SELECT * FROM account_pubkey_updates
                WHERE account_id = $1 AND block_number > $2
            ",
            i64::from(account_id),
            last_block
        )
        .fetch_all(transaction.conn())
        .await?;

        // Chain the diffs, converting them into `StorageAccountDiff`.
        let account_diff = {
            let mut account_diff = Vec::new();
            account_diff.extend(
                account_balance_diff
                    .into_iter()
                    .map(StorageAccountDiff::from),
            );
            account_diff.extend(
                account_creation_diff
                    .into_iter()
                    .map(StorageAccountDiff::from),
            );
            account_diff.extend(
                account_pubkey_diff
                    .into_iter()
                    .map(StorageAccountDiff::from),
            );
            account_diff.sort_by(StorageAccountDiff::cmp_order);

            account_diff
                .into_iter()
                .map(Into::into)
                .collect::<AccountUpdates>()
        };

        // Apply all the diffs to obtain the most recent account state.
        let account_state = account_diff
            .into_iter()
            .map(|(_, upd)| upd)
            .fold(account, Account::apply_update);

        transaction.commit().await?;

        Ok(account_state)
    }

    /// Loads the last verified state for the account (e.g. the one obtained in the last block
    /// which was both committed and verified).
    pub async fn last_verified_state_for_account(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<Option<Account>> {
        let (_, account) = self.get_account_and_last_block(account_id).await?;
        Ok(account)
    }

    /// Obtains the last verified state of the account.
    async fn get_account_and_last_block(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<(i64, Option<Account>)> {
        let mut transaction = self.0.conn().begin().await?;

        // `accounts::table` is updated only after the block verification, so we should
        // just load the account with the provided ID.
        let mut results = sqlx::query_as!(
            StorageAccount,
            "
                SELECT * FROM accounts
                WHERE id = $1
                LIMIT 1
            ",
            i64::from(account_id)
        )
        .fetch_all(&mut transaction)
        .await?;

        assert!(results.len() <= 1, "LIMIT 1 is in query");
        let maybe_account = results.pop();

        // let maybe_account = self.0.conn().transaction(|| {
        //     accounts::table
        //         .find(i64::from(account_id))
        //         .first::<StorageAccount>(self.0.conn())
        //         .optional()
        // })?;
        let result = if let Some(account) = maybe_account {
            let balances = sqlx::query_as!(
                StorageBalance,
                "
                    SELECT * FROM balances
                    WHERE account_id = $1
                ",
                i64::from(account_id)
            )
            .fetch_all(&mut transaction)
            .await?;

            let last_block = account.last_block;
            let (_, account) = restore_account(&account, balances);
            Ok((last_block, Some(account)))
        } else {
            Ok((0, None))
        };

        transaction.commit().await?;
        result
    }
}
