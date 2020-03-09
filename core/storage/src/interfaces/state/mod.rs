// Built-in deps
use std::cmp;
// External imports
use diesel::dsl::{delete, insert_into, max, update};
use diesel::prelude::*;
// Workspace imports
use models::node::PubKeyHash;
use models::node::{apply_updates, reverse_updates, AccountMap, AccountUpdate, AccountUpdates};
// Local imports
use self::records::{StoredBlockEvent, StoredStorageState};
use crate::diff::StorageAccountDiff;
use crate::interfaces::account::records::{
    StorageAccount, StorageAccountCreation, StorageAccountPubkeyUpdate,
    StorageAccountPubkeyUpdateInsert, StorageAccountUpdate, StorageAccountUpdateInsert,
    StorageBalance,
};
use crate::interfaces::account::restore_account;
use crate::schema::*;
use crate::StorageProcessor;

pub mod records;

impl StorageProcessor {
    pub fn commit_state_update(
        &self,
        block_number: u32,
        accounts_updated: &[(u32, AccountUpdate)],
    ) -> QueryResult<()> {
        self.conn().transaction(|| {
            for (update_order_id, (id, upd)) in accounts_updated.iter().enumerate() {
                log::debug!(
                    "Committing state update for account {} in block {}",
                    id,
                    block_number
                );
                match *upd {
                    AccountUpdate::Create { ref address, nonce } => {
                        diesel::insert_into(account_creates::table)
                            .values(&StorageAccountCreation {
                                update_order_id: update_order_id as i32,
                                account_id: i64::from(*id),
                                is_create: true,
                                block_number: i64::from(block_number),
                                address: address.as_bytes().to_vec(),
                                nonce: i64::from(nonce),
                            })
                            .execute(self.conn())?;
                    }
                    AccountUpdate::Delete { ref address, nonce } => {
                        diesel::insert_into(account_creates::table)
                            .values(&StorageAccountCreation {
                                update_order_id: update_order_id as i32,
                                account_id: i64::from(*id),
                                is_create: false,
                                block_number: i64::from(block_number),
                                address: address.as_bytes().to_vec(),
                                nonce: i64::from(nonce),
                            })
                            .execute(self.conn())?;
                    }
                    AccountUpdate::UpdateBalance {
                        balance_update: (token, ref old_balance, ref new_balance),
                        old_nonce,
                        new_nonce,
                    } => {
                        diesel::insert_into(account_balance_updates::table)
                            .values(&StorageAccountUpdateInsert {
                                update_order_id: update_order_id as i32,
                                account_id: i64::from(*id),
                                block_number: i64::from(block_number),
                                coin_id: i32::from(token),
                                old_balance: old_balance.clone(),
                                new_balance: new_balance.clone(),
                                old_nonce: i64::from(old_nonce),
                                new_nonce: i64::from(new_nonce),
                            })
                            .execute(self.conn())?;
                    }
                    AccountUpdate::ChangePubKeyHash {
                        ref old_pub_key_hash,
                        ref new_pub_key_hash,
                        old_nonce,
                        new_nonce,
                    } => {
                        diesel::insert_into(account_pubkey_updates::table)
                            .values(&StorageAccountPubkeyUpdateInsert {
                                update_order_id: update_order_id as i32,
                                account_id: i64::from(*id),
                                block_number: i64::from(block_number),
                                old_pubkey_hash: old_pub_key_hash.data.to_vec(),
                                new_pubkey_hash: new_pub_key_hash.data.to_vec(),
                                old_nonce: i64::from(old_nonce),
                                new_nonce: i64::from(new_nonce),
                            })
                            .execute(self.conn())?;
                    }
                }
            }
            Ok(())
        })
    }

