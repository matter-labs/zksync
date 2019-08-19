#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;

use bigdecimal::BigDecimal;
use chrono::prelude::*;
use diesel::dsl::*;
use failure::Fail;
use models::node::block::{Block, ExecutedTx};
use models::node::{
    apply_updates, reverse_updates, tx::FranklinTx, Account, AccountId, AccountMap, AccountUpdate,
    AccountUpdates, BlockNumber, FranklinOp, Nonce, TokenId,
};
use models::{Action, ActionType, EncodedProof, Operation, TxMeta, ACTION_COMMIT, ACTION_VERIFY};
use serde_derive::{Deserialize, Serialize};
use std::cmp;
use std::convert::TryInto;

mod schema;

use crate::schema::*;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError, PooledConnection};

use serde_json::value::Value;
use std::env;

use diesel::sql_types::{Integer, Nullable, Text, Timestamp};
sql_function!(coalesce, Coalesce, (x: Nullable<Integer>, y: Integer) -> Integer);

use models::node::AccountAddress;

#[derive(Clone)]
pub struct ConnectionPool {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let max_size = env::var("DB_POOL_SIZE").unwrap_or_else(|_| "10".to_string());
        let max_size = max_size.parse().expect("DB_POOL_SIZE must be integer");
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let pool = Pool::builder()
            .max_size(max_size)
            .build(manager)
            .expect("Failed to create connection pool");

        Self { pool }
    }

    pub fn access_storage(&self) -> Result<StorageProcessor, PoolError> {
        let connection = self.pool.get()?;
        Ok(StorageProcessor::from_pool(connection))
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Identifiable, Insertable, QueryableByName, Queryable)]
#[table_name = "accounts"]
struct StorageAccount {
    pub id: i32,
    pub last_block: i32,
    pub nonce: i64,
    pub address: Vec<u8>,
}

#[derive(Identifiable, Insertable, QueryableByName, Queryable, Associations)]
#[belongs_to(StorageAccount, foreign_key = "account_id")]
#[primary_key(account_id, coin_id)]
#[table_name = "balances"]
struct StorageBalance {
    pub account_id: i32,
    pub coin_id: i32,
    pub balance: BigDecimal,
}

#[derive(Insertable, QueryableByName, Queryable, Serialize, Deserialize)]
#[table_name = "tokens"]
pub struct Token {
    pub id: i32,
    pub address: String,
    pub symbol: Option<String>,
}

#[derive(Debug, Insertable)]
#[table_name = "account_balance_updates"]
struct StorageAccountUpdateInsert {
    pub account_id: i32,
    pub block_number: i32,
    pub coin_id: i32,
    pub old_balance: BigDecimal,
    pub new_balance: BigDecimal,
    pub old_nonce: i64,
    pub new_nonce: i64,
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "account_balance_updates"]
struct StorageAccountUpdate {
    balance_update_id: i32,
    pub account_id: i32,
    pub block_number: i32,
    pub coin_id: i32,
    pub old_balance: BigDecimal,
    pub new_balance: BigDecimal,
    pub old_nonce: i64,
    pub new_nonce: i64,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "account_creates"]
struct StorageAccountCreation {
    account_id: i32,
    is_create: bool,
    block_number: i32,
    address: Vec<u8>,
    nonce: i64,
}

#[derive(Debug, Insertable)]
#[table_name = "executed_transactions"]
struct NewExecutedTransaction {
    block_number: i64,
    tx_hash: Vec<u8>,
    operation: Option<Value>,
    success: bool,
    fail_reason: Option<String>,
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "executed_transactions"]
struct StoredExecutedTransaction {
    id: i32,
    block_number: i64,
    tx_hash: Vec<u8>,
    operation: Option<Value>,
    success: bool,
    fail_reason: Option<String>,
}

impl NewExecutedTransaction {
    fn prepare_stored_tx(exec_tx: &ExecutedTx, block: BlockNumber) -> Self {
        Self {
            block_number: block as i64,
            tx_hash: exec_tx.tx.hash(),
            operation: exec_tx.op.clone().map(|o| serde_json::to_value(o).unwrap()),
            success: exec_tx.success,
            fail_reason: exec_tx.fail_reason.clone(),
        }
    }
}

#[derive(Debug)]
enum StorageAccountDiff {
    BalanceUpdate(StorageAccountUpdate),
    Create(StorageAccountCreation),
    Delete(StorageAccountCreation),
}

impl From<StorageAccountUpdate> for StorageAccountDiff {
    fn from(update: StorageAccountUpdate) -> Self {
        StorageAccountDiff::BalanceUpdate(update)
    }
}

impl From<StorageAccountCreation> for StorageAccountDiff {
    fn from(create: StorageAccountCreation) -> Self {
        if create.is_create {
            StorageAccountDiff::Create(create)
        } else {
            StorageAccountDiff::Delete(create)
        }
    }
}

impl Into<(u32, AccountUpdate)> for StorageAccountDiff {
    fn into(self) -> (u32, AccountUpdate) {
        match self {
            StorageAccountDiff::BalanceUpdate(upd) => (
                upd.account_id as u32,
                AccountUpdate::UpdateBalance {
                    old_nonce: upd.old_nonce as u32,
                    new_nonce: upd.new_nonce as u32,
                    balance_update: (upd.coin_id as TokenId, upd.old_balance, upd.new_balance),
                },
            ),
            StorageAccountDiff::Create(upd) => (
                upd.account_id as u32,
                AccountUpdate::Create {
                    nonce: upd.nonce as u32,
                    address: AccountAddress {
                        data: upd.address.as_slice().try_into().unwrap(),
                    },
                },
            ),
            StorageAccountDiff::Delete(upd) => (
                upd.account_id as u32,
                AccountUpdate::Delete {
                    nonce: upd.nonce as u32,
                    address: AccountAddress {
                        data: upd.address.as_slice().try_into().unwrap(),
                    },
                },
            ),
        }
    }
}

impl StorageAccountDiff {
    fn nonce(&self) -> i64 {
        *match self {
            StorageAccountDiff::BalanceUpdate(StorageAccountUpdate { old_nonce, .. }) => old_nonce,
            StorageAccountDiff::Create(StorageAccountCreation { nonce, .. }) => nonce,
            StorageAccountDiff::Delete(StorageAccountCreation { nonce, .. }) => nonce,
        }
    }

    fn cmp_nonce(&self, other: &Self) -> std::cmp::Ordering {
        let type_cmp_number = |diff: &StorageAccountDiff| -> u32 {
            match diff {
                StorageAccountDiff::Create(..) => 0,
                StorageAccountDiff::BalanceUpdate(..) => 1,
                StorageAccountDiff::Delete(..) => 2,
            }
        };

        self.nonce()
            .cmp(&other.nonce())
            .then(type_cmp_number(self).cmp(&type_cmp_number(other)))
    }

    fn block_number(&self) -> i32 {
        *match self {
            StorageAccountDiff::BalanceUpdate(StorageAccountUpdate { block_number, .. }) => {
                block_number
            }
            StorageAccountDiff::Create(StorageAccountCreation { block_number, .. }) => block_number,
            StorageAccountDiff::Delete(StorageAccountCreation { block_number, .. }) => block_number,
        }
    }
}

