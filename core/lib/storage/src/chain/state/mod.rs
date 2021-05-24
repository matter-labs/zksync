// Built-in deps
use std::{cmp, collections::HashMap, time::Instant};
// External imports
use num::BigInt;
use sqlx::types::BigDecimal;
// Workspace imports
use zksync_types::{
    helpers::{apply_updates, reverse_updates},
    AccountId, AccountMap, AccountUpdate, AccountUpdates, Address, BlockNumber, PubKeyHash,
    TokenId, NFT,
};
// Local imports
use crate::chain::{
    account::{records::*, restore_account},
    block::BlockSchema,
};
use crate::diff::StorageAccountDiff;
use crate::utils::address_to_stored_string;
// use crate::schema::*;
use crate::{QueryResult, StorageProcessor};

/// State schema is capable of managing... well, the state of the chain.
///
/// This roughly includes the two main topics:
/// - Account management (applying the diffs to the account map).
/// - Block events (which blocks were committed/verified).
///
/// # Representation of the Sidechain State in the DB:
///
/// Saving state is done in two steps:
/// 1. When the block is committed, we save all state updates
///   (tables: `account_creates`, `account_balance_updates`)
/// 2. Once the block is verified, we apply this updates to stored state snapshot
///   (tables: `accounts`, `balances`)
///
/// This way we have the following advantages:
/// - Easy access to state for any block (useful for provers which work on different blocks)
/// - We can rewind any `committed` state (which is not final)
#[derive(Debug)]
pub struct StateSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> StateSchema<'a, 'c> {
    /// Stores the list of updates to the account map in the database.
    /// At this step, the changes are not verified yet, and thus are not applied.
    pub async fn commit_state_update(
        &mut self,
        block_number: BlockNumber,
        accounts_updated: &[(AccountId, AccountUpdate)],
        first_update_order_id: usize,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        // Simply go through the every account update, and update the corresponding table.
        // This may look scary, but every match arm is very simple by its nature.

        let update_order_ids =
            first_update_order_id..first_update_order_id + accounts_updated.len();

        for (update_order_id, (id, upd)) in update_order_ids.zip(accounts_updated.iter()) {
            vlog::debug!(
                "Committing state update for account {} in block {}",
                **id,
                *block_number
            );

            match *upd {
                AccountUpdate::Create { ref address, nonce } => {
                    let account_id = i64::from(**id);
                    let is_create = true;
                    let block_number = i64::from(*block_number);
                    let address = address.as_bytes().to_vec();
                    let nonce = i64::from(*nonce);
                    let update_order_id = update_order_id as i32;
                    sqlx::query!(
                        r#"
                        INSERT INTO account_creates ( account_id, is_create, block_number, address, nonce, update_order_id )
                        VALUES ( $1, $2, $3, $4, $5, $6 )
                        "#,
                        account_id, is_create, block_number, address, nonce, update_order_id
                    )
                    .execute(transaction.conn())
                    .await?;
                }
                AccountUpdate::Delete { ref address, nonce } => {
                    let account_id = i64::from(**id);
                    let is_create = false;
                    let block_number = i64::from(*block_number);
                    let address = address.as_bytes().to_vec();
                    let nonce = i64::from(*nonce);
                    let update_order_id = update_order_id as i32;
                    sqlx::query!(
                        r#"
                        INSERT INTO account_creates ( account_id, is_create, block_number, address, nonce, update_order_id )
                        VALUES ( $1, $2, $3, $4, $5, $6 )
                        "#,
                        account_id, is_create, block_number, address, nonce, update_order_id
                    )
                    .execute(transaction.conn())
                    .await?;
                }
                AccountUpdate::UpdateBalance {
                    balance_update: (token, ref old_balance, ref new_balance),
                    old_nonce,
                    new_nonce,
                } => {
                    let account_id = i64::from(**id);
                    let block_number = i64::from(*block_number);
                    let coin_id = *token as i32;
                    let old_balance = BigDecimal::from(BigInt::from(old_balance.clone()));
                    let new_balance = BigDecimal::from(BigInt::from(new_balance.clone()));
                    let old_nonce = i64::from(*old_nonce);
                    let new_nonce = i64::from(*new_nonce);
                    let update_order_id = update_order_id as i32;

                    sqlx::query!(
                        r#"
                        INSERT INTO account_balance_updates ( account_id, block_number, coin_id, old_balance, new_balance, old_nonce, new_nonce, update_order_id )
                        VALUES ( $1, $2, $3, $4, $5, $6, $7, $8 )
                        "#,
                        account_id,
                        block_number,
                        coin_id,
                        old_balance,
                        new_balance,
                        old_nonce,
                        new_nonce,
                        update_order_id,
                    )
                    .execute(transaction.conn())
                    .await?;
                }
                AccountUpdate::ChangePubKeyHash {
                    ref old_pub_key_hash,
                    ref new_pub_key_hash,
                    old_nonce,
                    new_nonce,
                } => {
                    let update_order_id = update_order_id as i32;
                    let account_id = i64::from(**id);
                    let block_number = i64::from(*block_number);
                    let old_pubkey_hash = old_pub_key_hash.data.to_vec();
                    let new_pubkey_hash = new_pub_key_hash.data.to_vec();
                    let old_nonce = i64::from(*old_nonce);
                    let new_nonce = i64::from(*new_nonce);
                    sqlx::query!(
                        r#"
                        INSERT INTO account_pubkey_updates ( update_order_id, account_id, block_number, old_pubkey_hash, new_pubkey_hash, old_nonce, new_nonce )
                        VALUES ( $1, $2, $3, $4, $5, $6, $7 )
                        "#,
                        update_order_id, account_id, block_number, old_pubkey_hash, new_pubkey_hash, old_nonce, new_nonce
                    )
                    .execute(transaction.conn())
                    .await?;
                }
                AccountUpdate::MintNFT { ref token } => {
                    let update_order_id = update_order_id as i32;
                    let token_id = token.id.0 as i32;
                    let creator_account_id = token.creator_id.0 as i32;
                    let serial_id = token.serial_id as i32;
                    let creator_address = token.creator_address.as_bytes().to_vec();
                    let address = token.address.as_bytes().to_vec();
                    let content_hash = token.content_hash.as_bytes().to_vec();
                    let block_number = i64::from(*block_number);
                    sqlx::query!(
                        r#"
                        INSERT INTO mint_nft_updates ( token_id, creator_account_id, creator_address, serial_id, address, content_hash, block_number, update_order_id, symbol )
                        VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9)
                        "#,
                        token_id, creator_account_id, creator_address, serial_id, address, content_hash, block_number, update_order_id, token.symbol
                    )
                        .execute(transaction.conn())
                        .await?;
                }
                AccountUpdate::RemoveNFT { ref token } => {
                    let token_id = token.id.0 as i32;
                    let block_number = i64::from(*block_number);
                    sqlx::query!(
                        r#"
                        DELETE FROM mint_nft_updates
                        WHERE token_id = $1 and block_number = $2
                        "#,
                        token_id,
                        block_number
                    )
                    .execute(transaction.conn())
                    .await?;
                }
            }
        }

