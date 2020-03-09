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

impl StorageProcessor {
    fn get_account_and_last_block(
        &self,
        account_id: AccountId,
    ) -> QueryResult<(i64, Option<Account>)> {
        self.conn().transaction(|| {
            if let Some(account) = accounts::table
                .find(i64::from(account_id))
                .first::<StorageAccount>(self.conn())
                .optional()?
            {
                let balances: Vec<StorageBalance> =
                    StorageBalance::belonging_to(&account).load(self.conn())?;

                let last_block = account.last_block;
                let (_, account) = restore_account(account, balances);
                Ok((last_block, Some(account)))
            } else {
                Ok((0, None))
            }
        })
    }

    // Verified, commited states.
    pub fn account_state_by_address(&self, address: &Address) -> QueryResult<StoredAccountState> {
        let account_create_record = account_creates::table
            .filter(account_creates::address.eq(address.as_bytes().to_vec()))
            .filter(account_creates::is_create.eq(true))
            .order(account_creates::block_number.desc())
            .first::<StorageAccountCreation>(self.conn())
            .optional()?;

        let account_id = if let Some(account_create_record) = account_create_record {
            account_create_record.account_id as AccountId
        } else {
            return Ok(StoredAccountState {
                committed: None,
                verified: None,
            });
        };

        let commited = self
            .last_committed_state_for_account(account_id)?
            .map(|a| (account_id, a));
        let verified = self
            .last_verified_state_for_account(account_id)?
            .map(|a| (account_id, a));
        Ok(StoredAccountState {
            committed: commited,
            verified,
        })
    }

    pub fn last_committed_state_for_account(
        &self,
        account_id: AccountId,
    ) -> QueryResult<Option<models::node::Account>> {
        self.conn().transaction(|| {
            let (last_block, account) = self.get_account_and_last_block(account_id)?;

            let account_balance_diff: Vec<StorageAccountUpdate> = {
                account_balance_updates::table
                    .filter(account_balance_updates::account_id.eq(&(i64::from(account_id))))
                    .filter(account_balance_updates::block_number.gt(&last_block))
                    .load::<StorageAccountUpdate>(self.conn())?
            };

            let account_creation_diff: Vec<StorageAccountCreation> = {
                account_creates::table
                    .filter(account_creates::account_id.eq(&(i64::from(account_id))))
                    .filter(account_creates::block_number.gt(&last_block))
                    .load::<StorageAccountCreation>(self.conn())?
            };

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
                    .map(|upd| upd.into())
                    .collect::<AccountUpdates>()
            };

            Ok(account_diff
                .into_iter()
                .map(|(_, upd)| upd)
                .fold(account, Account::apply_update))
        })
    }

    pub fn last_verified_state_for_account(
        &self,
        account_id: AccountId,
    ) -> QueryResult<Option<models::node::Account>> {
        let (_, account) = self.get_account_and_last_block(account_id)?;
        Ok(account)
    }
}