#[derive(Insertable)]
#[table_name = "operations"]
struct NewOperation {
    pub data: Value,
    pub block_number: i32,
    pub action_type: String,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "operations"]
pub struct StoredOperation {
    pub id: i32,
    pub data: serde_json::Value,
    pub addr: String,
    pub nonce: i32,
    pub block_number: i32,
    pub action_type: String,
    pub tx_hash: Option<String>,
    pub created_at: NaiveDateTime,
}

impl StoredOperation {
    pub fn get_meta(&self) -> TxMeta {
        TxMeta {
            addr: self.addr.clone(),
            nonce: self.nonce as u32,
        }
    }

    pub fn into_op(self, conn: &StorageProcessor) -> QueryResult<Operation> {
        let meta = self.get_meta();

        let debug_data = format!("data: {}", &self.data);

        // let op: Result<Operation, serde_json::Error> = serde_json::from_value(self.data);
        let op: Result<Operation, serde_json::Error> = serde_json::from_str(&self.data.to_string());

        if let Err(err) = &op {
            debug!("Error: {} on {}", err, debug_data)
        }

        let mut op = op.expect("Operation deserialization");
        op.tx_meta = Some(meta);
        op.id = Some(self.id);

        if op.accounts_updated.is_empty() {
            let updates = conn.load_state_diff_for_block(op.block.block_number)?;
            op.accounts_updated = updates;
        };

        Ok(op)
    }
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "proofs"]
pub struct NewProof {
    pub block_number: i32,
    pub proof: serde_json::Value,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "proofs"]
pub struct StoredProof {
    pub block_number: i32,
    pub proof: serde_json::Value,
    pub created_at: NaiveDateTime,
}

// Every time before a prover worker starts generating the proof, a prover run is recorded for monitoring purposes
#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "prover_runs"]
pub struct ProverRun {
    pub id: i32,
    pub block_number: i32,
    pub worker: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "active_provers"]
pub struct ActiveProver {
    pub id: i32,
    pub worker: String,
    pub created_at: NaiveDateTime,
    pub stopped_at: Option<NaiveDateTime>,
}

#[derive(Debug, QueryableByName)]
pub struct IntegerNumber {
    #[sql_type = "Integer"]
    pub integer_value: i32,
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "server_config"]
pub struct ServerConfig {
    pub id: bool,
    pub contract_addr: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, QueryableByName)]
pub struct BlockDetails {
    #[sql_type = "Integer"]
    pub block_number: i32,

    #[sql_type = "Text"]
    pub new_state_root: String,

    #[sql_type = "Nullable<Text>"]
    pub commit_tx_hash: Option<String>,

    #[sql_type = "Nullable<Text>"]
    pub verify_tx_hash: Option<String>,

    #[sql_type = "Timestamp"]
    pub committed_at: NaiveDateTime,

    #[sql_type = "Nullable<Timestamp>"]
    pub verified_at: Option<NaiveDateTime>,
}

#[derive(Debug, Insertable)]
#[table_name = "mempool"]
struct InsertTx {
    hash: Vec<u8>,
    primary_account_address: Vec<u8>,
    nonce: i64,
    tx: Value,
}

#[derive(Debug, Queryable)]
struct ReadTx {
    hash: Vec<u8>,
    primary_account_address: Vec<u8>,
    nonce: i64,
    tx: Value,
    created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Fail)]
pub enum TxAddError {
    #[fail(display = "Tx nonce is too low.")]
    NonceTooLow,
    #[fail(display = "Tx signature is incorrect.")]
    InvalidSignature,
}

enum ConnectionHolder {
    Pooled(PooledConnection<ConnectionManager<PgConnection>>),
    Direct(PgConnection),
}

pub struct StorageProcessor {
    conn: ConnectionHolder,
}

fn restore_account(
    stored_account: StorageAccount,
    stored_balances: Vec<StorageBalance>,
) -> (AccountId, Account) {
    let mut account = Account::default();
    for b in stored_balances.into_iter() {
        assert_eq!(b.account_id, stored_account.id);
        account.set_balance(b.coin_id as TokenId, b.balance);
    }
    account.nonce = stored_account.nonce as u32;
    account.address = AccountAddress {
        data: stored_account.address.as_slice().try_into().unwrap(),
    };
    (stored_account.id as u32, account)
}

impl StorageProcessor {
    pub fn establish_connection() -> ConnectionResult<Self> {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let connection = PgConnection::establish(&database_url)?; //.expect(&format!("Error connecting to {}", database_url));
        Ok(Self {
            conn: ConnectionHolder::Direct(connection),
        })
    }

    pub fn from_pool(conn: PooledConnection<ConnectionManager<PgConnection>>) -> Self {
        Self {
            conn: ConnectionHolder::Pooled(conn),
        }
    }

    fn conn(&self) -> &PgConnection {
        match self.conn {
            ConnectionHolder::Pooled(ref conn) => conn,
            ConnectionHolder::Direct(ref conn) => conn,
        }
    }

    pub fn load_config(&self) -> QueryResult<ServerConfig> {
        use crate::schema::server_config::dsl::*;
        server_config.first(self.conn())
    }

    /// Execute an operation: store op, modify state accordingly, load additional data and meta tx info
    /// - Commit => store account updates
    /// - Verify => apply account updates
    pub fn execute_operation(&self, op: &Operation) -> QueryResult<Operation> {
        self.conn().transaction(|| {
            match &op.action {
                Action::Commit => {
                    self.commit_state_update(op.block.block_number, &op.accounts_updated)?;
                    self.save_block_transactions(&op.block)?;
                }
                Action::Verify { .. } => self.apply_state_update(op.block.block_number)?,
            };

            // NOTE: tx meta is inserted automatically with sender addr and next expected nonce
            // see SQL migration code for `operations` table
            let stored: StoredOperation = diesel::insert_into(operations::table)
                .values(&NewOperation {
                    block_number: op.block.block_number as i32,
                    action_type: op.action.to_string(),
                    data: serde_json::to_value(&op).unwrap(),
                })
                .get_result(self.conn())?;
            stored.into_op(self)
        })
    }

    pub fn save_operation_tx_hash(&self, op_id: i32, hash: String) -> QueryResult<()> {
        use crate::schema::operations::dsl::*;
        let target = operations.filter(id.eq(op_id));
        diesel::update(target)
            .set(tx_hash.eq(hash))
            .execute(self.conn())
            .map(|_| ())
    }

    pub fn save_block_transactions(&self, block: &Block) -> QueryResult<()> {
        for block_tx in block.block_transactions.iter() {
            let stored_tx = NewExecutedTransaction::prepare_stored_tx(block_tx, block.block_number);
            diesel::insert_into(executed_transactions::table)
                .values(&stored_tx)
                .execute(self.conn())?;
        }
        Ok(())
    }

    pub fn get_block_operations(&self, block: BlockNumber) -> QueryResult<Vec<FranklinOp>> {
        let executed_txs: Vec<_> = executed_transactions::table
            .filter(executed_transactions::block_number.eq(block as i64))
            .load::<StoredExecutedTransaction>(self.conn())?;
        Ok(executed_txs
            .into_iter()
            .filter_map(|exec_tx| {
                exec_tx
                    .operation
                    .map(|op| serde_json::from_value(op).expect("stored op"))
            })
            .collect())
    }

