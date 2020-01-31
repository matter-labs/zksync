//! # Representation of the sidechain state in the DB:
//!
//! Saving state is done in two steps
//! 1) When we commit block we save all state updates (tables: `account_creates`, `account_balance_updates`)
//! 2) When we verify block we apply this updates to stored state snapshot (tables: `accounts`, `balances`)
//!
//! This way we have the following advantages:
//! 1) Easy access to state for any block (useful for provers which work on different blocks)
//! 2) We can rewind any `commited` state (which is not final)

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;

use bigdecimal::BigDecimal;
use chrono::prelude::*;
use diesel::dsl::*;
use failure::bail;
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use models::node::{
    apply_updates, reverse_updates, tx::FranklinTx, Account, AccountId, AccountMap, AccountUpdate,
    AccountUpdates, BlockNumber, Fr, FranklinOp, PriorityOp, TokenId,
};
use models::{Action, ActionType, EncodedProof, Operation, TokenAddedEvent};
use serde_derive::{Deserialize, Serialize};
use std::cmp;
use std::time;
use web3::types::H256;

mod schema;

use crate::schema::*;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError, PooledConnection};

use serde_json::value::Value;
use std::env;

use diesel::sql_types::{BigInt, Bool, Int4, Jsonb, Nullable, Text, Timestamp};

use itertools::Itertools;
use models::node::PubKeyHash;
use std::cmp::Ordering;
use std::collections::HashMap;
use web3::types::Address;

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
    pub id: i64,
    pub last_block: i64,
    pub nonce: i64,
    pub address: Vec<u8>,
    pub pubkey_hash: Vec<u8>,
}

#[derive(Identifiable, Insertable, QueryableByName, Queryable, Associations)]
#[belongs_to(StorageAccount, foreign_key = "account_id")]
#[primary_key(account_id, coin_id)]
#[table_name = "balances"]
struct StorageBalance {
    pub account_id: i64,
    pub coin_id: i32,
    pub balance: BigDecimal,
}

#[derive(
    Debug, Clone, Insertable, QueryableByName, Queryable, Serialize, Deserialize, AsChangeset,
)]
#[table_name = "tokens"]
pub struct Token {
    pub id: i32,
    pub address: String,
    pub symbol: Option<String>,
}

#[derive(Debug, Insertable)]
#[table_name = "account_balance_updates"]
struct StorageAccountUpdateInsert {
    pub update_order_id: i32,
    pub account_id: i64,
    pub block_number: i64,
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
    pub account_id: i64,
    pub block_number: i64,
    pub coin_id: i32,
    pub old_balance: BigDecimal,
    pub new_balance: BigDecimal,
    pub old_nonce: i64,
    pub new_nonce: i64,
    pub update_order_id: i32,
}

#[derive(Debug, Insertable)]
#[table_name = "account_pubkey_updates"]
struct StorageAccountPubkeyUpdateInsert {
    pub update_order_id: i32,
    pub account_id: i64,
    pub block_number: i64,
    pub old_pubkey_hash: Vec<u8>,
    pub new_pubkey_hash: Vec<u8>,
    pub old_nonce: i64,
    pub new_nonce: i64,
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "account_pubkey_updates"]
struct StorageAccountPubkeyUpdate {
    pubkey_update_id: i32,
    pub update_order_id: i32,
    pub account_id: i64,
    pub block_number: i64,
    pub old_pubkey_hash: Vec<u8>,
    pub new_pubkey_hash: Vec<u8>,
    pub old_nonce: i64,
    pub new_nonce: i64,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "account_creates"]
struct StorageAccountCreation {
    account_id: i64,
    is_create: bool,
    block_number: i64,
    address: Vec<u8>,
    nonce: i64,
    update_order_id: i32,
}

#[derive(Debug, Insertable)]
#[table_name = "executed_transactions"]
struct NewExecutedTransaction {
    block_number: i64,
    tx_hash: Vec<u8>,
    operation: Option<Value>,
    success: bool,
    fail_reason: Option<String>,
    block_index: Option<i32>,
}

impl NewExecutedTransaction {
    fn prepare_stored_tx(exec_tx: &ExecutedTx, block: BlockNumber) -> Self {
        Self {
            block_number: i64::from(block),
            tx_hash: exec_tx.tx.hash().as_ref().to_vec(),
            operation: exec_tx.op.clone().map(|o| serde_json::to_value(o).unwrap()),
            success: exec_tx.success,
            fail_reason: exec_tx.fail_reason.clone(),
            block_index: exec_tx.block_index.map(|idx| idx as i32),
        }
    }
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
    block_index: Option<i32>,
}

impl StoredExecutedTransaction {
    fn into_executed_tx(self, stored_tx: Option<ReadTx>) -> Result<ExecutedTx, failure::Error> {
        if let Some(op) = self.operation {
            let franklin_op: FranklinOp =
                serde_json::from_value(op).expect("Unparsable FranklinOp in db");
            Ok(ExecutedTx {
                tx: franklin_op
                    .try_get_tx()
                    .expect("FranklinOp should not have tx"),
                success: true,
                op: Some(franklin_op),
                fail_reason: None,
                block_index: Some(self.block_index.expect("Block idx should be set") as u32),
            })
        } else if let Some(stored_tx) = stored_tx {
            let tx: FranklinTx = serde_json::from_value(stored_tx.tx).expect("Unparsable tx in db");
            Ok(ExecutedTx {
                tx,
                success: false,
                op: None,
                fail_reason: self.fail_reason,
                block_index: None,
            })
        } else {
            bail!("Unsuccessful tx was lost from db.");
        }
    }
}

#[derive(Debug, Insertable)]
#[table_name = "executed_priority_operations"]
struct NewExecutedPriorityOperation {
    block_number: i64,
    block_index: i32,
    operation: Value,
    priority_op_serialid: i64,
    deadline_block: i64,
    eth_fee: BigDecimal,
    eth_hash: Vec<u8>,
}

impl NewExecutedPriorityOperation {
    fn prepare_stored_priority_op(exec_prior_op: &ExecutedPriorityOp, block: BlockNumber) -> Self {
        Self {
            block_number: i64::from(block),
            block_index: exec_prior_op.block_index as i32,
            operation: serde_json::to_value(&exec_prior_op.op).unwrap(),
            priority_op_serialid: exec_prior_op.priority_op.serial_id as i64,
            deadline_block: exec_prior_op.priority_op.deadline_block as i64,
            eth_fee: exec_prior_op.priority_op.eth_fee.clone(),
            eth_hash: exec_prior_op.priority_op.eth_hash.clone(),
        }
    }
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "executed_priority_operations"]
pub struct StoredExecutedPriorityOperation {
    pub id: i32,
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_fee: BigDecimal,
    pub eth_hash: Vec<u8>,
}