        transaction.commit().await?;

        metrics::histogram!("sql.chain.state.commit_state_update", start.elapsed());
        Ok(())
    }

    pub async fn apply_storage_account_diff(
        &mut self,
        acc_update: StorageAccountDiff,
    ) -> QueryResult<()> {
        match acc_update {
            StorageAccountDiff::BalanceUpdate(upd) => {
                sqlx::query!(
                    r#"
                    INSERT INTO balances ( account_id, coin_id, balance )
                    VALUES ( $1, $2, $3 )
                    ON CONFLICT (account_id, coin_id)
                    DO UPDATE
                      SET balance = $3
                    "#,
                    upd.account_id,
                    upd.coin_id,
                    upd.new_balance.clone(),
                )
                .execute(self.0.conn())
                .await?;

                sqlx::query!(
                    r#"
                    UPDATE accounts 
                    SET last_block = $1, nonce = $2
                    WHERE id = $3
                    "#,
                    upd.block_number,
                    upd.new_nonce,
                    upd.account_id,
                )
                .execute(self.0.conn())
                .await?;
            }

            StorageAccountDiff::Create(upd) => {
                sqlx::query!(
                    r#"
                    INSERT INTO accounts ( id, last_block, nonce, address, pubkey_hash )
                    VALUES ( $1, $2, $3, $4, $5 )
                    "#,
                    upd.account_id,
                    upd.block_number,
                    upd.nonce,
                    upd.address,
                    PubKeyHash::default().data.to_vec()
                )
                .execute(self.0.conn())
                .await?;
            }
            StorageAccountDiff::Delete(upd) => {
                sqlx::query!(
                    r#"
                    DELETE FROM accounts
                    WHERE id = $1
                    "#,
                    upd.account_id,
                )
                .execute(self.0.conn())
                .await?;
            }
            StorageAccountDiff::ChangePubKey(upd) => {
                sqlx::query!(
                    r#"
                    UPDATE accounts 
                    SET last_block = $1, nonce = $2, pubkey_hash = $3
                    WHERE id = $4
                    "#,
                    upd.block_number,
                    upd.new_nonce,
                    upd.new_pubkey_hash,
                    upd.account_id,
                )
                .execute(self.0.conn())
                .await?;
            }
            StorageAccountDiff::MintNFT(upd) => {
                let address = address_to_stored_string(&Address::from_slice(&upd.address));
                sqlx::query!(
                    r#"
                    INSERT INTO tokens ( id, address, symbol, decimals, is_nft )
                    VALUES ( $1, $2, $3, $4, true )
                    "#,
                    upd.token_id,
                    address,
                    upd.symbol,
                    0
                )
                .execute(self.0.conn())
                .await?;

                sqlx::query!(
                    r#"
                    INSERT INTO nft ( token_id, creator_address, creator_account_id, serial_id, address, content_hash )
                    VALUES ( $1, $2, $3, $4, $5, $6)
                    "#,
                    upd.token_id,
                    upd.creator_address,
                    upd.creator_account_id,
                    upd.serial_id,
                    upd.address,
                    upd.content_hash,
                )
                    .execute(self.0.conn())
                    .await?;
            }
        }

        Ok(())
    }

    /// Applies the previously stored list of account changes to the stored state.
    ///
    /// This method is invoked from the `zksync_eth_sender` after corresponding `Verify` transaction
    /// is confirmed on Ethereum blockchain.
    pub async fn apply_state_update(&mut self, block_number: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        vlog::info!("Applying state update for block: {}", block_number);
        let mut transaction = self.0.start_transaction().await?;

        // Collect the stored updates. This includes collecting entries from three tables:
        // `account_creates` (for creating/removing accounts),
        // `account_balance_updates` (for changing the balance of accounts),
        // `account_pubkey_updates` (for changing the accounts public keys).
        let account_balance_diff = sqlx::query_as!(
            StorageAccountUpdate,
            "SELECT * FROM account_balance_updates WHERE block_number = $1",
            i64::from(*block_number)
        )
        .fetch_all(transaction.conn())
        .await?;

        let account_creation_diff = sqlx::query_as!(
            StorageAccountCreation,
            "
                SELECT * FROM account_creates
                WHERE block_number = $1
            ",
            i64::from(*block_number)
        )
        .fetch_all(transaction.conn())
        .await?;

        let account_change_pubkey_diff = sqlx::query_as!(
            StorageAccountPubkeyUpdate,
            "
                SELECT * FROM account_pubkey_updates
                WHERE block_number = $1
            ",
            i64::from(*block_number)
        )
        .fetch_all(transaction.conn())
        .await?;

        let mint_nft_updates_diff = sqlx::query_as!(
            StorageMintNFTUpdate,
            "
                SELECT * FROM mint_nft_updates
                WHERE block_number = $1
            ",
            i64::from(*block_number)
        )
        .fetch_all(transaction.conn())
        .await?;

        // Collect the updates into one list of `StorageAccountDiff`.
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
            account_diff.extend(
                mint_nft_updates_diff
                    .into_iter()
                    .map(StorageAccountDiff::from),
            );
            account_diff.sort_by(StorageAccountDiff::cmp_order);
            account_diff
        };

        vlog::debug!("Sorted account update list: {:?}", account_updates);

        // Then go through the collected list of changes and apply them by one.
        for acc_update in account_updates.into_iter() {
            transaction
                .chain()
                .state_schema()
                .apply_storage_account_diff(acc_update)
                .await?;
        }

        transaction.commit().await?;

        metrics::histogram!("sql.chain.state.apply_state_update", start.elapsed());
        Ok(())
    }

    /// Loads the committed (not necessarily verified) account map state along
    /// with a block number to which this state applies.
    /// If the provided block number is `None`, then the latest committed
    /// state will be loaded.
    pub async fn load_committed_state(
        &mut self,
        block: Option<BlockNumber>,
    ) -> QueryResult<(BlockNumber, AccountMap)> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let (verif_block, mut accounts) =
            StateSchema(&mut transaction).load_verified_state().await?;
        vlog::debug!(
            "Verified state block: {}, accounts: {:#?}",
            *verif_block,
            accounts
        );

        let state_diff = StateSchema(&mut transaction)
            .load_state_diff(verif_block, block)
            .await?;

        // Fetch updates from blocks: verif_block +/- 1, ... , block
        let result = if let Some((block, state_diff)) = state_diff {
            vlog::debug!("Loaded state diff: {:#?}", state_diff);
            apply_updates(&mut accounts, state_diff);
            Ok((block, accounts))
        } else {
            Ok((verif_block, accounts))
        };

        transaction.commit().await?;

        metrics::histogram!("sql.chain.state.load_committed_state", start.elapsed());
        result
    }

    /// Loads the verified account map state along with a block number
    /// to which this state applies.
    /// If the provided block number is `None`, then the latest committed
    /// state will be loaded.
    pub async fn load_verified_state(&mut self) -> QueryResult<(BlockNumber, AccountMap)> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let last_block = BlockSchema(&mut transaction)
            .get_last_verified_confirmed_block()
            .await?;

        let accounts = sqlx::query_as!(StorageAccount, "SELECT * FROM accounts")
            .fetch_all(transaction.conn())
            .await?;

        let mut account_map = AccountMap::default();

        for stored_accounts in accounts.chunks(2usize.pow(15)) {
            let stored_account_ids: Vec<_> = stored_accounts.iter().map(|acc| acc.id).collect();
            let balances = sqlx::query_as!(
                StorageBalance,
                "SELECT * FROM balances WHERE account_id = ANY($1)",
                &stored_account_ids
            )
            .fetch_all(transaction.conn())
            .await?;

            let mut balances_for_id: HashMap<AccountId, Vec<StorageBalance>> = HashMap::new();

            for balance in balances.into_iter() {
                balances_for_id
                    .entry(AccountId(balance.account_id as u32))
                    .and_modify(|balances| balances.push(balance.clone()))
                    .or_insert_with(|| vec![balance]);
            }

            for stored_account in stored_accounts {
                let id = AccountId(stored_account.id as u32);
                let balances = balances_for_id.remove(&id).unwrap_or_default();
                let (id, account) = restore_account(stored_account, balances);
                account_map.insert(id, account);
            }
        }

        transaction.commit().await?;
        metrics::histogram!("sql.chain.state.load_verified_state", start.elapsed());
        Ok((last_block, account_map))
    }

    /// Returns the list of updates, and the block number such that if we apply
    /// these updates to the state of the block #(from_block), we will obtain state of the block
    /// #(returned block number).
    /// Returned block number is either `to_block`, latest committed block before `to_block`.
    /// If `to_block` is `None`, then it will be assumed to be the number of the latest committed
    /// block.
    pub async fn load_state_diff(
        &mut self,
        from_block: BlockNumber,
        to_block: Option<BlockNumber>,
    ) -> QueryResult<Option<(BlockNumber, AccountUpdates)>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        // Resolve the end of range: if it was not provided, we have to fetch
        // the latest committed block.
        let to_block_resolved = if let Some(to_block) = to_block {
            to_block
        } else {
            let last_block = sqlx::query!("SELECT max(number) FROM blocks",)
                .fetch_one(transaction.conn())
                .await?;

            last_block
                .max
                .map(|last_block| BlockNumber(last_block as u32))
                .unwrap_or(BlockNumber(0))
        };

        // Determine the order: are we going forward or backwards.
        // Depending on that, determine the start/end of the block range as well.
        let (time_forward, start_block, end_block) = (
            from_block <= to_block_resolved,
            cmp::min(from_block, to_block_resolved),
            cmp::max(from_block, to_block_resolved),
        );

        // Collect the stored updates. This includes collecting entries from three tables:
        // `account_creates` (for creating/removing accounts),
        // `account_balance_updates` (for changing the balance of accounts),
        // `account_pubkey_updates` (for changing the accounts public keys).
        // The updates are loaded for the given blocks range.
        let account_balance_diff = sqlx::query_as!(
            StorageAccountUpdate,
            "SELECT * FROM account_balance_updates WHERE block_number > $1 AND block_number <= $2 ",
            i64::from(*start_block),
            i64::from(*end_block),
        )
        .fetch_all(transaction.conn())
        .await?;

        let account_creation_diff = sqlx::query_as!(
            StorageAccountCreation,
            "SELECT * FROM account_creates WHERE block_number > $1 AND block_number <= $2 ",
            i64::from(*start_block),
            i64::from(*end_block),
        )
        .fetch_all(transaction.conn())
        .await?;

        let account_pubkey_diff = sqlx::query_as!(
            StorageAccountPubkeyUpdate,
            "SELECT * FROM account_pubkey_updates WHERE block_number > $1 AND block_number <= $2 ",
            i64::from(*start_block),
            i64::from(*end_block),
        )
        .fetch_all(transaction.conn())
        .await?;

        let mint_nft_diffs = sqlx::query_as!(
            StorageMintNFTUpdate,
            "SELECT * FROM mint_nft_updates WHERE block_number > $1 AND block_number <= $2 ",
            i64::from(*start_block),
            i64::from(*end_block),
        )
        .fetch_all(transaction.conn())
        .await?;

        vlog::debug!(
            "Loading state diff: forward: {}, start_block: {}, end_block: {}, unbounded: {}",
            time_forward,
            *start_block,
            *end_block,
            to_block.is_none()
        );
        vlog::debug!("Loaded account balance diff: {:#?}", account_balance_diff);
        vlog::debug!("Loaded account creation diff: {:#?}", account_creation_diff);

        // Fold the updates into one list and determine the actual last block
        // (since user-provided one may not exist yet).
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
            account_diff.extend(mint_nft_diffs.into_iter().map(StorageAccountDiff::from));
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
                BlockNumber(last_block as u32),
            )
        };

        // Reverse the blocks order if needed.
        if !time_forward {
            reverse_updates(&mut account_updates);
        }

        // Determine the block number which state will be obtained after
        // applying the changes.
        let block_after_updates = if time_forward {
            last_block
        } else {
            start_block
        };

        transaction.commit().await?;
        metrics::histogram!("sql.chain.state.load_state_diff", start.elapsed());

        // We don't want to return an empty list to avoid the confusion, so return
        // `None` if there are no changes.
        if !account_updates.is_empty() {
            Ok(Some((block_after_updates, account_updates)))
        } else {
            Ok(None)
        }
    }

    /// Loads the state of accounts updated in a specific block.
    pub async fn load_state_diff_for_block(
        &mut self,
        block_number: BlockNumber,
    ) -> QueryResult<AccountUpdates> {
        let start = Instant::now();
        let result = self
            .load_state_diff(block_number - 1, Some(block_number))
            .await
            .map(|diff| diff.unwrap_or_default().1);

        metrics::histogram!("sql.chain.state.load_state_diff", start.elapsed());
        result
    }

    pub async fn get_mint_nft_update(&mut self, token_id: TokenId) -> QueryResult<Option<NFT>> {
        let start = Instant::now();
        let nft = sqlx::query_as!(
            StorageMintNFTUpdate,
            r#"
            SELECT * FROM mint_nft_updates 
            WHERE token_id = $1
            "#,
            *token_id as i32
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.get_mint_nft_update", start.elapsed());
        Ok(nft.map(|p| p.into()))
    }
    pub async fn load_committed_nft_tokens(
        &mut self,
        block_number: Option<BlockNumber>,
    ) -> QueryResult<Vec<StorageMintNFTUpdate>> {
        let tokens = if let Some(block_number) = block_number {
            sqlx::query_as!(
                StorageMintNFTUpdate,
                "SELECT * FROM mint_nft_updates WHERE block_number <= $1",
                block_number.0 as i64
            )
            .fetch_all(self.0.conn())
            .await
        } else {
            sqlx::query_as!(StorageMintNFTUpdate, "SELECT * FROM mint_nft_updates")
                .fetch_all(self.0.conn())
                .await
        };
        Ok(tokens?)
    }

    // Removes account balance updates for blocks with number greater than `last_block`
    pub async fn remove_account_balance_updates(
        &mut self,
        last_block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM account_balance_updates WHERE block_number > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.state.remove_account_balance_updates",
            start.elapsed()
        );
        Ok(())
    }

    // Removes account creates for blocks with number greater than `last_block`
    pub async fn remove_account_creates(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM account_creates WHERE block_number > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.state.remove_account_creates", start.elapsed());
        Ok(())
    }

    // Removes mint_nft_updates for blocks with number greater than `last_block`
    pub async fn remove_mint_nft_updates(&mut self, last_block: BlockNumber) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM mint_nft_updates WHERE block_number > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.state.remove_mint_nft_updates", start.elapsed());
        Ok(())
    }

    // Removes account pubkey updates for blocks with number greater than `last_block`
    pub async fn remove_account_pubkey_updates(
        &mut self,
        last_block: BlockNumber,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            "DELETE FROM account_pubkey_updates WHERE block_number > $1",
            *last_block as i64
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.chain.state.remove_account_pubkey_updates",
            start.elapsed()
        );
        Ok(())
    }
}