    pub fn commit_state_update(
        &self,
        block_number: u32,
        accounts_updated: &[(u32, AccountUpdate)],
    ) -> QueryResult<()> {
        self.conn().transaction(|| {
            for (id, upd) in accounts_updated.iter() {
                debug!(
                    "Committing state update for account {} in block {}",
                    id, block_number
                );
                match *upd {
                    AccountUpdate::Create { ref address, nonce } => {
                        diesel::insert_into(account_creates::table)
                            .values(&StorageAccountCreation {
                                account_id: *id as i32,
                                is_create: true,
                                block_number: block_number as i32,
                                address: address.data.to_vec(),
                                nonce: i64::from(nonce),
                            })
                            .execute(self.conn())?;
                    }
                    AccountUpdate::Delete { ref address, nonce } => {
                        diesel::insert_into(account_creates::table)
                            .values(&StorageAccountCreation {
                                account_id: *id as i32,
                                is_create: false,
                                block_number: block_number as i32,
                                address: address.data.to_vec(),
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
                                account_id: *id as i32,
                                block_number: block_number as i32,
                                coin_id: token as i32,
                                old_balance: old_balance.clone(),
                                new_balance: new_balance.clone(),
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
        info!("Applying state update for block: {}", block_number);
        self.conn().transaction(|| {
            let account_balance_diff = account_balance_updates::table
                .filter(account_balance_updates::block_number.eq(&(block_number as i32)))
                .load::<StorageAccountUpdate>(self.conn())?;

            let account_creation_diff = account_creates::table
                .filter(account_creates::block_number.eq(&(block_number as i32)))
                .load::<StorageAccountCreation>(self.conn())?;

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
                account_diff.sort_by(|l, r| l.cmp_nonce(r));
                account_diff
            };

            debug!("Sorted account update list: {:?}", account_updates);

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
                        };
                        insert_into(accounts::table)
                            .values(&storage_account)
                            .execute(self.conn())?;
                    }

                    StorageAccountDiff::Delete(upd) => {
                        delete(accounts::table.filter(accounts::id.eq(upd.account_id)))
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

            // Fetch updates from blocks: verif_block +/- 1, ... , block
            if let Some((block, state_diff)) = self.load_state_diff(verif_block, block)? {
                debug!("Loaded state diff: {:#?}", state_diff);
                apply_updates(&mut accounts, state_diff.clone());
                Ok((block, accounts))
            } else {
                Ok((verif_block, accounts))
            }
        })
    }

    pub fn load_verified_state(&self) -> QueryResult<(u32, AccountMap)> {
        self.conn().transaction(|| {
            let accounts: Vec<StorageAccount> = accounts::table.load(self.conn())?;
            let balances: Vec<Vec<StorageBalance>> = StorageBalance::belonging_to(&accounts)
                .load(self.conn())?
                .grouped_by(&accounts);

            let last_block = accounts
                .iter()
                .map(|a| a.last_block as u32)
                .max()
                .unwrap_or(0);

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
            let (to_block, unbounded) = if let Some(to_block) = to_block {
                (to_block, false)
            } else {
                (0, true)
            };

            let (time_forward, start_block, end_block) = if unbounded {
                (true, from_block, 0)
            } else {
                (
                    from_block <= to_block,
                    cmp::min(from_block, to_block),
                    cmp::max(from_block, to_block),
                )
            };

            let account_balance_diff = account_balance_updates::table
                .filter(
                    account_balance_updates::block_number
                        .gt(&(start_block as i32))
                        .and(
                            account_balance_updates::block_number
                                .le(&(end_block as i32))
                                .or(unbounded),
                        ),
                )
                .load::<StorageAccountUpdate>(self.conn())?;
            let account_creation_diff = account_creates::table
                .filter(
                    account_creates::block_number.gt(&(start_block as i32)).and(
                        account_creates::block_number
                            .le(&(end_block as i32))
                            .or(unbounded),
                    ),
                )
                .load::<StorageAccountCreation>(self.conn())?;

            debug!(
                "Loading state diff: forward: {}, start_block: {}, end_block: {}, unbounded: {}",
                time_forward, start_block, end_block, unbounded
            );
            debug!("Loaded account balance diff: {:#?}", account_balance_diff);
            debug!("Loaded account creation diff: {:#?}", account_creation_diff);

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
                let last_block = account_diff
                    .iter()
                    .map(|acc| acc.block_number())
                    .max()
                    .unwrap_or(0);
                account_diff.sort_by(|l, r| l.cmp_nonce(r));
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

            let block_after_updates = if time_forward { last_block } else { to_block };

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

    /// Sets up `op_config` to new address and nonce for scheduling ETH transactions for operations
    /// If current nonce from ETH netorkw is higher, nonce config is fast-forwarded to the new value
    pub fn prepare_nonce_scheduling(&self, sender: &str, current_nonce: Nonce) -> QueryResult<()> {
        // The code below does this:
        // next_nonce = max( current_nonce, max(nonces from ops scheduled for this sender) + 1 )
        diesel::sql_query(
            format!(
                "
            UPDATE op_config 
            SET addr = '{addr}', next_nonce = s.next_nonce
            FROM (
                SELECT max(t.next_nonce) AS next_nonce
                FROM (
                    SELECT max(nonce) + 1 AS next_nonce FROM operations WHERE addr = '{addr}' 
                    UNION SELECT {current_nonce} AS next_nonce
                ) t
            ) s",
                addr = sender,
                current_nonce = current_nonce as i32
            )
            .as_str(),
        )
        .execute(self.conn())
        .map(|_| ())
    }

    pub fn load_stored_op_with_block_number(
        &self,
        block_number: BlockNumber,
        action_type: ActionType,
    ) -> Option<StoredOperation> {
        use crate::schema::operations::dsl;
        dsl::operations
            .filter(dsl::block_number.eq(block_number as i32))
            .filter(dsl::action_type.eq(action_type.to_string().as_str()))
            .get_result(self.conn())
            .ok()
    }

    pub fn load_block_range(
        &self,
        max_block: BlockNumber,
        limit: u32,
    ) -> QueryResult<Vec<BlockDetails>> {
        let query = format!(
            "
            with committed as (
                select 
                    data -> 'block' ->> 'new_root_hash' as new_state_root,    
                    block_number,
                    tx_hash as commit_tx_hash,
                    created_at as committed_at
                from operations
                where 
                    block_number <= {max_block}
                    and action_type = 'Commit'
                order by block_number desc
                limit {limit}
            )
            select 
                committed.*, 
                verified.tx_hash as verify_tx_hash,
                verified.created_at as verified_at
            from committed
            left join operations verified
            on
                committed.block_number = verified.block_number
                and action_type = 'Verify'
            order by committed.block_number desc
        ",
            max_block = max_block as i32,
            limit = limit as i32
        );
        diesel::sql_query(query).load(self.conn())
    }

    pub fn handle_search(&self, query: String) -> Option<BlockDetails> {
        let block_number = query.parse::<i32>().unwrap_or(i32::max_value());
        let l_query = query.to_lowercase();
        let has_prefix = l_query.starts_with("0x");
        let prefix = "0x".to_owned();
        let query_with_prefix = if has_prefix {
            l_query
        } else {
            format!("{}{}", prefix, l_query)
        };
        let sql_query = format!(
            "
            with committed as (
                select 
                    data -> 'block' ->> 'new_root_hash' as new_state_root,    
                    block_number,
                    tx_hash as commit_tx_hash,
                    created_at as committed_at
                from operations
                where action_type = 'Commit'
                order by block_number desc
            )
            select 
                committed.*, 
                verified.tx_hash as verify_tx_hash,
                verified.created_at as verified_at
            from committed
            left join operations verified
            on
                committed.block_number = verified.block_number
                and action_type = 'Verify'
            where false
                or lower(commit_tx_hash) = $1
                or lower(verified.tx_hash) = $1
                or lower(new_state_root) = $1
                or committed.block_number = {block_number}
            order by committed.block_number desc
            limit 1
        ",
            block_number = block_number as i32
        );
        diesel::sql_query(sql_query)
            .bind::<Text, _>(query_with_prefix)
            .get_result(self.conn())
            .ok()
    }

    // pub fn load_stored_ops_in_blocks_range(&self, max_block: BlockNumber, limit: u32, action_type: ActionType) -> Vec<StoredOperation> {
    //     let query = format!("
    //         SELECT * FROM operations
    //         WHERE block_number <= {max_block} AND action_type = '{action_type}'
    //         ORDER BY block_number
    //         DESC
    //         LIMIT {limit}
    //     ", max_block = max_block as i32, limit = limit as i32, action_type = action_type.to_string());
    //     let r = diesel::sql_query(query)
    //         .load(self.conn());
    //     r.unwrap_or(vec![])
    // }

    pub fn load_commit_op(&self, block_number: BlockNumber) -> Option<Operation> {
        let op = self.load_stored_op_with_block_number(block_number, ActionType::COMMIT);
        op.and_then(|r| r.into_op(self).ok())
    }

    pub fn load_committed_block(&self, block_number: BlockNumber) -> Option<Block> {
        let op = self.load_commit_op(block_number);
        op.and_then(|r| Some(r.block))
    }

    pub fn load_unsent_ops(&self, current_nonce: Nonce) -> QueryResult<Vec<Operation>> {
        use crate::schema::operations::dsl;
        self.conn().transaction(|| {
            let ops: Vec<StoredOperation> = dsl::operations
                .filter(dsl::nonce.ge(current_nonce as i32)) // WHERE nonce >= current_nonce
                .load(self.conn())?;
            ops.into_iter().map(|o| o.into_op(self)).collect()
        })
    }

    pub fn load_unverified_commitments(&self) -> QueryResult<Vec<Operation>> {
        self.conn().transaction(|| {
            // // https://docs.diesel.rs/diesel/query_dsl/trait.QueryDsl.html
            // use crate::schema::operations::dsl::{*};
            // let ops: Vec<StoredOperation> = operations
            //     .filter(action_type.eq(ACTION_COMMIT)
            //             .and(block_number.gt(
            //                 coalesce(
            //                     operations
            //                     .select(block_number)
            //                     .filter(action_type.eq(ACTION_VERIFY))
            //                     .single_value(), 0)
            //             ))
            //     )
            //     .load(self.conn())?;

            let ops: Vec<StoredOperation> = diesel::sql_query(
                "
                SELECT * FROM operations
                WHERE action_type = 'Commit'
                AND block_number > (
                    SELECT COALESCE(max(block_number), 0)  
                    FROM operations 
                    WHERE action_type = 'Verify'
                )
            ",
            )
            .load(self.conn())?;
            ops.into_iter().map(|o| o.into_op(self)).collect()
        })
    }

    fn load_number(&self, query: &str) -> QueryResult<i32> {
        diesel::sql_query(query)
            .get_result::<IntegerNumber>(self.conn())
            .map(|r| r.integer_value)
    }

    pub fn load_last_committed_deposit_batch(&self) -> QueryResult<i32> {
        self.load_number("
            SELECT COALESCE(max((data->'block'->'block_data'->>'batch_number')::int), -1) as integer_value FROM operations 
            WHERE data->'action'->>'type' = 'Commit' 
            AND data->'block'->'block_data'->>'type' = 'Deposit'
        ")
    }

    pub fn load_last_committed_exit_batch(&self) -> QueryResult<i32> {
        self.load_number("
            SELECT COALESCE(max((data->'block'->'block_data'->>'batch_number')::int), -1) as integer_value FROM operations 
            WHERE data->'action'->>'type' = 'Commit' 
            AND data->'block'->'block_data'->>'type' = 'Exit'
        ")
    }

    fn get_account_and_last_block(
        &self,
        account_id: AccountId,
    ) -> QueryResult<(i32, Option<Account>)> {
        self.conn().transaction(|| {
            if let Some(account) = accounts::table
                .find(account_id as i32)
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
    pub fn account_state_by_address(
        &self,
        address: &AccountAddress,
    ) -> QueryResult<(Option<AccountId>, Option<Account>, Option<Account>)> {
        let account_create_record = account_creates::table
            .filter(account_creates::address.eq(address.data.to_vec()))
            .filter(account_creates::is_create.eq(true))
            .order(account_creates::block_number.desc())
            .first::<StorageAccountCreation>(self.conn())
            .optional()?;

        let account_id = if let Some(account_create_record) = account_create_record {
            account_create_record.account_id as AccountId
        } else {
            return Ok((None, None, None));
        };

        let commited_state = self.last_committed_state_for_account(account_id)?;
        let verified_state = self.last_verified_state_for_account(account_id)?;
        Ok((Some(account_id), verified_state, commited_state))
    }

    pub fn last_committed_state_for_account(
        &self,
        account_id: AccountId,
    ) -> QueryResult<Option<models::node::Account>> {
        self.conn().transaction(|| {
            let (last_block, account) = self.get_account_and_last_block(account_id)?;

            let account_balance_diff: Vec<StorageAccountUpdate> = {
                account_balance_updates::table
                    .filter(account_balance_updates::account_id.eq(&(account_id as i32)))
                    .filter(account_balance_updates::block_number.gt(&last_block))
                    .load::<StorageAccountUpdate>(self.conn())?
            };

            let account_creation_diff: Vec<StorageAccountCreation> = {
                account_creates::table
                    .filter(account_creates::account_id.eq(&(account_id as i32)))
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
                account_diff.sort_by(|l, r| l.cmp_nonce(r));
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

    pub fn count_outstanding_proofs(&self, after_block: BlockNumber) -> QueryResult<u32> {
        use crate::schema::transactions::dsl::*;
        let count: i64 = transactions
            .select(count_star())
            .filter(block_number.gt(after_block as i32))
            .first(self.conn())?;
        Ok(count as u32)
    }

    pub fn count_total_transactions(&self) -> QueryResult<u32> {
        use crate::schema::transactions::dsl::*;
        let count: i64 = transactions.select(count_star()).first(self.conn())?;
        Ok(count as u32)
    }

    pub fn get_last_committed_block(&self) -> QueryResult<BlockNumber> {
        // use crate::schema::account_updates::dsl::*;
        // account_updates
        //     .select(max(block_number))
        //     .get_result::<Option<i32>>(self.conn())
        //     .map(|max| max.unwrap_or(0))

        use crate::schema::operations::dsl::*;
        operations
            .select(max(block_number))
            .filter(action_type.eq(ACTION_COMMIT))
            .get_result::<Option<i32>>(self.conn())
            .map(|max| max.unwrap_or(0) as BlockNumber)

        //self.load_number("SELECT COALESCE(max(block_number), 0) AS integer_value FROM account_updates")
    }

    pub fn get_last_verified_block(&self) -> QueryResult<BlockNumber> {
        // use crate::schema::accounts::dsl::*;
        // accounts
        //     .select(max(last_block))
        //     .get_result::<Option<i32>>(self.conn())
        //     .map(|max| max.unwrap_or(0))

        use crate::schema::operations::dsl::*;
        operations
            .select(max(block_number))
            .filter(action_type.eq(ACTION_VERIFY))
            .get_result::<Option<i32>>(self.conn())
            .map(|max| max.unwrap_or(0) as BlockNumber)

        //self.load_number("SELECT COALESCE(max(last_block), 0) AS integer_value FROM accounts")
    }

    pub fn fetch_prover_job(
        &self,
        worker_: &str,
        timeout_seconds: usize,
    ) -> QueryResult<Option<ProverRun>> {
        self.conn().transaction(|| {
            sql_query("LOCK TABLE prover_runs IN EXCLUSIVE MODE").execute(self.conn())?;
            let job: Option<BlockNumber> = diesel::sql_query(format!("
                    SELECT min(block_number) as integer_value FROM operations o
                    WHERE action_type = 'Commit'
                    AND block_number >
                        (SELECT COALESCE(max(block_number),0) FROM operations WHERE action_type = 'Verify')
                    AND NOT EXISTS 
                        (SELECT * FROM proofs WHERE block_number = o.block_number)
                    AND NOT EXISTS
                        (SELECT * FROM prover_runs 
                            WHERE block_number = o.block_number AND (now() - updated_at) < interval '{} seconds')
                ", timeout_seconds))
                .get_result::<Option<IntegerNumber>>(self.conn())?
                .map(|i| i.integer_value as BlockNumber);
            if let Some(block_number_) = job {
                // let to_store = NewProverRun{
                //     block_number: block_number as i32,
                //     worker: worker.to_string(),
                // };
                use crate::schema::prover_runs::dsl::*;
                let inserted: ProverRun = insert_into(prover_runs)
                    .values(&vec![(
                        block_number.eq(block_number_ as i32),
                        worker.eq(worker_.to_string())
                    )])
                    .get_result(self.conn())?;
                Ok(Some(inserted))
            } else {
                Ok(None)
            }
        })
    }

    pub fn update_prover_job(&self, job_id: i32) -> QueryResult<()> {
        use crate::schema::prover_runs::dsl::*;

        let target = prover_runs.filter(id.eq(job_id));
        diesel::update(target)
            .set(updated_at.eq(now))
            .execute(self.conn())
            .map(|_| ())
    }

    pub fn register_prover(&self, worker_: &str) -> QueryResult<i32> {
        use crate::schema::active_provers::dsl::*;
        let inserted: ActiveProver = insert_into(active_provers)
            .values(&vec![(worker.eq(worker_.to_string()))])
            .get_result(self.conn())?;
        Ok(inserted.id)
    }

    pub fn record_prover_stop(&self, prover_id: i32) -> QueryResult<()> {
        use crate::schema::active_provers::dsl::*;

        let target = active_provers.filter(id.eq(prover_id));
        diesel::update(target)
            .set(stopped_at.eq(now))
            .execute(self.conn())
            .map(|_| ())
    }

    /// Store the timestamp of the prover finish and the proof
    pub fn store_proof(
        &self,
        block_number: BlockNumber,
        proof: &EncodedProof,
    ) -> QueryResult<usize> {
        let to_store = NewProof {
            block_number: block_number as i32,
            proof: serde_json::to_value(proof).unwrap(),
        };
        use crate::schema::proofs::dsl::proofs;
        insert_into(proofs).values(&to_store).execute(self.conn())
    }

    pub fn load_proof(&self, block_number: BlockNumber) -> QueryResult<EncodedProof> {
        use crate::schema::proofs::dsl;
        let stored: StoredProof = dsl::proofs
            .filter(dsl::block_number.eq(block_number as i32))
            .get_result(self.conn())?;
        Ok(serde_json::from_value(stored.proof).unwrap())
    }

    pub fn store_token(&self, id: TokenId, address: &str, symbol: Option<&str>) -> QueryResult<()> {
        let new_token = Token {
            id: id as i32,
            address: address.to_string(),
            symbol: symbol.map(String::from),
        };
        diesel::insert_into(tokens::table)
            .values(&new_token)
            .on_conflict_do_nothing()
            .execute(self.conn())
            .map(drop)
    }

    pub fn load_tokens(&self) -> QueryResult<Vec<Token>> {
        let tokens = tokens::table
            .order(tokens::id.asc())
            .load::<Token>(self.conn())?;
        Ok(tokens.into_iter().collect())
    }

    pub fn mempool_get_size(&self) -> QueryResult<usize> {
        mempool::table
            .select(count(mempool::primary_account_address))
            .execute(self.conn())
    }

    pub fn mempool_add_tx(&self, tx: &FranklinTx) -> QueryResult<Result<(), TxAddError>> {
        if !tx.check_signature() {
            return Ok(Err(TxAddError::InvalidSignature));
        }

        let (_, _, commited_state) = self.account_state_by_address(&tx.account())?;
        let lowest_possible_nonce = commited_state.map(|a| a.nonce as u32).unwrap_or_default();
        if tx.nonce() < lowest_possible_nonce {
            return Ok(Err(TxAddError::NonceTooLow));
        }

        let tx_failed = executed_transactions::table
            .filter(executed_transactions::tx_hash.eq(tx.hash()))
            .filter(executed_transactions::success.eq(false))
            .first::<StoredExecutedTransaction>(self.conn())
            .optional()?;
        // Remove executed tx from db
        if let Some(tx_failed) = tx_failed {
            diesel::delete(
                executed_transactions::table.filter(executed_transactions::id.eq(tx_failed.id)),
            )
            .execute(self.conn())?;
        } else {
            // TODO Check tx and add only txs with valid nonce.
            insert_into(mempool::table)
                .values(&InsertTx {
                    hash: tx.hash(),
                    primary_account_address: tx.account().data.to_vec(),
                    nonce: i64::from(tx.nonce()),
                    tx: serde_json::to_value(tx).unwrap(),
                })
                .execute(self.conn())
                .map(drop)?;
        }

        Ok(Ok(()))
    }

    pub fn get_pending_txs(&self, address: &AccountAddress) -> QueryResult<Vec<FranklinTx>> {
        let (_, _, commited_state) = self.account_state_by_address(address)?;
        let commited_nonce = commited_state.map(|a| a.nonce as i64).unwrap_or_default();

        let pending_txs: Vec<_> = mempool::table
            .filter(mempool::primary_account_address.eq(address.data.to_vec()))
            .filter(mempool::nonce.ge(commited_nonce))
            .left_join(
                executed_transactions::table.on(executed_transactions::tx_hash.eq(mempool::hash)),
            )
            .filter(executed_transactions::tx_hash.is_null())
            .load::<(ReadTx, Option<StoredExecutedTransaction>)>(self.conn())?;

        Ok(pending_txs
            .into_iter()
            .map(|(stored_tx, _)| serde_json::from_value(stored_tx.tx).unwrap())
            .collect())
    }

    pub fn mempool_get_txs(&self, max_size: usize) -> QueryResult<Vec<FranklinTx>> {
        //TODO use "gaps and islands" sql solution for this.
        let stored_txs: Vec<_> = mempool::table
            .left_join(
                executed_transactions::table.on(executed_transactions::tx_hash.eq(mempool::hash)),
            )
            .filter(executed_transactions::tx_hash.is_null())
            .left_join(accounts::table.on(accounts::address.eq(mempool::primary_account_address)))
            .filter(
                accounts::nonce
                    .is_null()
                    .or(accounts::nonce.ge(mempool::nonce)),
            )
            .order(mempool::created_at.asc())
            .limit(max_size as i64)
            .load::<(
                ReadTx,
                Option<StoredExecutedTransaction>,
                Option<StorageAccount>,
            )>(self.conn())?;

        Ok(stored_txs
            .into_iter()
            .map(|stored_tx| serde_json::from_value(stored_tx.0.tx).unwrap())
            .collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use diesel::Connection;

    #[test]
    fn test_store_proof() {
        let pool = ConnectionPool::new();
        let conn = pool.access_storage().unwrap();
        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test

        assert!(conn.load_proof(1).is_err());

        let proof = EncodedProof::default();
        assert!(conn.store_proof(1, &proof).is_ok());

        let loaded = conn.load_proof(1).expect("must load proof");
        assert_eq!(loaded, proof);
    }

    //        #[test]
    //        fn test_store_commited_updates() {
    //            let _ = env_logger::try_init();
    //
    //            let pool = ConnectionPool::new();
    //            let conn = pool.access_storage().unwrap();
    //            conn.conn().begin_test_transaction().unwrap(); // this will revert db after test
    //
    //            let mut account_map = AccountMap::default();
    //
    //            let (_, state) = conn.load_committed_state(None).unwrap();
    //            assert_eq!(
    //                state
    //                    .into_iter()
    //                    .collect::<Vec<(u32, models::plasma::account::Account)>>(),
    //                account_map
    //                    .clone()
    //                    .into_iter()
    //                    .collect::<Vec<(u32, models::plasma::account::Account)>>()
    //            );
    //
    //            let create_account = |id| {
    //                let a = models::plasma::account::Account::default();
    //                vec![(
    //                    id,
    //                    AccountUpdate::Create {
    //                        nonce: a.nonce,
    //                        public_key_x: a.public_key_x,
    //                        public_key_y: a.public_key_y,
    //                    },
    //                )]
    //                .into_iter()
    //            };
    //            let transfer = |id_1, nonce_1, id_2, nonce_2| {
    //                let mut _a = models::plasma::account::Account::default();
    //                vec![
    //                    (
    //                        id_1,
    //                        AccountUpdate::UpdateBalance {
    //                            old_nonce: nonce_1,
    //                            new_nonce: nonce_1,
    //                            balance_update: (0, 1, 2),
    //                        },
    //                    ),
    //                    (
    //                        id_2,
    //                        AccountUpdate::UpdateBalance {
    //                            old_nonce: nonce_2,
    //                            new_nonce: nonce_2,
    //                            balance_update: (0, 2, 3),
    //                        },
    //                    ),
    //                ]
    //                .into_iter()
    //            };
    //
    //            let mut updates = Vec::new();
    //            updates.extend(create_account(2));
    //            updates.extend(create_account(4));
    //            updates.extend(transfer(2, 1, 4, 0));
    //            updates.extend(transfer(4, 0, 2, 1));
    //            updates.extend(transfer(2, 1, 4, 1));
    //            updates.extend(create_account(5));
    //
    //            conn.commit_state_update(1, &updates).expect("Commit state");
    //            apply_updates(&mut account_map, updates);
    //
    //            let (_, state) = conn.load_committed_state(None).unwrap();
    //            assert_eq!(
    //                state
    //                    .into_iter()
    //                    .collect::<Vec<(u32, models::plasma::account::Account)>>(),
    //                account_map
    //                    .clone()
    //                    .into_iter()
    //                    .collect::<Vec<(u32, models::plasma::account::Account)>>()
    //            )
    //        }

    fn acc_create_updates(
        id: u32,
        balance: u32,
        nonce: u32,
    ) -> impl Iterator<Item = (u32, AccountUpdate)> {
        let mut a = models::node::account::Account::default();
        a.nonce = nonce;
        let old_balance = a.get_balance(0).clone();
        a.set_balance(0, BigDecimal::from(balance));
        let new_balance = a.get_balance(0).clone();
        vec![
            (
                id,
                AccountUpdate::Create {
                    nonce: a.nonce,
                    address: a.address,
                },
            ),
            (
                id,
                AccountUpdate::UpdateBalance {
                    old_nonce: a.nonce,
                    new_nonce: a.nonce,
                    balance_update: (0, old_balance, new_balance),
                },
            ),
        ]
        .into_iter()
    }

    #[test]
    fn test_commit_rewind() {
        let _ = env_logger::try_init();

        let pool = ConnectionPool::new();
        let conn = pool.access_storage().unwrap();
        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test

        let (accounts_block_1, updates_block_1) = {
            let mut accounts = AccountMap::default();
            let updates = {
                let mut updates = Vec::new();
                updates.extend(acc_create_updates(1, 1, 2));
                updates.extend(acc_create_updates(2, 2, 4));
                updates.extend(acc_create_updates(3, 3, 8));
                updates
            };
            apply_updates(&mut accounts, updates.clone());
            (accounts, updates)
        };

        let (accounts_block_2, updates_block_2) = {
            let mut accounts = accounts_block_1.clone();
            let updates = {
                let mut updates = Vec::new();
                updates.extend(acc_create_updates(4, 1, 2));
                updates.extend(acc_create_updates(5, 2, 4));
                updates.extend(acc_create_updates(6, 3, 8));
                updates
            };
            apply_updates(&mut accounts, updates.clone());
            (accounts, updates)
        };
        let (accounts_block_3, updates_block_3) = {
            let mut accounts = accounts_block_2.clone();
            let updates = {
                let mut updates = Vec::new();
                updates.extend(acc_create_updates(7, 1, 2));
                updates.extend(acc_create_updates(8, 2, 4));
                updates.extend(acc_create_updates(9, 3, 8));
                updates
            };
            apply_updates(&mut accounts, updates.clone());
            (accounts, updates)
        };

        conn.commit_state_update(1, &updates_block_1).unwrap();
        conn.commit_state_update(2, &updates_block_2).unwrap();
        conn.commit_state_update(3, &updates_block_3).unwrap();

        let (block, state) = conn.load_committed_state(Some(1)).unwrap();
        assert_eq!(block, 1);
        assert_eq!(state, accounts_block_1);

        let (block, state) = conn.load_committed_state(Some(2)).unwrap();
        assert_eq!(block, 2);
        assert_eq!(state, accounts_block_2);

        let (block, state) = conn.load_committed_state(Some(3)).unwrap();
        assert_eq!(block, 3);
        assert_eq!(state, accounts_block_3);

        conn.apply_state_update(1).unwrap();
        conn.apply_state_update(2).unwrap();

        let (block, state) = conn.load_committed_state(Some(1)).unwrap();
        assert_eq!(block, 1);
        assert_eq!(state, accounts_block_1);

        let (block, state) = conn.load_committed_state(Some(2)).unwrap();
        assert_eq!(block, 2);
        assert_eq!(state, accounts_block_2);

        let (block, state) = conn.load_committed_state(Some(3)).unwrap();
        assert_eq!(block, 3);
        assert_eq!(state, accounts_block_3);

        let (block, state) = conn.load_committed_state(None).unwrap();
        assert_eq!(block, 3);
        assert_eq!(state, accounts_block_3);
    }
    //
    //    #[test]
    //    fn test_store_state() {
    //        let _ = env_logger::try_init();
    //
    //        let pool = ConnectionPool::new();
    //        let conn = pool.access_storage().unwrap();
    //        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test
    //
    //        let mut accounts = AccountMap::default();
    //
    //        // commit initial state update
    //        let updates = {
    //            let mut updates = Vec::new();
    //            updates.extend(acc_create_updates(1, 1, 2));
    //            updates.extend(acc_create_updates(2, 2, 4));
    //            updates.extend(acc_create_updates(3, 3, 8));
    //            updates
    //        };
    //        apply_updates(&mut accounts, updates.clone());
    //
    //        conn.commit_state_update(1, &updates).unwrap();
    //
    //        let (_, state) = conn.load_verified_state().unwrap();
    //        assert_eq!(state.len(), 0);
    //
    //        // committed state must be computed from updates
    //        let (last_block, state) = conn.load_committed_state(None).unwrap();
    //        assert_eq!(last_block, 1);
    //        assert_eq!(
    //            state
    //                .into_iter()
    //                .collect::<Vec<(u32, models::plasma::account::Account)>>(),
    //            accounts
    //                .clone()
    //                .into_iter()
    //                .collect::<Vec<(u32, models::plasma::account::Account)>>()
    //        );
    //
    //        // now apply commitment
    //        conn.apply_state_update(1).expect("update must work");
    //
    //        // verified state must be equal the commitment
    //        let (_, state) = conn.load_verified_state().unwrap();
    //        assert_eq!(
    //            state
    //                .into_iter()
    //                .collect::<Vec<(u32, models::plasma::account::Account)>>(),
    //            accounts
    //                .clone()
    //                .into_iter()
    //                .collect::<Vec<(u32, models::plasma::account::Account)>>()
    //        );
    //    }

    #[test]
    fn test_store_txs() {
        unimplemented!()
        //        let pool = ConnectionPool::new();
        //        let conn = pool.access_storage().unwrap();
        //        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test
        //        conn.prepare_nonce_scheduling("0x0", 0).unwrap();
        //
        //        let mut accounts = AccountMap::default();
        //        // commit initial state update
        //        let updates = {
        //            let mut updates = Vec::new();
        //            updates.extend(acc_create_updates(3, 1, 11));
        //            updates.extend(acc_create_updates(5, 2, 12));
        //            updates.extend(acc_create_updates(7, 3, 13));
        //            updates.extend(acc_create_updates(8, 4, 14));
        //            updates
        //        };
        //        apply_updates(&mut accounts, updates.clone());
        //
        //        conn.execute_operation(&Operation {
        //            id: None,
        //            action: Action::Commit,
        //            block: Block {
        //                block_number: 1,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Deposit {
        //                    batch_number: 0,
        //                    transactions: vec![],
        //                },
        //            },
        //            accounts_updated: updates,
        //            tx_meta: None,
        //        })
        //        .unwrap();
        //        assert_eq!(conn.last_verified_state_for_account(5).unwrap(), None);
        //        assert_eq!(
        //            conn.last_committed_state_for_account(5)
        //                .unwrap()
        //                .unwrap()
        //                .get_balance(ETH_TOKEN_ID),
        //            &BigDecimal::from(2)
        //        );
        //
        //        conn.execute_operation(&Operation {
        //            id: None,
        //            action: Action::Verify {
        //                proof: Box::new(EncodedProof::default()),
        //            },
        //            block: Block {
        //                block_number: 1,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Deposit {
        //                    batch_number: 0,
        //                    transactions: vec![],
        //                },
        //            },
        //            accounts_updated: AccountUpdates::default(),
        //            tx_meta: None,
        //        })
        //        .unwrap();
        //
        //        assert_eq!(
        //            conn.last_verified_state_for_account(7)
        //                .unwrap()
        //                .unwrap()
        //                .get_balance(ETH_TOKEN_ID),
        //            &BigDecimal::from(3)
        //        );
        //        assert_eq!(
        //            conn.last_committed_state_for_account(7)
        //                .unwrap()
        //                .unwrap()
        //                .get_balance(ETH_TOKEN_ID),
        //            &BigDecimal::from(3)
        //        );
        //
        //        let pending = conn.load_unsent_ops(0).unwrap();
        //        assert_eq!(pending.len(), 2);
        //        assert_eq!(pending[0].tx_meta.as_ref().unwrap().nonce, 0);
        //        assert_eq!(pending[1].tx_meta.as_ref().unwrap().nonce, 1);
        //
        //        let pending = conn.load_unsent_ops(1).unwrap();
        //        assert_eq!(pending.len(), 1);
        //        assert_eq!(pending[0].tx_meta.as_ref().unwrap().nonce, 1);
        //
        //        let pending = conn.load_unsent_ops(2).unwrap();
        //        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_store_proof_reqs() {
        unimplemented!()
        //        let pool = ConnectionPool::new();
        //        let conn = pool.access_storage().unwrap();
        //        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test
        //        conn.prepare_nonce_scheduling("0x0", 0).unwrap();
        //
        //        conn.execute_operation(&Operation {
        //            id: None,
        //            action: Action::Commit,
        //            block: Block {
        //                block_number: 1,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Deposit {
        //                    batch_number: 1,
        //                    transactions: Vec::new(),
        //                },
        //            },
        //            accounts_updated: AccountUpdates::default(),
        //            tx_meta: None,
        //        })
        //        .unwrap();
        //
        //        let pending = conn.load_unverified_commitments().unwrap();
        //        assert_eq!(pending.len(), 1);
        //
        //        conn.execute_operation(&Operation {
        //            id: None,
        //            action: Action::Verify {
        //                proof: Box::new(EncodedProof::default()),
        //            },
        //            block: Block {
        //                block_number: 1,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Deposit {
        //                    batch_number: 1,
        //                    transactions: Vec::new(),
        //                },
        //            },
        //            accounts_updated: AccountUpdates::default(),
        //            tx_meta: None,
        //        })
        //        .unwrap();
        //
        //        let pending = conn.load_unverified_commitments().unwrap();
        //        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_store_helpers() {
        unimplemented!()
        //        let pool = ConnectionPool::new();
        //        let conn = pool.access_storage().unwrap();
        //        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test
        //
        //        assert_eq!(-1, conn.load_last_committed_deposit_batch().unwrap());
        //        assert_eq!(-1, conn.load_last_committed_exit_batch().unwrap());
        //        assert_eq!(0, conn.get_last_committed_block().unwrap());
        //        assert_eq!(0, conn.get_last_verified_block().unwrap());
        //        assert_eq!(conn.last_committed_state_for_account(9999).unwrap(), None);
        //        assert_eq!(conn.last_verified_state_for_account(9999).unwrap(), None);
        //
        //        conn.execute_operation(&Operation {
        //            id: None,
        //            action: Action::Commit,
        //            block: Block {
        //                block_number: 1,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Deposit {
        //                    batch_number: 3,
        //                    transactions: Vec::new(),
        //                },
        //            },
        //            accounts_updated: AccountUpdates::default(),
        //            tx_meta: None,
        //        })
        //        .unwrap();
        //        assert_eq!(3, conn.load_last_committed_deposit_batch().unwrap());
        //
        //        conn.execute_operation(&Operation {
        //            id: None,
        //            action: Action::Commit,
        //            block: Block {
        //                block_number: 1,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Exit {
        //                    batch_number: 2,
        //                    transactions: Vec::new(),
        //                },
        //            },
        //            accounts_updated: AccountUpdates::default(),
        //            tx_meta: None,
        //        })
        //        .unwrap();
        //        assert_eq!(2, conn.load_last_committed_exit_batch().unwrap());
    }

    #[test]
    fn test_store_txs_2() {
        unimplemented!()
        //        let pool = ConnectionPool::new();
        //        let conn = pool.access_storage().unwrap();
        //        conn.conn().begin_test_transaction().unwrap();
        //
        //        let deposit_tx: NewDepositTx = NewDepositTx {
        //            account: 1,
        //            amount: BigDecimal::from(10000),
        //            pub_x: Fr::zero(),
        //            pub_y: Fr::zero(),
        //        };
        //
        //        let transfer_tx: TransferTx = TransferTx {
        //            from: 1,
        //            to: 2,
        //            amount: BigDecimal::from(5000),
        //            fee: BigDecimal::from(0),
        //            nonce: 1,
        //            good_until_block: 100_000,
        //            signature: TxSignature::default(),
        //        };
        //
        //        let exit_tx: ExitTx = ExitTx {
        //            account: 1,
        //            amount: BigDecimal::from(5000),
        //        };
        //
        //        conn.execute_operation(&Operation {
        //            id: None,
        //            action: Action::Commit,
        //            block: Block {
        //                block_number: 1,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Deposit {
        //                    batch_number: 1,
        //                    transactions: vec![deposit_tx.clone(), deposit_tx.clone()],
        //                },
        //            },
        //            accounts_updated: AccountUpdates::default(),
        //            tx_meta: None,
        //        })
        //        .unwrap();
        //
        //        conn.execute_operation(&Operation {
        //            id: None,
        //            action: Action::Commit,
        //            block: Block {
        //                block_number: 2,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Transfer {
        //                    total_fees: BigDecimal::from(0),
        //                    transactions: vec![transfer_tx.clone(), transfer_tx.clone()],
        //                },
        //            },
        //            accounts_updated: AccountUpdates::default(),
        //            tx_meta: None,
        //        })
        //        .unwrap();
        //
        //        conn.execute_operation(&Operation {
        //            id: None,
        //            action: Action::Commit,
        //            block: Block {
        //                block_number: 3,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Exit {
        //                    batch_number: 2,
        //                    transactions: vec![exit_tx.clone(), exit_tx.clone()],
        //                },
        //            },
        //            accounts_updated: AccountUpdates::default(),
        //            tx_meta: None,
        //        })
        //        .unwrap();
        //
        //        let txs = conn.load_last_saved_transactions(10);
        //        assert_eq!(txs.len(), 6);
    }

    fn dummy_op(_action: Action, _block_number: BlockNumber) -> Operation {
        unimplemented!()
        //        Operation {
        //            id: None,
        //            action,
        //            block: Block {
        //                block_number,
        //                new_root_hash: Fr::default(),
        //                block_data: BlockData::Deposit {
        //                    batch_number: 1,
        //                    transactions: Vec::new(),
        //                },
        //            },
        //            accounts_updated: AccountUpdates::default(),
        //            tx_meta: None,
        //        }
    }

    #[test]
    fn test_nonce_fast_forward() {
        let pool = ConnectionPool::new();
        let conn = pool.access_storage().unwrap();
        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test

        conn.prepare_nonce_scheduling("0x123", 0).expect("failed");

        let mut i = 0;
        let stored_op = loop {
            let stored_op = conn
                .execute_operation(&dummy_op(Action::Commit, 1))
                .unwrap();
            i += 1;
            if i >= 6 {
                break stored_op;
            }
        };

        // after 6 iterations starting with nonce = 0, last inserted nonce should be 5
        assert_eq!(stored_op.tx_meta.as_ref().expect("no meta?").nonce, 5);

        // addr for
        assert_eq!(stored_op.tx_meta.as_ref().expect("no meta?").addr, "0x123");

        // if the nonce from network is lower than expected, this should not affect the scheduler
        conn.prepare_nonce_scheduling("0x123", 3).expect("failed");
        let stored_op = conn
            .execute_operation(&dummy_op(Action::Commit, 1))
            .unwrap();
        assert_eq!(stored_op.tx_meta.as_ref().expect("no meta?").nonce, 6);

        // if the nonce from network is same as expected, this should not affect the scheduler
        conn.prepare_nonce_scheduling("0x123", 7).expect("failed");
        let stored_op = conn
            .execute_operation(&dummy_op(Action::Commit, 1))
            .unwrap();
        assert_eq!(stored_op.tx_meta.as_ref().expect("no meta?").nonce, 7);

        // if the nonce from network is higher than expected, scheduler should be fast-forwarded
        conn.prepare_nonce_scheduling("0x123", 9).expect("failed");
        let stored_op = conn
            .execute_operation(&dummy_op(Action::Commit, 1))
            .unwrap();
        assert_eq!(stored_op.tx_meta.as_ref().expect("no meta?").nonce, 9);

        // if the address is new, nonces start from the given value
        conn.prepare_nonce_scheduling("0x456", 2).expect("failed");
        let stored_op = conn
            .execute_operation(&dummy_op(Action::Commit, 1))
            .unwrap();
        assert_eq!(stored_op.tx_meta.as_ref().expect("no meta?").nonce, 2);

        // addr should now switch
        assert_eq!(stored_op.tx_meta.as_ref().expect("no meta?").addr, "0x456");

        // if we switch back to existing sender, nonce sequence should continue from where it started with that addr
        conn.prepare_nonce_scheduling("0x123", 0).expect("failed");
        let stored_op = conn
            .execute_operation(&dummy_op(Action::Commit, 1))
            .unwrap();
        assert_eq!(stored_op.tx_meta.as_ref().expect("no meta?").nonce, 10);

        // add should now switch back
        assert_eq!(stored_op.tx_meta.as_ref().expect("no meta?").addr, "0x123");
    }
}
