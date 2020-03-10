// Built-in deps
// External imports
use diesel::prelude::*;
use web3::types::Address;
// Workspace imports
use models::node::{Account, AccountId, AccountUpdates};
// Local imports
use self::records::*;
use crate::diff::StorageAccountDiff;
use crate::schema::*;
use crate::StorageProcessor;

pub mod records;
mod restore_account;
mod stored_state;

pub(crate) use self::restore_account::restore_account;
pub use self::stored_state::StoredAccountState;

/// Account schema contains interfaces to interact with the stored
/// ZKSync accounts.
pub struct AccountSchema<'a>(pub &'a StorageProcessor);

impl<'a> AccountSchema<'a> {
    /// Obtains both committed and verified state for the account by its address.
    pub fn account_state_by_address(&self, address: &Address) -> QueryResult<StoredAccountState> {
        // Find the account in `account_creates` table.
        let account_create_record = account_creates::table
            .filter(account_creates::address.eq(address.as_bytes().to_vec()))
            .filter(account_creates::is_create.eq(true))
            .order(account_creates::block_number.desc())
            .first::<StorageAccountCreation>(self.0.conn())
            .optional()?;

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
            .last_committed_state_for_account(account_id)?
            .map(|a| (account_id, a));
        let verified = self
            .last_verified_state_for_account(account_id)?
            .map(|a| (account_id, a));
        Ok(StoredAccountState {
            committed,
            verified,
        })
    }

    /// Loads the last committed (e.g. just added but no necessarily verified) state for
    /// account given its ID.
    pub fn last_committed_state_for_account(
        &self,
        account_id: AccountId,
    ) -> QueryResult<Option<Account>> {
        // Get the last certain state of the account.
        // Note that `account` can be `None` here (if it wasn't verified yet), since
        // we will update the committed changes below.
        let (last_block, account) = self.get_account_and_last_block(account_id)?;

        // Collect the diffs that we have to apply to the account.
        let (account_balance_diff, account_creation_diff) = self
            .0
            .conn()
            .transaction::<_, diesel::result::Error, _>(|| {
                // From `account_balance_updates` load entries with the same ID and height
                // greater than for the last verified block.
                let account_balance_diff: Vec<StorageAccountUpdate> = {
                    account_balance_updates::table
                        .filter(account_balance_updates::account_id.eq(&(i64::from(account_id))))
                        .filter(account_balance_updates::block_number.gt(&last_block))
                        .load::<StorageAccountUpdate>(self.0.conn())?
                };

                // The same as above, but for `account_creates` table.
                let account_creation_diff: Vec<StorageAccountCreation> = {
                    account_creates::table
                        .filter(account_creates::account_id.eq(&(i64::from(account_id))))
                        .filter(account_creates::block_number.gt(&last_block))
                        .load::<StorageAccountCreation>(self.0.conn())?
                };

                Ok((account_balance_diff, account_creation_diff))
            })?;

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

        Ok(account_state)
    }

    /// Loads the last verified state for the account (e.g. the one obtained in the last block
    /// which was both committed and verified).
    pub fn last_verified_state_for_account(
        &self,
        account_id: AccountId,
    ) -> QueryResult<Option<Account>> {
        let (_, account) = self.get_account_and_last_block(account_id)?;
        Ok(account)
    }

    /// Obtains the last verified state of the account.
    fn get_account_and_last_block(
        &self,
        account_id: AccountId,
    ) -> QueryResult<(i64, Option<Account>)> {
        // `accounts::table` is updated only after the block verification, so we should
        // just load the account with the provided ID.
        let maybe_account = self.0.conn().transaction(|| {
            accounts::table
                .find(i64::from(account_id))
                .first::<StorageAccount>(self.0.conn())
                .optional()
        })?;
        if let Some(account) = maybe_account {
            let balances: Vec<StorageBalance> =
                StorageBalance::belonging_to(&account).load(self.0.conn())?;

            let last_block = account.last_block;
            let (_, account) = restore_account(account, balances);
            Ok((last_block, Some(account)))
        } else {
            Ok((0, None))
        }
    }
}