impl Into<ExecutedPriorityOp> for StoredExecutedPriorityOperation {
    fn into(self) -> ExecutedPriorityOp {
        let franklin_op: FranklinOp =
            serde_json::from_value(self.operation).expect("Unparsable priority op in db");
        ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id: self.priority_op_serialid as u64,
                data: franklin_op
                    .try_get_priority_op()
                    .expect("FranklinOp should have priority op"),
                deadline_block: self.deadline_block as u64,
                eth_fee: self.eth_fee,
                eth_hash: self.eth_hash,
            },
            op: franklin_op,
            block_index: self.block_index as u32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxReceiptResponse {
    pub tx_hash: String,
    pub block_number: i64,
    pub success: bool,
    pub verified: bool,
    pub fail_reason: Option<String>,
    pub prover_run: Option<ProverRun>,
}

// TODO: jazzandrock add more info(?)
#[derive(Debug, Serialize, Deserialize)]
pub struct PriorityOpReceiptResponse {
    committed: bool,
    verified: bool,
    prover_run: Option<ProverRun>,
}

#[derive(Debug, Serialize, Deserialize, Queryable, QueryableByName)]
pub struct TxByHashResponse {
    #[sql_type = "Text"]
    pub tx_type: String, // all

    #[sql_type = "Text"]
    pub from: String, // transfer(from) | deposit(our contract) | withdraw(sender)

    #[sql_type = "Text"]
    pub to: String, // transfer(to) | deposit(sender) | withdraw(our contract)

    #[sql_type = "Int4"]
    pub token: i32, // all

    #[sql_type = "Text"]
    pub amount: String, // all

    #[sql_type = "Nullable<Text>"]
    pub fee: Option<String>, // means Sync fee, not eth. transfer(sync fee), deposit(none), withdraw(Sync fee)

    #[sql_type = "BigInt"]
    pub block_number: i64, // all
}

#[derive(Debug)]
enum StorageAccountDiff {
    BalanceUpdate(StorageAccountUpdate),
    Create(StorageAccountCreation),
    Delete(StorageAccountCreation),
    ChangePubKey(StorageAccountPubkeyUpdate),
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

impl From<StorageAccountPubkeyUpdate> for StorageAccountDiff {
    fn from(update: StorageAccountPubkeyUpdate) -> Self {
        StorageAccountDiff::ChangePubKey(update)
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
                    address: Address::from_slice(&upd.address.as_slice()),
                },
            ),
            StorageAccountDiff::Delete(upd) => (
                upd.account_id as u32,
                AccountUpdate::Delete {
                    nonce: upd.nonce as u32,
                    address: Address::from_slice(&upd.address.as_slice()),
                },
            ),
            StorageAccountDiff::ChangePubKey(upd) => (
                upd.account_id as u32,
                AccountUpdate::ChangePubKeyHash {
                    old_nonce: upd.old_nonce as u32,
                    new_nonce: upd.new_nonce as u32,
                    old_pub_key_hash: PubKeyHash::from_bytes(&upd.old_pubkey_hash)
                        .expect("PubkeyHash update from db deserialzie"),
                    new_pub_key_hash: PubKeyHash::from_bytes(&upd.new_pubkey_hash)
                        .expect("PubkeyHash update from db deserialzie"),
                },
            ),
        }
    }
}

impl StorageAccountDiff {
    fn update_order_id(&self) -> i32 {
        *match self {
            StorageAccountDiff::BalanceUpdate(StorageAccountUpdate {
                update_order_id, ..
            }) => update_order_id,
            StorageAccountDiff::Create(StorageAccountCreation {
                update_order_id, ..
            }) => update_order_id,
            StorageAccountDiff::Delete(StorageAccountCreation {
                update_order_id, ..
            }) => update_order_id,
            StorageAccountDiff::ChangePubKey(StorageAccountPubkeyUpdate {
                update_order_id,
                ..
            }) => update_order_id,
        }
    }

    /// Compares updates by `block number` then by `update_order_id(number within block)`.
    fn cmp_order(&self, other: &Self) -> Ordering {
        self.block_number()
            .cmp(&other.block_number())
            .then(self.update_order_id().cmp(&other.update_order_id()))
    }

    fn block_number(&self) -> i64 {
        *match self {
            StorageAccountDiff::BalanceUpdate(StorageAccountUpdate { block_number, .. }) => {
                block_number
            }
            StorageAccountDiff::Create(StorageAccountCreation { block_number, .. }) => block_number,
            StorageAccountDiff::Delete(StorageAccountCreation { block_number, .. }) => block_number,
            StorageAccountDiff::ChangePubKey(StorageAccountPubkeyUpdate {
                block_number, ..
            }) => block_number,
        }
    }
}

#[derive(Debug, Insertable)]
#[table_name = "operations"]
struct NewOperation {
    pub block_number: i64,
    pub action_type: String,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "operations"]
pub struct StoredOperation {
    pub id: i64,
    pub block_number: i64,
    pub action_type: String,
    pub created_at: NaiveDateTime,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "eth_operations"]
pub struct StorageETHOperation {
    pub id: i64,
    pub op_id: i64,
    pub nonce: i64,
    pub deadline_block: i64,
    pub gas_price: BigDecimal,
    pub tx_hash: Vec<u8>,
    pub confirmed: bool,
    pub raw_tx: Vec<u8>,
}

#[derive(Debug, Insertable)]
#[table_name = "eth_operations"]
struct NewETHOperation {
    op_id: i64,
    nonce: i64,
    deadline_block: i64,
    gas_price: BigDecimal,
    tx_hash: Vec<u8>,
    raw_tx: Vec<u8>,
}

#[derive(Debug, Insertable, Queryable)]
#[table_name = "blocks"]
struct StorageBlock {
    number: i64,
    root_hash: String,
    fee_account_id: i64,
    unprocessed_prior_op_before: i64,
    unprocessed_prior_op_after: i64,
}

impl StoredOperation {
    pub fn into_op(self, conn: &StorageProcessor) -> QueryResult<Operation> {
        let block_number = self.block_number as BlockNumber;
        let id = Some(self.id);

        let action = if self.action_type == ActionType::COMMIT.to_string() {
            Action::Commit
        } else if self.action_type == ActionType::VERIFY.to_string() {
            // verify
            let proof = Box::new(conn.load_proof(block_number)?);
            Action::Verify { proof }
        } else {
            unreachable!("Incorrect action type in db");
        };

        let block = conn
            .get_block(block_number)?
            .expect("Block for action does not exist");
        let accounts_updated = conn.load_state_diff_for_block(block_number)?;
        Ok(Operation {
            id,
            action,
            block,
            accounts_updated,
        })
    }
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "proofs"]
pub struct NewProof {
    pub block_number: i64,
    pub proof: serde_json::Value,
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "proofs"]
pub struct StoredProof {
    pub block_number: i64,
    pub proof: serde_json::Value,
    pub created_at: NaiveDateTime,
}

// Every time before a prover worker starts generating the proof, a prover run is recorded for monitoring purposes
#[derive(Debug, Clone, Insertable, Queryable, QueryableByName, Serialize, Deserialize)]
#[table_name = "prover_runs"]
pub struct ProverRun {
    pub id: i32,
    pub block_number: i64,
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
    #[sql_type = "BigInt"]
    pub integer_value: i64,
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "server_config"]
pub struct ServerConfig {
    pub id: bool,
    pub contract_addr: Option<String>,
    pub gov_contract_addr: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, QueryableByName)]
pub struct BlockDetails {
    #[sql_type = "BigInt"]
    pub block_number: i64,

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

#[derive(Insertable)]
#[table_name = "storage_state_update"]
pub struct NewStorageState {
    pub storage_state: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Queryable, QueryableByName)]
#[table_name = "storage_state_update"]
pub struct StoredStorageState {
    pub id: i32,
    pub storage_state: String,
}

