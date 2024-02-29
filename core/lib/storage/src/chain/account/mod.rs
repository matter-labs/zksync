// Built-in deps
use std::time::Instant;
// External imports
use num::{BigUint, Zero};
use sqlx::{types::BigDecimal, Acquire};
// Workspace imports
use zksync_crypto::params::{MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID};
use zksync_types::{
    Account, AccountId, AccountUpdates, Address, BlockNumber, Nonce, PubKeyHash, TokenId,
};
// Local imports
use self::records::*;
use crate::chain::block::BlockSchema;
use crate::diff::StorageAccountDiff;
use crate::{QueryResult, StorageProcessor};

pub mod records;
pub mod restore_account;
mod stored_state;

pub(crate) use self::restore_account::restore_account;
pub use self::stored_state::StoredAccountState;
use crate::tokens::records::StorageNFT;
use num::bigint::ToBigInt;

/// Account schema contains interfaces to interact with the stored
/// ZKSync accounts.
#[derive(Debug)]
pub struct AccountSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> AccountSchema<'a, 'c> {
    /// Stores account type in the database
    pub async fn set_account_type(
        &mut self,
        account_id: AccountId,
        account_type: EthAccountType,
    ) -> QueryResult<()> {
        let start = Instant::now();

        let mut transaction = self.0.start_transaction().await?;

        let (db_account_type, pub_key_hash) = account_type.into_db_types();

        sqlx::query!(
            r#"
            INSERT INTO eth_account_types VALUES ( $1, $2 )
            ON CONFLICT (account_id) DO UPDATE SET account_type = $2
            "#,
            i64::from(*account_id),
            db_account_type as DbAccountType
        )
        .execute(transaction.conn())
        .await?;

        if let Some(hash) = pub_key_hash {
            sqlx::query!(
                r#"
                INSERT INTO no_2fa_pub_key_hash VALUES ( $1, $2 )
                ON CONFLICT (account_id) DO UPDATE SET pub_key_hash = $2
                "#,
                i64::from(*account_id),
                hash.as_hex()
            )
            .execute(transaction.conn())
            .await?;
        } else {
            sqlx::query!(
                r#"
                DELETE FROM no_2fa_pub_key_hash WHERE account_id = $1
                "#,
                i64::from(*account_id)
            )
            .execute(transaction.conn())
            .await?;
        }

        transaction.commit().await?;

        metrics::histogram!("sql.chain.state.set_account_type", start.elapsed());
        Ok(())
    }

    /// Gets currently committed to the database nonce, if not exist return verified.
    /// After reverting blocks this nonce could be less than actual.
    /// Use this function only for verifying the lower bounds of a nonce.
    pub async fn estimate_nonce(&mut self, account_id: AccountId) -> QueryResult<Option<Nonce>> {
        let start = Instant::now();

        let mut transaction = self.0.start_transaction().await?;

        let committed_nonce = sqlx::query!(
            "SELECT nonce FROM committed_nonce WHERE account_id = $1",
            i64::from(*account_id)
        )
        .fetch_optional(transaction.conn())
        .await?;

        let current_nonce = if let Some(nonce) = committed_nonce {
            Some(nonce.nonce)
        } else {
            let verified_nonce = sqlx::query!(
                "SELECT nonce FROM accounts WHERE id = $1",
                i64::from(*account_id)
            )
            .fetch_optional(transaction.conn())
            .await?;
            verified_nonce.map(|nonce| nonce.nonce)
        };

        metrics::histogram!("sql.chain.account.current_nonce", start.elapsed());
        Ok(current_nonce.map(|v| Nonce(v as u32)))
    }