    pub fn apply_state_update(&self, block_number: u32) -> QueryResult<()> {
        log::info!("Applying state update for block: {}", block_number);
        self.conn().transaction(|| {
            let account_balance_diff = account_balance_updates::table
                .filter(account_balance_updates::block_number.eq(&(i64::from(block_number))))
                .load::<StorageAccountUpdate>(self.conn())?;

            let account_creation_diff = account_creates::table
                .filter(account_creates::block_number.eq(&(i64::from(block_number))))
                .load::<StorageAccountCreation>(self.conn())?;

            let account_change_pubkey_diff = account_pubkey_updates::table
                .filter(account_pubkey_updates::block_number.eq(&(i64::from(block_number))))
                .load::<StorageAccountPubkeyUpdate>(self.conn())?;

            let account_updates: Vec<StorageAccountDiff> = {
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
                    account_change_pubkey_diff
                        .into_iter()
                        .map(StorageAccountDiff::from),
                );
                account_diff.sort_by(StorageAccountDiff::cmp_order);
                account_diff
            };

            log::debug!("Sorted account update list: {:?}", account_updates);

            for acc_update in account_updates.into_iter() {
                match acc_update {
                    StorageAccountDiff::BalanceUpdate(upd) => {
                        let storage_balance = StorageBalance {
                            coin_id: upd.coin_id,
                            account_id: upd.account_id,
                            balance: upd.new_balance.clone(),
                        };
                        insert_into(balances::table)
                            .values(&storage_balance)
                            .on_conflict((balances::coin_id, balances::account_id))
                            .do_update()
                            .set(balances::balance.eq(upd.new_balance))
                            .execute(self.conn())?;

                        update(accounts::table.filter(accounts::id.eq(upd.account_id)))
                            .set((
                                accounts::last_block.eq(upd.block_number),
                                accounts::nonce.eq(upd.new_nonce),
                            ))
                            .execute(self.conn())?;
                    }

                    StorageAccountDiff::Create(upd) => {
                        let storage_account = StorageAccount {
                            id: upd.account_id,
                            last_block: upd.block_number,
                            nonce: upd.nonce,
                            address: upd.address,
                            pubkey_hash: PubKeyHash::default().data.to_vec(),
                        };
                        insert_into(accounts::table)
                            .values(&storage_account)
                            .execute(self.conn())?;
                    }
                    StorageAccountDiff::Delete(upd) => {
                        delete(accounts::table.filter(accounts::id.eq(upd.account_id)))
                            .execute(self.conn())?;
                    }
                    StorageAccountDiff::ChangePubKey(upd) => {
                        update(accounts::table.filter(accounts::id.eq(upd.account_id)))
                            .set((
                                accounts::last_block.eq(upd.block_number),
                                accounts::nonce.eq(upd.new_nonce),
                                accounts::pubkey_hash.eq(upd.new_pubkey_hash),
                            ))
                            .execute(self.conn())?;
                    }
                }
            }

            Ok(())
        })
    }

    pub fn load_committed_state(&self, block: Option<u32>) -> QueryResult<(u32, AccountMap)> {
        self.conn().transaction(|| {
            let (verif_block, mut accounts) = self.load_verified_state()?;
            log::debug!(
                "Verified state block: {}, accounts: {:#?}",
                verif_block,
                accounts
            );

            // Fetch updates from blocks: verif_block +/- 1, ... , block
            if let Some((block, state_diff)) = self.load_state_diff(verif_block, block)? {
                log::debug!("Loaded state diff: {:#?}", state_diff);
                apply_updates(&mut accounts, state_diff);
                Ok((block, accounts))
            } else {
                Ok((verif_block, accounts))
            }
        })
    }

    pub fn load_verified_state(&self) -> QueryResult<(u32, AccountMap)> {
        self.conn().transaction(|| {
            let last_block = self.get_last_verified_block()?;

            let accounts: Vec<StorageAccount> = accounts::table.load(self.conn())?;
            let balances: Vec<Vec<StorageBalance>> = StorageBalance::belonging_to(&accounts)
                .load(self.conn())?
                .grouped_by(&accounts);

            let account_map: AccountMap = accounts
                .into_iter()
                .zip(balances.into_iter())
                .map(|(stored_account, balances)| {
                    let (id, account) = restore_account(stored_account, balances);
                    (id, account)
                })
                .collect();

            Ok((last_block, account_map))
        })
    }

    /// Returns updates, and block number such that
    /// if we apply this updates to state of the block #(from_block) we will have state of the block
    /// #(returned block number)
    /// returned block number is either to_block, last commited block before to_block, (if to_block == None
    /// we assume to_bloc = +Inf)
    pub fn load_state_diff(
        &self,
        from_block: u32,
        to_block: Option<u32>,
    ) -> QueryResult<Option<(u32, AccountUpdates)>> {
        self.conn().transaction(|| {
            let to_block_resolved = if let Some(to_block) = to_block {
                to_block
            } else {
                let last_block = blocks::table
                    .select(max(blocks::number))
                    .first::<Option<i64>>(self.conn())?;
                last_block.map(|n| n as u32).unwrap_or(0)
            };

            let (time_forward, start_block, end_block) = (
                from_block <= to_block_resolved,
                cmp::min(from_block, to_block_resolved),
                cmp::max(from_block, to_block_resolved),
            );

            let account_balance_diff = account_balance_updates::table
                .filter(
                    account_balance_updates::block_number
                        .gt(&(i64::from(start_block)))
                        .and(account_balance_updates::block_number.le(&(i64::from(end_block)))),
                )
                .load::<StorageAccountUpdate>(self.conn())?;
            let account_creation_diff = account_creates::table
                .filter(
                    account_creates::block_number
                        .gt(&(i64::from(start_block)))
                        .and(account_creates::block_number.le(&(i64::from(end_block)))),
                )
                .load::<StorageAccountCreation>(self.conn())?;
            let account_pubkey_diff = account_pubkey_updates::table
                .filter(
                    account_pubkey_updates::block_number
                        .gt(&(i64::from(start_block)))
                        .and(account_pubkey_updates::block_number.le(&(i64::from(end_block)))),
                )
                .load::<StorageAccountPubkeyUpdate>(self.conn())?;

            log::debug!(
                "Loading state diff: forward: {}, start_block: {}, end_block: {}, unbounded: {}",
                time_forward,
                start_block,
                end_block,
                to_block.is_none()
            );
            log::debug!("Loaded account balance diff: {:#?}", account_balance_diff);
            log::debug!("Loaded account creation diff: {:#?}", account_creation_diff);

            let (mut account_updates, last_block) = {
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
                let last_block = account_diff
                    .iter()
                    .map(|acc| acc.block_number())
                    .max()
                    .unwrap_or(0);
                account_diff.sort_by(StorageAccountDiff::cmp_order);
                (
                    account_diff
                        .into_iter()
                        .map(|d| d.into())
                        .collect::<AccountUpdates>(),
                    last_block as u32,
                )
            };

            if !time_forward {
                reverse_updates(&mut account_updates);
            }

            let block_after_updates = if time_forward {
                last_block
            } else {
                start_block
            };

            if !account_updates.is_empty() {
                Ok(Some((block_after_updates, account_updates)))
            } else {
                Ok(None)
            }
        })
    }

    /// loads the state of accounts updated in a specific block
    pub fn load_state_diff_for_block(&self, block_number: u32) -> QueryResult<AccountUpdates> {
        self.load_state_diff(block_number - 1, Some(block_number))
            .map(|diff| diff.unwrap_or_default().1)
    }

    pub fn load_committed_events_state(&self) -> QueryResult<Vec<StoredBlockEvent>> {
        let events = events_state::table
            .filter(events_state::block_type.eq("Committed".to_string()))
            .order(events_state::block_num.asc())
            .load::<StoredBlockEvent>(self.conn())?;
        Ok(events)
    }

    pub fn load_verified_events_state(&self) -> QueryResult<Vec<StoredBlockEvent>> {
        let events = events_state::table
            .filter(events_state::block_type.eq("Verified".to_string()))
            .order(events_state::block_num.asc())
            .load::<StoredBlockEvent>(self.conn())?;
        Ok(events)
    }

    pub fn load_storage_state(&self) -> QueryResult<StoredStorageState> {
        storage_state_update::table.first(self.conn())
    }
}