#[derive(Insertable)]
#[table_name = "data_restore_last_watched_eth_block"]
pub struct NewLastWatchedEthBlockNumber {
    pub block_number: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Queryable, QueryableByName)]
#[table_name = "data_restore_last_watched_eth_block"]
pub struct StoredLastWatchedEthBlockNumber {
    pub id: i32,
    pub block_number: String,
}

#[derive(Insertable)]
#[table_name = "events_state"]
pub struct NewBlockEvent {
    pub block_type: String, // 'Committed', 'Verified'
    pub transaction_hash: Vec<u8>,
    pub block_num: i64,
}

#[derive(Insertable, Serialize, Deserialize, Debug, Clone, Queryable, QueryableByName)]
#[table_name = "events_state"]
pub struct StoredBlockEvent {
    pub id: i32,
    pub block_type: String, // 'Committed', 'Verified'
    pub transaction_hash: Vec<u8>,
    pub block_num: i64,
}

#[derive(Debug, Insertable)]
#[table_name = "rollup_ops"]
pub struct NewFranklinOp {
    pub block_num: i64,
    pub operation: Value,
    pub fee_account: i64,
}

impl NewFranklinOp {
    fn prepare_stored_op(
        franklin_op: &FranklinOp,
        block: BlockNumber,
        fee_account: AccountId,
    ) -> Self {
        Self {
            block_num: i64::from(block),
            operation: serde_json::to_value(franklin_op.clone()).unwrap(),
            fee_account: i64::from(fee_account),
        }
    }
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "rollup_ops"]
pub struct StoredFranklinOp {
    pub id: i32,
    pub block_num: i64,
    pub operation: Value,
    pub fee_account: i64,
}

impl StoredFranklinOp {
    fn into_franklin_op(self) -> FranklinOp {
        serde_json::from_value(self.operation).expect("Unparsable FranklinOp in db")
    }
}

#[derive(Debug, Clone, Queryable)]
pub struct StoredRollupOpsBlock {
    pub block_num: BlockNumber,
    pub ops: Vec<FranklinOp>,
    pub fee_account: AccountId,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountTransaction {
    tx: Value,
    tx_hash: String,
    success: bool,
    fail_reason: Option<String>,
    committed: bool,
    verified: bool,
}

enum ConnectionHolder {
    Pooled(PooledConnection<ConnectionManager<PgConnection>>),
    Direct(PgConnection),
}

pub struct StorageProcessor {
    conn: ConnectionHolder,
}

#[derive(Debug, Serialize, Deserialize, QueryableByName)]
pub struct TransactionsHistoryItem {
    #[sql_type = "Nullable<Text>"]
    pub hash: Option<String>,

    #[sql_type = "Nullable<BigInt>"]
    pub pq_id: Option<i64>,

    #[sql_type = "Jsonb"]
    pub tx: Value,

    #[sql_type = "Nullable<Bool>"]
    pub success: Option<bool>,

    #[sql_type = "Nullable<Text>"]
    pub fail_reason: Option<String>,

    #[sql_type = "Bool"]
    pub commited: bool,

