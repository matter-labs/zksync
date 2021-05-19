// Built-in deps
use std::time::Instant;
// External imports
use sqlx::Acquire;
// Workspace imports
use zksync_types::{Account, AccountId, AccountUpdates, Address, TokenId};
// Local imports
use self::records::*;
use crate::diff::StorageAccountDiff;
use crate::{QueryResult, StorageProcessor};

pub mod records;
pub mod restore_account;
mod stored_state;

pub(crate) use self::restore_account::restore_account;
pub use self::stored_state::StoredAccountState;
use crate::tokens::records::StorageNFT;

/// Account schema contains interfaces to interact with the stored
/// ZKSync accounts.
#[derive(Debug)]
pub struct AccountSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> AccountSchema<'a, 'c> {
    /// Stores account type in the databse
    /// There are 2 types: Owned and CREATE2
    pub async fn set_account_type(
        &mut self,
        account_id: AccountId,
        account_type: EthAccountType,
    ) -> QueryResult<()> {
        let start = Instant::now();

        sqlx::query!(
            r#"
            INSERT INTO eth_account_types VALUES ( $1, $2 )
            ON CONFLICT (account_id) DO UPDATE SET account_type = $2
            "#,
            i64::from(*account_id),
            account_type as EthAccountType
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.chain.state.set_account_type", start.elapsed());
        Ok(())
    }

    /// Fetches account type from the database
    pub async fn account_type_by_id(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<Option<EthAccountType>> {
        let start = Instant::now();

        let result = sqlx::query_as!(
            StorageAccountType,
            r#"
            SELECT account_id, account_type as "account_type!: EthAccountType" 
            FROM eth_account_types WHERE account_id = $1
            "#,
            i64::from(*account_id)
        )
        .fetch_optional(self.0.conn())
        .await?;

        let account_type = result.map(|record| record.account_type as EthAccountType);
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
        let committed = self
            .last_committed_state_for_account(account_id)
            .await?
            .map(|a| (account_id, a));
        let verified = self
            .last_verified_state_for_account(account_id)
            .await?
            .map(|a| (account_id, a));

        metrics::histogram!("sql.chain.account.account_state_by_id", start.elapsed());
        Ok(StoredAccountState {
            committed,
            verified,
        })
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
    pub async fn last_committed_state_for_account(
        &mut self,
        account_id: AccountId,
    ) -> QueryResult<Option<Account>> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        // Get the last certain state of the account.
        // Note that `account` can be `None` here (if it wasn't verified yet), since
        // we will update the committed changes below.
        let (last_block, account) = AccountSchema(&mut transaction)
            .account_and_last_block(account_id)
            .await?;

        let account_balance_diff = sqlx::query_as!(
            StorageAccountUpdate,
            "
                SELECT * FROM account_balance_updates
                WHERE account_id = $1 AND block_number > $2
            ",
            i64::from(*account_id),
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
            i64::from(*account_id),
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
            i64::from(*account_id),
            last_block
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
            .fold(account, Account::apply_update);

        transaction.commit().await?;

        metrics::histogram!(
            "sql.chain.account.last_committed_state_for_account",
            start.elapsed()
        );
        Ok(account_state)
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
    async fn account_and_last_block(
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
                    SELECT * FROM nft 
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

    // This method does not have metrics, since it is used only for the
    // migration for the nft regenesis.
    // Remove this function once the regenesis is complete and the tool is not
    // needed anymore: ZKS-663
    pub async fn get_all_accounts(&mut self) -> QueryResult<Vec<StorageAccount>> {
        let result = sqlx::query_as!(StorageAccount, "SELECT * FROM accounts")
            .fetch_all(self.0.conn())
            .await?;

        Ok(result)
    }

    // This method does not have metrics, since it is used only for the
    // migration for the nft regenesis.
    // Remove this function once the regenesis is complete and the tool is not
    // needed anymore: ZKS-663
    pub async fn get_all_balances(&mut self) -> QueryResult<Vec<StorageBalance>> {
        let result = sqlx::query_as!(StorageBalance, "SELECT * FROM balances",)
            .fetch_all(self.0.conn())
            .await?;

        Ok(result)
    }
}