    /// Fetches account type from the database
    pub async fn account_type_by_id(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<Option<EthAccountType>> {
        let start = Instant::now();

        let mut transaction = self.0.start_transaction().await?;

        let db_account_type = sqlx::query_as!(
            StorageAccountType,
            r#"
            SELECT account_id, account_type as "account_type!: DbAccountType" 
            FROM eth_account_types WHERE account_id = $1
            "#,
            i64::from(*account_id)
        )
        .fetch_optional(transaction.conn())
        .await?
        .map(|record| record.account_type);

        let pub_key_hash = if let Some(DbAccountType::No2FA) = db_account_type {
            let result = sqlx::query!(
                r#"
                SELECT pub_key_hash 
                FROM no_2fa_pub_key_hash WHERE account_id = $1
                "#,
                i64::from(*account_id)
            )
            .fetch_optional(transaction.conn())
            .await?;

            result.map(|record| PubKeyHash::from_hex(&record.pub_key_hash).unwrap())
        } else {
            None
        };

        let account_type =
            db_account_type.map(|db_type| EthAccountType::from_db(db_type, pub_key_hash));
        metrics::histogram!("sql.chain.account.account_type_by_id", start.elapsed());
        Ok(account_type)
    }

    /// Obtains both committed and verified state for the account by its ID.
    pub async fn account_state_by_id(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<StoredAccountState> {
        let start = Instant::now();
        // Load committed & verified states, and return them.
        let (verified_state, committed_state) = self
            .0
            .chain()
            .account_schema()
            .last_committed_state_for_account(account_id)
            .await?;

        metrics::histogram!("sql.chain.account.account_state_by_id", start.elapsed());
        Ok(StoredAccountState {
            committed: committed_state.map(|a| (account_id, a)),
            verified: verified_state.1.map(|a| (account_id, a)),
        })
    }

    /// Check the existence of an account by the address on the zksync network,
    /// will return true if the account exists
    pub async fn does_account_exist(&mut self, address: Address) -> QueryResult<bool> {
        let start = Instant::now();

        let result = sqlx::query!(
            r#"
                SELECT account_id 
                FROM account_creates WHERE address = $1
                "#,
            address.as_bytes()
        )
        .fetch_optional(self.0.conn())
        .await?;
        metrics::histogram!("sql.chain.account.does_account_exist", start.elapsed());
        Ok(result.is_some())
    }

    /// Obtains both committed and verified state for the account by its address.
    pub async fn account_state_by_address(
        &mut self,
        address: Address,
    ) -> QueryResult<StoredAccountState> {
        let start = Instant::now();

        let account_state = if let Some(account_id) = self.account_id_by_address(address).await? {
            self.account_state_by_id(account_id).await
        } else {
            Ok(StoredAccountState {
                committed: None,
                verified: None,
            })
        };

        metrics::histogram!(
            "sql.chain.account.account_state_by_address",
            start.elapsed()
        );
        account_state
    }

    /// Loads the last committed (e.g. just added but no necessarily verified) state for
    /// account given its ID.
    /// Returns both verified and committed states.
    pub async fn last_committed_state_for_account(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<((i64, Option<Account>), Option<Account>)> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        // Get the last certain state of the account.
        // Note that `account` can be `None` here (if it wasn't verified yet), since
        // we will update the committed changes below.
        let (last_block, account) = AccountSchema(&mut transaction)
            .account_and_last_block(account_id)
            .await?;

        let last_verified_block = BlockSchema(&mut transaction)
            .get_last_verified_confirmed_block()
            .await?
            .0 as i64;

        let account_balance_diff = sqlx::query_as!(
            StorageAccountUpdate,
            "
                SELECT * FROM account_balance_updates
                WHERE account_id = $1 AND block_number > $2
            ",
            i64::from(*account_id),
            last_verified_block
        )
        .fetch_all(transaction.conn())
        .await?;

        let account_creation_diff = sqlx::query_as!(
            StorageAccountCreation,
            "
                SELECT * FROM account_creates
                WHERE account_id = $1 AND block_number > $2
            ",
            i64::from(*account_id),
            last_verified_block
        )
        .fetch_all(transaction.conn())
        .await?;

        let account_pubkey_diff = sqlx::query_as!(
            StorageAccountPubkeyUpdate,
            "
                SELECT * FROM account_pubkey_updates
                WHERE account_id = $1 AND block_number > $2
            ",
            i64::from(*account_id),
            last_verified_block
        )
        .fetch_all(transaction.conn())
        .await?;
        let mint_nft_updates = sqlx::query_as!(
            StorageMintNFTUpdate,
            "
                SELECT * FROM mint_nft_updates
                WHERE creator_account_id = $1 AND block_number > $2
            ",
            *account_id as i32,
            last_verified_block
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
            account_diff.extend(mint_nft_updates.into_iter().map(StorageAccountDiff::from));
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
            .fold(account.clone(), Account::apply_update);

        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.account.last_committed_state_for_account",
            start.elapsed()
        );
        Ok(((last_block, account), account_state))
    }

    /// Loads the last verified state for the account (i.e. the one obtained in the last block
    /// which was both committed and verified).
    pub async fn last_verified_state_for_account(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<Option<Account>> {
        let start = Instant::now();
        let (_, account) = self.account_and_last_block(account_id).await?;
        metrics::histogram!(
            "sql.chain.account.last_verified_state_for_account",
            start.elapsed()
        );
        Ok(account)
    }

    /// Obtains the last verified state of the account.
    pub async fn account_and_last_block(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<(i64, Option<Account>)> {
        let start = Instant::now();
        let mut transaction = self.0.conn().begin().await?;

        // `accounts::table` is updated only after the block verification, so we should
        // just load the account with the provided ID.
        let maybe_account = sqlx::query_as!(
            StorageAccount,
            "
                SELECT * FROM accounts
                WHERE id = $1
            ",
            i64::from(*account_id)
        )
        .fetch_optional(&mut transaction)
        .await?;

        let result = if let Some(account) = maybe_account {
            let balances = sqlx::query_as!(
                StorageBalance,
                "
                    SELECT * FROM balances
                    WHERE account_id = $1
                ",
                i64::from(*account_id)
            )
            .fetch_all(&mut transaction)
            .await?;

            let last_block = account.last_block;
            let (_, mut account) = restore_account(&account, balances);
            let nfts: Vec<StorageNFT> = sqlx::query_as!(
                StorageNFT,
                "
                    SELECT nft.*, tokens.symbol FROM nft
                    INNER JOIN tokens
                    ON tokens.id = nft.token_id
                    WHERE creator_account_id = $1
                ",
                *account_id as i32
            )
            .fetch_all(&mut transaction)
            .await?;
            account.minted_nfts.extend(
                nfts.into_iter()
                    .map(|nft| (TokenId(nft.token_id as u32), nft.into())),
            );
            Ok((last_block, Some(account)))
        } else {
            Ok((0, None))
        };

        transaction.commit().await?;
        metrics::histogram!(
            "sql.chain.account.get_account_and_last_block",
            start.elapsed()
        );
        result
    }

    pub async fn account_id_by_address(
        &mut self,
        address: Address,
    ) -> QueryResult<Option<AccountId>> {
        let start = Instant::now();
        // Find the account ID in `account_creates` table.
        let result = sqlx::query!(
            r#"
                SELECT account_id FROM account_creates
                WHERE address = $1 AND is_create = $2
                ORDER BY block_number desc
                LIMIT 1
            "#,
            address.as_bytes(),
            true
        )
        .fetch_optional(self.0.conn())
        .await?;

        let account_id = result.map(|record| AccountId(record.account_id as u32));
        metrics::histogram!("sql.chain.account.account_id_by_address", start.elapsed());
        Ok(account_id)
    }

    pub async fn account_address_by_id(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<Option<Address>> {
        let start = Instant::now();
        // Find the account address in `account_creates` table.
        let result = sqlx::query!(
            "SELECT address FROM account_creates WHERE account_id = $1",
            i64::from(*account_id)
        )
        .fetch_optional(self.0.conn())
        .await?;

        let address = result.map(|record| Address::from_slice(&record.address));
        metrics::histogram!("sql.chain.account.account_address_by_id", start.elapsed());
        Ok(address)
    }

    /// Obtains the last committed block that affects the account.
    pub async fn last_committed_block_with_update_for_acc(
        &mut self,
        account_id: AccountId,
        block_number: BlockNumber,
    ) -> QueryResult<BlockNumber> {
        let start = Instant::now();

        let block_number = *block_number as i64;
        let block_number = sqlx::query!(
            "
            SELECT GREATEST(
                (SELECT block_number FROM account_balance_updates
                    WHERE account_id = $1 AND block_number >= $2 ORDER BY block_number DESC LIMIT 1
                ),
                (SELECT block_number FROM account_creates
                    WHERE account_id = $1 AND block_number >= $2 ORDER BY block_number DESC LIMIT 1
                ),
                (SELECT block_number FROM account_pubkey_updates
                    WHERE account_id = $1 AND block_number >= $2 ORDER BY block_number DESC LIMIT 1
                )
            )
    ",
            i64::from(*account_id),
            block_number,
        )
        .fetch_one(self.0.conn())
        .await?
        .greatest
        .unwrap_or(block_number);

        metrics::histogram!(
            "sql.chain.account.last_committed_block_with_update_for_acc",
            start.elapsed()
        );
        Ok(BlockNumber(block_number as u32))
    }

    pub async fn get_account_balance_for_block(
        &mut self,
        address: Address,
        block_number: BlockNumber,
        token_id: TokenId,
    ) -> QueryResult<BigUint> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let account_id = transaction
            .chain()
            .account_schema()
            .account_id_by_address(address)
            .await?;
        let account_id = match account_id {
            Some(id) => id,
            None => {
                return Ok(BigUint::zero());
            }
        };

        let record = sqlx::query!(
            r#"
                SELECT new_balance FROM account_balance_updates
                WHERE account_id = $1 AND block_number <= $2 AND coin_id = $3
                ORDER BY block_number DESC, update_order_id DESC
                LIMIT 1
            "#,
            i64::from(account_id.0),
            i64::from(block_number.0),
            token_id.0 as i32
        )
        .fetch_optional(transaction.conn())
        .await?;
        let last_balance_update: Option<BigDecimal> = record.map(|r| r.new_balance);

        let result = last_balance_update
            .map(|b| b.to_bigint().unwrap().to_biguint().unwrap())
            .unwrap_or_else(BigUint::zero);

        transaction.commit().await?;
        metrics::histogram!(
            "sql.chain.account.get_account_balance_for_block",
            start.elapsed()
        );

        Ok(result)
    }

    pub async fn get_account_nft_balance(&mut self, address: Address) -> QueryResult<u32> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        let account_id = transaction
            .chain()
            .account_schema()
            .account_id_by_address(address)
            .await?;
        let account_id = match account_id {
            Some(id) => id,
            None => {
                return Ok(0);
            }
        };
        if account_id == NFT_STORAGE_ACCOUNT_ID {
            // It is special account ID, just return 0 for it.
            return Ok(0);
        }

        let balance = sqlx::query!(
            r#"
                SELECT COUNT(*) FROM balances
                WHERE account_id = $1 AND coin_id >= $2 AND coin_id < $3 AND balance = 1
            "#,
            i64::from(account_id.0),
            MIN_NFT_TOKEN_ID as i32,
            NFT_TOKEN_ID.0 as i32
        )
        .fetch_one(transaction.conn())
        .await?
        .count
        .unwrap_or(0) as u32;

        transaction.commit().await?;
        metrics::histogram!("sql.chain.account.get_account_nft_balance", start.elapsed());

        Ok(balance)
    }

    pub async fn get_nft_owner(&mut self, token_id: TokenId) -> QueryResult<Option<AccountId>> {
        let start = Instant::now();

        let record = sqlx::query!(
            r#"
                SELECT account_id FROM balances
                WHERE coin_id = $1 AND balance = 1 AND account_id != $2
            "#,
            token_id.0 as i32,
            i64::from(NFT_STORAGE_ACCOUNT_ID.0)
        )
        .fetch_optional(self.0.conn())
        .await?;
        let owner_id = record.map(|record| AccountId(record.account_id as u32));

        metrics::histogram!("sql.chain.account.get_nft_owner", start.elapsed());
        Ok(owner_id)
    }
}