    #[sql_type = "Bool"]
    pub verified: bool,
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
    account.address = Address::from_slice(&stored_account.address);
    account.pub_key_hash = PubKeyHash::from_bytes(&stored_account.pubkey_hash)
        .expect("db stored pubkey hash deserialize");
    (stored_account.id as u32, account)
}

pub struct StoredAccountState {
    pub committed: Option<(AccountId, Account)>,
    pub verified: Option<(AccountId, Account)>,
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
                    self.save_block(&op.block)?;
                }
                Action::Verify { .. } => self.apply_state_update(op.block.block_number)?,
            };

            let stored: StoredOperation = diesel::insert_into(operations::table)
                .values(&NewOperation {
                    block_number: i64::from(op.block.block_number),
                    action_type: op.action.to_string(),
                })
                .get_result(self.conn())?;
            stored.into_op(self)
        })
    }

    fn save_block(&self, block: &Block) -> QueryResult<()> {
        self.conn().transaction(|| {
            self.save_block_transactions(block)?;

            let new_block = StorageBlock {
                number: i64::from(block.block_number),
                root_hash: format!("sync-bl:{}", &block.new_root_hash.to_hex()),
                fee_account_id: i64::from(block.fee_account),
                unprocessed_prior_op_before: block.processed_priority_ops.0 as i64,
                unprocessed_prior_op_after: block.processed_priority_ops.1 as i64,
            };

            diesel::insert_into(blocks::table)
                .values(&new_block)
                .execute(self.conn())?;

            Ok(())
        })
    }

    pub fn save_block_transactions(&self, block: &Block) -> QueryResult<()> {
        for block_tx in block.block_transactions.iter() {
            match block_tx {
                ExecutedOperations::Tx(tx) => {
                    let stored_tx =
                        NewExecutedTransaction::prepare_stored_tx(tx, block.block_number);
                    diesel::insert_into(mempool::table)
                        .values(&InsertTx {
                            hash: tx.tx.hash().as_ref().to_vec(),
                            primary_account_address: tx.tx.account().as_bytes().to_vec(),
                            nonce: tx.tx.nonce() as i64,
                            tx: serde_json::to_value(&tx.tx).unwrap_or_default(),
                        })
                        .on_conflict_do_nothing()
                        .execute(self.conn())?;
                    diesel::insert_into(executed_transactions::table)
                        .values(&stored_tx)
                        .execute(self.conn())?;
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    let stored_priority_op =
                        NewExecutedPriorityOperation::prepare_stored_priority_op(
                            prior_op,
                            block.block_number,
                        );
                    diesel::insert_into(executed_priority_operations::table)
                        .values(&stored_priority_op)
                        .execute(self.conn())?;
                }
            }
        }
        Ok(())
    }

    pub fn get_priority_op_receipt(&self, op_id: i64) -> QueryResult<PriorityOpReceiptResponse> {
        // TODO: jazzandrock maybe use one db query(?).
        let stored_executed_prior_op = executed_priority_operations::table
            .filter(executed_priority_operations::priority_op_serialid.eq(op_id))
            .first::<StoredExecutedPriorityOperation>(self.conn())
            .optional()?;

        match stored_executed_prior_op {
            Some(stored_executed_prior_op) => {
                let prover_run: Option<ProverRun> = prover_runs::table
                    .filter(prover_runs::block_number.eq(stored_executed_prior_op.block_number))
                    .first::<ProverRun>(self.conn())
                    .optional()?;

                let commit = operations::table
                    .filter(operations::block_number.eq(stored_executed_prior_op.block_number))
                    .filter(operations::action_type.eq(ActionType::COMMIT.to_string()))
                    .first::<StoredOperation>(self.conn())
                    .optional()?;

                let confirm = operations::table
                    .filter(operations::block_number.eq(stored_executed_prior_op.block_number))
                    .filter(operations::action_type.eq(ActionType::VERIFY.to_string()))
                    .first::<StoredOperation>(self.conn())
                    .optional()?;

                Ok(PriorityOpReceiptResponse {
                    committed: commit.is_some(),
                    verified: confirm.is_some(),
                    prover_run,
                })
            }
            None => Ok(PriorityOpReceiptResponse {
                committed: false,
                verified: false,
                prover_run: None,
            }),
        }
    }

    pub fn get_tx_by_hash(&self, hash: &[u8]) -> QueryResult<Option<TxByHashResponse>> {
        // TODO: Maybe move the transformations to api_server?

        // first check executed_transactions
        let tx: Option<StoredExecutedTransaction> = executed_transactions::table
            .filter(executed_transactions::tx_hash.eq(hash))
            .first(self.conn())
            .optional()?;

        if let Some(tx) = tx {
            let block_number = tx.block_number;
            let operation = tx.operation.unwrap_or_else(|| {
                debug!("operation empty in executed_transactions");
                Value::default()
            });

            let tx_type = operation["type"].as_str().unwrap_or("unknown type");
            let tx_token = operation["tx"]["token"].as_i64().unwrap_or(-1);
            let tx_amount = operation["tx"]["amount"]
                .as_str()
                .unwrap_or("unknown amount");

            let (tx_from, tx_to, tx_fee) = match tx_type {
                "Withdraw" => (
                    operation["tx"]["account"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["tx"]["ethAddress"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["tx"]["fee"].as_str().map(|v| v.to_string()),
                ),
                "Transfer" | "TransferToNew" => (
                    operation["tx"]["from"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["tx"]["to"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["tx"]["fee"].as_str().map(|v| v.to_string()),
                ),
                &_ => (
                    "unknown from".to_string(),
                    "unknown to".to_string(),
                    Some("unknown fee".to_string()),
                ),
            };

            let tx_type_user = if tx_type == "TransferToNew" {
                "Transfer"
            } else {
                tx_type
            };

            return Ok(Some(TxByHashResponse {
                tx_type: tx_type_user.to_string(),
                from: tx_from,
                to: tx_to,
                token: tx_token as i32,
                amount: tx_amount.to_string(),
                fee: tx_fee,
                block_number,
            }));
        };

        // then check executed_priority_operations
        let tx: Option<StoredExecutedPriorityOperation> = executed_priority_operations::table
            .filter(executed_priority_operations::eth_hash.eq(hash))
            .first(self.conn())
            .optional()?;

        if let Some(tx) = tx {
            let operation = tx.operation;
            let block_number = tx.block_number;

            let tx_type = operation["type"].as_str().unwrap_or("unknown type");
            let tx_token = operation["priority_op"]["token"]
                .as_i64()
                .expect("must be here");
            let tx_amount = operation["priority_op"]["amount"]
                .as_str()
                .unwrap_or("unknown amount");

            let (tx_from, tx_to, tx_fee) = match tx_type {
                "Deposit" => (
                    operation["priority_op"]["sender"]
                        .as_str()
                        .unwrap_or("unknown from")
                        .to_string(),
                    operation["priority_op"]["account"]
                        .as_str()
                        .unwrap_or("unknown to")
                        .to_string(),
                    operation["priority_op"]["fee"]
                        .as_str()
                        .map(|v| v.to_string()),
                ),
                &_ => (
                    "unknown from".to_string(),
                    "unknown to".to_string(),
                    Some("unknown fee".to_string()),
                ),
            };

            return Ok(Some(TxByHashResponse {
                tx_type: tx_type.to_string(),
                from: tx_from,
                to: tx_to,
                token: tx_token as i32,
                amount: tx_amount.to_string(),
                fee: tx_fee,
                block_number,
            }));
        };

        Ok(None)
    }

    pub fn get_account_transactions_history(
        &self,
        address: &PubKeyHash,
        offset: i64,
        limit: i64,
    ) -> QueryResult<Vec<TransactionsHistoryItem>> {
        // TODO: txs are not ordered
        let query = format!(
            "
            select
                hash,
                pq_id,
                tx,
                success,
                fail_reason,
                coalesce(commited, false) as commited,
                coalesce(verified, false) as verified
            from (
                select
                    *
                from (
                    select
                        tx,
                        'sync-tx:' || encode(hash, 'hex') as hash,
                        null as pq_id,
                        success,
                        fail_reason,
                        block_number
                    from
                        mempool
                    left join
                        executed_transactions
                    on
                        tx_hash = hash
                    where
                        'sync:' || encode(primary_account_address, 'hex') = '{address}'
                        or
                        tx->>'to' = '{address}'
                    union all
                    select
                        operation as tx,
                        '0x' || encode(eth_hash, 'hex') as hash,
                        priority_op_serialid as pq_id,
                        null as success,
                        null as fail_reason,
                        block_number
                    from 
                        executed_priority_operations
                    where 
                        operation->'priority_op'->>'account' = '{address}') t
                order by
                    block_number desc
                offset 
                    {offset}
                limit 
                    {limit}
            ) t
            left join
                crosstab($$
                    select 
                        block_number as rowid, 
                        action_type as category, 
                        true as values 
                    from 
                        operations
                    order by
                        block_number
                    $$) t3 (
                        block_number bigint, 
                        commited boolean, 
                        verified boolean)
            using 
                (block_number)
            ",
            address = address.to_hex(),
            offset = offset,
            limit = limit
        );

        diesel::sql_query(query).load::<TransactionsHistoryItem>(self.conn())
    }

    pub fn get_account_transactions(
        &self,
        address: &PubKeyHash,
    ) -> QueryResult<Vec<AccountTransaction>> {
        let all_txs: Vec<_> = mempool::table
            .filter(mempool::primary_account_address.eq(address.data.to_vec()))
            .left_join(
                executed_transactions::table.on(executed_transactions::tx_hash.eq(mempool::hash)),
            )
            .left_join(
                operations::table
                    .on(operations::block_number.eq(executed_transactions::block_number)),
            )
            .load::<(
                ReadTx,
                Option<StoredExecutedTransaction>,
                Option<StoredOperation>,
            )>(self.conn())?;

        let res = all_txs
            .into_iter()
            .group_by(|(mempool_tx, _, _)| mempool_tx.hash.clone())
            .into_iter()
            .map(|(_op_id, mut group_iter)| {
                // TODO: replace the query with pivot
                let (mempool_tx, executed_tx, operation) = group_iter.next().unwrap();
                let mut res = AccountTransaction {
                    tx: mempool_tx.tx,
                    tx_hash: hex::encode(mempool_tx.hash.as_slice()),
                    success: false,
                    fail_reason: None,
                    committed: false,
                    verified: false,
                };
                if let Some(executed_tx) = executed_tx {
                    res.success = executed_tx.success;
                    res.fail_reason = executed_tx.fail_reason;
                }
                if let Some(operation) = operation {
                    if operation.action_type == ActionType::COMMIT.to_string() {
                        res.committed = operation.confirmed;
                    } else {
                        res.verified = operation.confirmed;
                    }
                }
                if let Some((_mempool_tx, _executed_tx, operation)) = group_iter.next() {
                    if let Some(operation) = operation {
                        if operation.action_type == ActionType::COMMIT.to_string() {
                            res.committed = operation.confirmed;
                        } else {
                            res.verified = operation.confirmed;
                        }
                    };
                }
                res
            })
            .collect::<Vec<AccountTransaction>>();

        Ok(res)
    }

    pub fn get_block(&self, block: BlockNumber) -> QueryResult<Option<Block>> {
        let stored_block = if let Some(block) = blocks::table
            .find(i64::from(block))
            .first::<StorageBlock>(self.conn())
            .optional()?
        {
            block
        } else {
            return Ok(None);
        };

        let block_transactions = self.get_block_executed_ops(block)?;

        Ok(Some(Block {
            block_number: block,
            new_root_hash: Fr::from_hex(&format!("0x{}", &stored_block.root_hash[8..]))
                .expect("Unparsable root hash"),
            fee_account: stored_block.fee_account_id as AccountId,
            block_transactions,
            processed_priority_ops: (
                stored_block.unprocessed_prior_op_before as u64,
                stored_block.unprocessed_prior_op_after as u64,
            ),
        }))
    }

    pub fn get_block_executed_ops(
        &self,
        block: BlockNumber,
    ) -> QueryResult<Vec<ExecutedOperations>> {
        self.conn().transaction(|| {
            let mut executed_operations = Vec::new();

            let stored_executed_txs: Vec<_> = executed_transactions::table
                .left_join(mempool::table.on(executed_transactions::tx_hash.eq(mempool::hash)))
                .filter(executed_transactions::block_number.eq(i64::from(block)))
                .load::<(StoredExecutedTransaction, Option<ReadTx>)>(self.conn())?;
            let executed_txs = stored_executed_txs
                .into_iter()
                .filter_map(|(stored_exec, stored_tx)| stored_exec.into_executed_tx(stored_tx).ok())
                .map(|tx| ExecutedOperations::Tx(Box::new(tx)));
            executed_operations.extend(executed_txs);

            let stored_executed_prior_ops: Vec<_> = executed_priority_operations::table
                .filter(executed_priority_operations::block_number.eq(i64::from(block)))
                .load::<StoredExecutedPriorityOperation>(self.conn())?;
            let executed_prior_ops = stored_executed_prior_ops
                .into_iter()
                .map(|op| ExecutedOperations::PriorityOp(Box::new(op.into())));
            executed_operations.extend(executed_prior_ops);

            executed_operations.sort_by_key(|exec_op| {
                match exec_op {
                    ExecutedOperations::Tx(tx) => {
                        if let Some(idx) = tx.block_index {
                            idx
                        } else {
                            // failed operations are at the end.
                            u32::max_value()
                        }
                    }
                    ExecutedOperations::PriorityOp(op) => op.block_index,
                }
            });

            Ok(executed_operations)
        })
    }

    pub fn get_executed_priority_op(
        &self,
        priority_op_id: u32,
    ) -> QueryResult<Option<StoredExecutedPriorityOperation>> {
        executed_priority_operations::table
            .filter(
                executed_priority_operations::priority_op_serialid.eq(i64::from(priority_op_id)),
            )
            .first::<StoredExecutedPriorityOperation>(self.conn())
            .optional()
    }

    pub fn get_block_operations(&self, block: BlockNumber) -> QueryResult<Vec<FranklinOp>> {
        let executed_ops = self.get_block_executed_ops(block)?;
        Ok(executed_ops
            .into_iter()
            .filter_map(|exec_op| match exec_op {
                ExecutedOperations::Tx(tx) => tx.op,
                ExecutedOperations::PriorityOp(priorop) => Some(priorop.op),
            })
            .collect())
    }

    pub fn commit_state_update(
        &self,
        block_number: u32,
        accounts_updated: &[(u32, AccountUpdate)],
    ) -> QueryResult<()> {
        self.conn().transaction(|| {
            for (update_order_id, (id, upd)) in accounts_updated.iter().enumerate() {
                debug!(
                    "Committing state update for account {} in block {}",
                    id, block_number
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
        info!("Applying state update for block: {}", block_number);
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
            debug!(
                "Verified state block: {}, accounts: {:#?}",
                verif_block, accounts
            );

            // Fetch updates from blocks: verif_block +/- 1, ... , block
            if let Some((block, state_diff)) = self.load_state_diff(verif_block, block)? {
                debug!("Loaded state diff: {:#?}", state_diff);
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

            debug!(
                "Loading state diff: forward: {}, start_block: {}, end_block: {}, unbounded: {}",
                time_forward,
                start_block,
                end_block,
                to_block.is_none()
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

    pub fn load_stored_op_with_block_number(
        &self,
        block_number: BlockNumber,
        action_type: ActionType,
    ) -> Option<StoredOperation> {
        use crate::schema::operations::dsl;
        dsl::operations
            .filter(dsl::block_number.eq(i64::from(block_number)))
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
            with eth_ops as (
            	select
            		operations.block_number,
                    '0x' || encode(eth_operations.tx_hash::bytea, 'hex') as tx_hash,
            		operations.action_type,
            		operations.created_at
            	from operations
            		left join eth_operations on eth_operations.op_id = operations.id
            )
            select
            	blocks.number as block_number,
            	blocks.root_hash as new_state_root,
            	commited.tx_hash as commit_tx_hash,
            	verified.tx_hash as verify_tx_hash,
            	commited.created_at as committed_at,
            	verified.created_at as verified_at
            from blocks
            inner join eth_ops commited on
            	commited.block_number = blocks.number and commited.action_type = 'COMMIT'
            left join eth_ops verified on
            	verified.block_number = blocks.number and verified.action_type = 'VERIFY'
            where
            	blocks.number <= {max_block}
            order by blocks.number desc
            limit {limit};
        ",
            max_block = i64::from(max_block),
            limit = i64::from(limit)
        );
        diesel::sql_query(query).load(self.conn())
    }

    pub fn handle_search(&self, query: String) -> Option<BlockDetails> {
        let block_number = query.parse::<i64>().unwrap_or(i64::max_value());
        let l_query = query.to_lowercase();
        let sql_query = format!(
            "
            with eth_ops as (
            	select
            		operations.block_number,
                    'sync-tx:' || encode(eth_operations.tx_hash::bytea, 'hex') as tx_hash,
            		operations.action_type,
            		operations.created_at
            	from operations
            		left join eth_operations on eth_operations.op_id = operations.id
            )
            select
            	blocks.number as block_number,
            	blocks.root_hash as new_state_root,
            	commited.tx_hash as commit_tx_hash,
            	verified.tx_hash as verify_tx_hash,
            	commited.created_at as committed_at,
            	verified.created_at as verified_at
            from blocks
            inner join eth_ops commited on
            	commited.block_number = blocks.number and commited.action_type = 'COMMIT'
            left join eth_ops verified on
            	verified.block_number = blocks.number and verified.action_type = 'VERIFY'
            where false
                or lower(commited.tx_hash) = $1
                or lower(verified.tx_hash) = $1
                or lower(blocks.root_hash) = $1
                or blocks.number = {block_number}
            order by blocks.number desc
            limit 1;
        ",
            block_number = block_number
        );
        diesel::sql_query(sql_query)
            .bind::<Text, _>(l_query)
            .get_result(self.conn())
            .ok()
    }

    pub fn load_commit_op(&self, block_number: BlockNumber) -> Option<Operation> {
        let op = self.load_stored_op_with_block_number(block_number, ActionType::COMMIT);
        op.and_then(|r| r.into_op(self).ok())
    }

    pub fn load_committed_block(&self, block_number: BlockNumber) -> Option<Block> {
        self.load_commit_op(block_number).map(|r| r.block)
    }

    pub fn load_unconfirmed_operations(
        &self,
        // TODO: move Eth transaction state to models and add it here
    ) -> QueryResult<Vec<(Operation, Vec<StorageETHOperation>)>> {
        self.conn().transaction(|| {
            let ops: Vec<_> = operations::table
                .left_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                .filter(operations::confirmed.eq(false))
                .order(operations::id.asc())
                .load::<(StoredOperation, Option<StorageETHOperation>)>(self.conn())?;

            let mut ops = ops
                .into_iter()
                .map(|(o, e)| o.into_op(self).map(|o| (o, e)))
                .collect::<QueryResult<Vec<_>>>()?;
            ops.sort_by_key(|(o, _)| o.id.unwrap()); // operations from db MUST have and id.

            Ok(ops
                .into_iter()
                .group_by(|(o, _)| o.id.unwrap())
                .into_iter()
                .map(|(_op_id, group_iter)| {
                    let fold_result = group_iter.fold(
                        (None, Vec::new()),
                        |(mut accum_op, mut accum_eth_ops): (Option<Operation>, _),
                         (op, eth_op)| {
                            if let Some(accum_op) = accum_op.as_ref() {
                                assert_eq!(accum_op.id, op.id);
                            } else {
                                accum_op = Some(op);
                            }
                            if let Some(eth_op) = eth_op {
                                accum_eth_ops.push(eth_op);
                            }

                            (accum_op, accum_eth_ops)
                        },
                    );
                    (fold_result.0.unwrap(), fold_result.1)
                })
                .collect())
        })
    }

    pub fn load_unsent_ops(&self) -> QueryResult<Vec<Operation>> {
        self.conn().transaction(|| {
            let ops: Vec<_> = operations::table
                .left_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                .filter(eth_operations::id.is_null())
                .order(operations::id.asc())
                .load::<(StoredOperation, Option<StorageETHOperation>)>(self.conn())?;
            ops.into_iter().map(|(o, _)| o.into_op(self)).collect()
        })
    }

    pub fn load_sent_unconfirmed_ops(
        &self,
    ) -> QueryResult<Vec<(Operation, Vec<StorageETHOperation>)>> {
        self.conn().transaction(|| {
            let ops: Vec<_> = operations::table
                .filter(eth_operations::confirmed.eq(false))
                .inner_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                .order(operations::id.asc())
                .load::<(StoredOperation, StorageETHOperation)>(self.conn())?;
            let mut ops_with_eth_actions = Vec::new();
            for (op, eth_op) in ops.into_iter() {
                ops_with_eth_actions.push((op.into_op(self)?, eth_op));
            }
            Ok(ops_with_eth_actions
                .into_iter()
                .group_by(|(o, _)| o.id.unwrap())
                .into_iter()
                .map(|(_op_id, group_iter)| {
                    let fold_result = group_iter.fold(
                        (None, Vec::new()),
                        |(mut accum_op, mut accum_eth_ops): (Option<Operation>, _),
                         (op, eth_op)| {
                            if let Some(accum_op) = accum_op.as_ref() {
                                assert_eq!(accum_op.id, op.id);
                            } else {
                                accum_op = Some(op);
                            }
                            accum_eth_ops.push(eth_op);

                            (accum_op, accum_eth_ops)
                        },
                    );
                    (fold_result.0.unwrap(), fold_result.1)
                })
                .collect())
        })
    }

    pub fn save_operation_eth_tx(
        &self,
        op_id: i64,
        hash: H256,
        deadline_block: u64,
        nonce: u32,
        gas_price: BigDecimal,
        raw_tx: Vec<u8>,
    ) -> QueryResult<()> {
        insert_into(eth_operations::table)
            .values(&NewETHOperation {
                op_id,
                nonce: i64::from(nonce),
                deadline_block: deadline_block as i64,
                gas_price,
                tx_hash: hash.as_bytes().to_vec(),
                raw_tx,
            })
            .execute(self.conn())
            .map(drop)
    }

    pub fn confirm_eth_tx(&self, hash: &H256) -> QueryResult<()> {
        self.conn().transaction(|| {
            update(eth_operations::table.filter(eth_operations::tx_hash.eq(hash.as_bytes())))
                .set(eth_operations::confirmed.eq(true))
                .execute(self.conn())
                .map(drop)?;
            let (op, _) = operations::table
                .inner_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                .filter(eth_operations::tx_hash.eq(hash.as_bytes()))
                .first::<(StoredOperation, StorageETHOperation)>(self.conn())?;

            update(operations::table.filter(operations::id.eq(op.id)))
                .set(operations::confirmed.eq(true))
                .execute(self.conn())
                .map(drop)
        })
    }

    pub fn load_unverified_commits_after_block(
        &self,
        block: i64,
        limit: i64,
    ) -> QueryResult<Vec<Operation>> {
        self.conn().transaction(|| {
            let ops: Vec<StoredOperation> = diesel::sql_query(format!(
                "
                SELECT * FROM operations
                  WHERE action_type = 'COMMIT'
                   AND block_number > (
                     SELECT COALESCE(max(block_number), 0)
                       FROM operations
                       WHERE action_type = 'VERIFY'
                   )
                   AND block_number > {}
                  LIMIT {}
            ",
                block, limit
            ))
            .load(self.conn())?;
            ops.into_iter().map(|o| o.into_op(self)).collect()
        })
    }

    pub fn load_unverified_commits(&self) -> QueryResult<Vec<Operation>> {
        self.load_unverified_commits_after_block(0, 10e9 as i64)
    }

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

    pub fn tx_receipt(&self, hash: &[u8]) -> QueryResult<Option<TxReceiptResponse>> {
        self.conn().transaction(|| {
            let tx = executed_transactions::table
                .filter(executed_transactions::tx_hash.eq(hash))
                .first::<StoredExecutedTransaction>(self.conn())
                .optional()?;

            if let Some(tx) = tx {
                let commited = operations::table
                    .filter(operations::block_number.eq(tx.block_number))
                    .filter(operations::action_type.eq(ActionType::COMMIT.to_string()))
                    .first::<StoredOperation>(self.conn())
                    .optional()?
                    .is_some();

                if !commited {
                    return Ok(None);
                }

                let verified = operations::table
                    .filter(operations::block_number.eq(tx.block_number))
                    .filter(operations::action_type.eq(ActionType::VERIFY.to_string()))
                    .first::<StoredOperation>(self.conn())
                    .optional()?
                    .map(|v| v.confirmed)
                    .unwrap_or(false);

                let prover_run: Option<ProverRun> = prover_runs::table
                    .filter(prover_runs::block_number.eq(tx.block_number))
                    .first::<ProverRun>(self.conn())
                    .optional()?;

                Ok(Some(TxReceiptResponse {
                    tx_hash: hex::encode(&hash),
                    block_number: tx.block_number,
                    success: tx.success,
                    verified,
                    fail_reason: tx.fail_reason,
                    prover_run,
                }))
            } else {
                Ok(None)
            }
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

    pub fn count_outstanding_proofs(&self, after_block: BlockNumber) -> QueryResult<u32> {
        use crate::schema::executed_transactions::dsl::*;
        let count: i64 = executed_transactions
            .filter(block_number.gt(i64::from(after_block)))
            .select(count_star())
            .first(self.conn())?;
        Ok(count as u32)
    }

    pub fn count_total_transactions(&self) -> QueryResult<u32> {
        let count_tx: i64 = executed_transactions::table
            .filter(executed_transactions::success.eq(true))
            .select(count_star())
            .first(self.conn())?;
        let prior_ops: i64 = executed_priority_operations::table
            .select(count_star())
            .first(self.conn())?;
        Ok((count_tx + prior_ops) as u32)
    }

    pub fn get_last_committed_block(&self) -> QueryResult<BlockNumber> {
        use crate::schema::operations::dsl::*;
        operations
            .select(max(block_number))
            .filter(action_type.eq(&ActionType::COMMIT.to_string()))
            .get_result::<Option<i64>>(self.conn())
            .map(|max| max.unwrap_or(0) as BlockNumber)
    }

    pub fn get_last_verified_block(&self) -> QueryResult<BlockNumber> {
        use crate::schema::operations::dsl::*;
        operations
            .select(max(block_number))
            .filter(action_type.eq(&ActionType::VERIFY.to_string()))
            .get_result::<Option<i64>>(self.conn())
            .map(|max| max.unwrap_or(0) as BlockNumber)
    }

    pub fn prover_run_for_next_commit(
        &self,
        worker_: &str,
        prover_timeout: time::Duration,
    ) -> QueryResult<Option<ProverRun>> {
        self.conn().transaction(|| {
            sql_query("LOCK TABLE prover_runs IN EXCLUSIVE MODE").execute(self.conn())?;
            let job: Option<BlockNumber> = diesel::sql_query(format!("
                    SELECT min(block_number) as integer_value FROM operations o
                    WHERE action_type = 'COMMIT'
                    AND block_number >
                        (SELECT COALESCE(max(block_number),0) FROM operations WHERE action_type = 'VERIFY')
                    AND NOT EXISTS 
                        (SELECT * FROM proofs WHERE block_number = o.block_number)
                    AND NOT EXISTS
                        (SELECT * FROM prover_runs 
                            WHERE block_number = o.block_number AND (now() - updated_at) < interval '{} seconds')
                ", prover_timeout.as_secs()))
                .get_result::<Option<IntegerNumber>>(self.conn())?
                .map(|i| i.integer_value as BlockNumber);
            if let Some(block_number_) = job {
                use crate::schema::prover_runs::dsl::*;
                let inserted: ProverRun = insert_into(prover_runs)
                    .values(&vec![(
                        block_number.eq(i64::from(block_number_) ),
                        worker.eq(worker_.to_string())
                    )])
                    .get_result(self.conn())?;
                Ok(Some(inserted))
            } else {
                Ok(None)
            }
        })
    }

    pub fn record_prover_is_working(&self, job_id: i32) -> QueryResult<()> {
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

    pub fn prover_by_id(&self, prover_id: i32) -> QueryResult<ActiveProver> {
        use crate::schema::active_provers::dsl::*;

        let ret: ActiveProver = active_provers
            .filter(id.eq(prover_id))
            .get_result(self.conn())?;
        Ok(ret)
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
            block_number: i64::from(block_number),
            proof: serde_json::to_value(proof).unwrap(),
        };
        use crate::schema::proofs::dsl::proofs;
        insert_into(proofs).values(&to_store).execute(self.conn())
    }

    pub fn load_proof(&self, block_number: BlockNumber) -> QueryResult<EncodedProof> {
        use crate::schema::proofs::dsl;
        let stored: StoredProof = dsl::proofs
            .filter(dsl::block_number.eq(i64::from(block_number)))
            .get_result(self.conn())?;
        Ok(serde_json::from_value(stored.proof).unwrap())
    }

    pub fn store_token(&self, id: TokenId, address: &str, symbol: Option<&str>) -> QueryResult<()> {
        let new_token = Token {
            id: i32::from(id),
            address: address.to_string(),
            symbol: symbol.map(String::from),
        };
        diesel::insert_into(tokens::table)
            .values(&new_token)
            .on_conflict(tokens::id)
            .do_update()
            // update token address but not symbol -- so we can update it externally
            .set(tokens::address.eq(new_token.address.clone()))
            .execute(self.conn())
            .map(drop)
    }

    pub fn load_tokens(&self) -> QueryResult<HashMap<TokenId, Token>> {
        let tokens = tokens::table
            .order(tokens::id.asc())
            .load::<Token>(self.conn())?;
        Ok(tokens.into_iter().map(|t| (t.id as TokenId, t)).collect())
    }

    // Data restore part

    fn save_operation(&self, op: &Operation) -> QueryResult<()> {
        self.conn().transaction(|| {
            match &op.action {
                Action::Commit => {
                    self.commit_state_update(op.block.block_number, &op.accounts_updated)?;
                    self.save_block(&op.block)?;
                }
                Action::Verify { .. } => self.apply_state_update(op.block.block_number)?,
            };

            let _stored: StoredOperation = diesel::insert_into(operations::table)
                .values(&NewOperation {
                    block_number: i64::from(op.block.block_number),
                    action_type: op.action.to_string(),
                })
                .get_result(self.conn())?;
            Ok(())
        })
    }

    fn update_block_events(&self, events: &[NewBlockEvent]) -> QueryResult<()> {
        self.conn().transaction(|| {
            diesel::delete(events_state::table).execute(self.conn())?;
            for event in events.iter() {
                diesel::insert_into(events_state::table)
                    .values(event)
                    .execute(self.conn())?;
            }
            Ok(())
        })
    }

    fn update_last_watched_block_number(
        &self,
        number: &NewLastWatchedEthBlockNumber,
    ) -> QueryResult<()> {
        self.conn().transaction(|| {
            diesel::delete(data_restore_last_watched_eth_block::table).execute(self.conn())?;
            diesel::insert_into(data_restore_last_watched_eth_block::table)
                .values(number)
                .execute(self.conn())?;
            Ok(())
        })
    }

    fn update_storage_state(&self, state: NewStorageState) -> QueryResult<()> {
        self.conn().transaction(|| {
            diesel::delete(storage_state_update::table).execute(self.conn())?;
            diesel::insert_into(storage_state_update::table)
                .values(state)
                .execute(self.conn())?;
            Ok(())
        })
    }

    pub fn save_genesis_state(&self, genesis_acc_update: AccountUpdate) -> QueryResult<()> {
        self.conn().transaction(|| {
            self.commit_state_update(0, &[(0, genesis_acc_update)])?;
            self.apply_state_update(0)?;
            Ok(())
        })
    }

    pub fn save_block_transactions_with_data_restore_state(
        &self,
        block: &Block,
    ) -> QueryResult<()> {
        self.conn().transaction(|| {
            self.save_block_transactions(block)?;
            let state = NewStorageState {
                storage_state: "None".to_string(),
            };
            self.update_storage_state(state)?;
            Ok(())
        })
    }

    pub fn save_block_operations_with_data_restore_state(
        &self,
        commit_op: &Operation,
        verify_op: &Operation,
    ) -> QueryResult<()> {
        self.conn().transaction(|| {
            self.save_operation(commit_op)?;
            self.save_operation(verify_op)?;
            let state = NewStorageState {
                storage_state: "None".to_string(),
            };
            self.update_storage_state(state)?;
            Ok(())
        })
    }

    pub fn save_events_state_with_data_restore_state(
        &self,
        block_events: &[NewBlockEvent],
        token_events: &[TokenAddedEvent],
        last_watched_eth_number: &NewLastWatchedEthBlockNumber,
    ) -> QueryResult<()> {
        self.conn().transaction(|| {
            self.update_block_events(block_events)?;

            for token in token_events.iter() {
                self.store_token(token.id as u16, &format!("0x{:x}", token.address), None)?;
            }

            self.update_last_watched_block_number(last_watched_eth_number)?;

            let state = NewStorageState {
                storage_state: "Events".to_string(),
            };
            self.update_storage_state(state)?;

            Ok(())
        })
    }

    pub fn save_rollup_ops_with_data_restore_state(
        &self,
        ops: &[(BlockNumber, &FranklinOp, AccountId)],
    ) -> QueryResult<()> {
        self.conn().transaction(|| {
            diesel::delete(rollup_ops::table).execute(self.conn())?;
            for op in ops.iter() {
                let stored_op = NewFranklinOp::prepare_stored_op(&op.1, op.0, op.2);
                diesel::insert_into(rollup_ops::table)
                    .values(&stored_op)
                    .execute(self.conn())?;
            }
            let state = NewStorageState {
                storage_state: "Operations".to_string(),
            };
            self.update_storage_state(state)?;
            Ok(())
        })
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

    pub fn load_last_watched_block_number(&self) -> QueryResult<StoredLastWatchedEthBlockNumber> {
        data_restore_last_watched_eth_block::table.first(self.conn())
    }

    pub fn load_rollup_ops_blocks(&self) -> QueryResult<Vec<StoredRollupOpsBlock>> {
        let stored_operations = rollup_ops::table
            .order(rollup_ops::id.asc())
            .load::<StoredFranklinOp>(self.conn())?;
        let ops_blocks: Vec<StoredRollupOpsBlock> = stored_operations
            .into_iter()
            .group_by(|op| op.block_num)
            .into_iter()
            .map(|(_, stored_ops)| {
                // let stored_ops = group.clone();
                // let mut ops: Vec<FranklinOp> = vec![];
                let mut block_num: i64 = 0;
                let mut fee_account: i64 = 0;
                let ops: Vec<FranklinOp> = stored_ops
                    .map(|stored_op| {
                        block_num = stored_op.block_num;
                        fee_account = stored_op.fee_account;
                        stored_op.into_franklin_op()
                    })
                    .collect();
                StoredRollupOpsBlock {
                    block_num: block_num as u32,
                    ops,
                    fee_account: fee_account as u32,
                }
            })
            .collect();
        Ok(ops_blocks)
    }
}

#[cfg(test)]
/// This tests require empty DB setup and ignored by default
/// use `zksync db-test-no-reset`/`franklin db-test` script to run them
mod test {
    use super::*;
    use diesel::Connection;
    use models::primitives::u128_to_bigdecimal;
    use rand::prelude::*;

    fn acc_create_random_updates<R: Rng>(
        rng: &mut R,
    ) -> impl Iterator<Item = (u32, AccountUpdate)> {
        let id: u32 = rng.gen();
        let balance: u128 = rng.gen();
        let nonce: u32 = rng.gen();
        let pub_key_hash = PubKeyHash { data: rng.gen() };
        let address: Address = rng.gen::<[u8; 20]>().into();

        let mut a = models::node::account::Account::default_with_address(&address);
        let old_nonce = nonce;
        a.nonce = old_nonce + 2;
        a.pub_key_hash = pub_key_hash;

        let old_balance = a.get_balance(0);
        a.set_balance(0, u128_to_bigdecimal(balance));
        let new_balance = a.get_balance(0);
        vec![
            (
                id,
                AccountUpdate::Create {
                    nonce: old_nonce,
                    address: a.address,
                },
            ),
            (
                id,
                AccountUpdate::ChangePubKeyHash {
                    old_nonce,
                    old_pub_key_hash: PubKeyHash::default(),
                    new_nonce: old_nonce + 1,
                    new_pub_key_hash: a.pub_key_hash,
                },
            ),
            (
                id,
                AccountUpdate::UpdateBalance {
                    old_nonce: old_nonce + 1,
                    new_nonce: old_nonce + 2,
                    balance_update: (0, old_balance, new_balance),
                },
            ),
        ]
        .into_iter()
    }

    #[test]
    #[cfg_attr(not(feature = "db_test"), ignore)]
    // Here we create updates for blocks 1,2,3 (commit 3 blocks)
    // We apply updates for blocks 1,2 (verify 2 blocks)
    // Make sure that we can get state for all blocks.
    fn test_commit_rewind() {
        let _ = env_logger::try_init();

        let mut rng = StdRng::seed_from_u64(0x1234);

        let pool = ConnectionPool::new();
        let conn = pool.access_storage().unwrap();
        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test

        let (accounts_block_1, updates_block_1) = {
            let mut accounts = AccountMap::default();
            let updates = {
                let mut updates = Vec::new();
                updates.extend(acc_create_random_updates(&mut rng));
                updates.extend(acc_create_random_updates(&mut rng));
                updates.extend(acc_create_random_updates(&mut rng));
                updates
            };
            apply_updates(&mut accounts, updates.clone());
            (accounts, updates)
        };

        let (accounts_block_2, updates_block_2) = {
            let mut accounts = accounts_block_1.clone();
            let updates = {
                let mut updates = Vec::new();
                updates.extend(acc_create_random_updates(&mut rng));
                updates.extend(acc_create_random_updates(&mut rng));
                updates.extend(acc_create_random_updates(&mut rng));
                updates
            };
            apply_updates(&mut accounts, updates.clone());
            (accounts, updates)
        };
        let (accounts_block_3, updates_block_3) = {
            let mut accounts = accounts_block_2.clone();
            let updates = {
                let mut updates = Vec::new();
                updates.extend(acc_create_random_updates(&mut rng));
                updates.extend(acc_create_random_updates(&mut rng));
                updates.extend(acc_create_random_updates(&mut rng));
                updates
            };
            apply_updates(&mut accounts, updates.clone());
            (accounts, updates)
        };

        let save_test_block = |block_number| {
            conn.save_block(&Block {
                block_number,
                new_root_hash: Fr::default(),
                fee_account: 0,
                block_transactions: Vec::new(),
                processed_priority_ops: (0, 0),
            })
            .unwrap();
        };

        conn.commit_state_update(1, &updates_block_1).unwrap();
        save_test_block(1);
        conn.commit_state_update(2, &updates_block_2).unwrap();
        save_test_block(2);
        conn.commit_state_update(3, &updates_block_3).unwrap();
        save_test_block(3);

        let (block, state) = conn.load_committed_state(Some(1)).unwrap();
        assert_eq!((block, &state), (1, &accounts_block_1));

        let (block, state) = conn.load_committed_state(Some(2)).unwrap();
        assert_eq!((block, &state), (2, &accounts_block_2));

        let (block, state) = conn.load_committed_state(Some(3)).unwrap();
        assert_eq!((block, &state), (3, &accounts_block_3));

        conn.apply_state_update(1).unwrap();
        conn.apply_state_update(2).unwrap();

        let (block, state) = conn.load_committed_state(Some(1)).unwrap();
        assert_eq!((block, &state), (1, &accounts_block_1));

        let (block, state) = conn.load_committed_state(Some(2)).unwrap();
        assert_eq!((block, &state), (2, &accounts_block_2));

        let (block, state) = conn.load_committed_state(Some(3)).unwrap();
        assert_eq!((block, &state), (3, &accounts_block_3));

        let (block, state) = conn.load_committed_state(None).unwrap();
        assert_eq!((block, &state), (3, &accounts_block_3));
    }

    #[test]
    #[ignore]
    // TODO: Implement
    fn test_eth_sender_storage() {}

    #[test]
    #[cfg_attr(not(feature = "db_test"), ignore)]
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
}
