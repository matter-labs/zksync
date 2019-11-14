#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;

use bigdecimal::BigDecimal;
use chrono::prelude::*;
use diesel::dsl::*;
use failure::{bail, Fail};
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use models::node::{
    apply_updates, reverse_updates, tx::FranklinTx, Account, AccountId, AccountMap, AccountUpdate,
    AccountUpdates, BlockNumber, Fr, FranklinOp, PriorityOp, TokenId,
};
use models::{Action, ActionType, EncodedProof, Operation, ACTION_COMMIT, ACTION_VERIFY};
use serde_derive::{Deserialize, Serialize};
use std::cmp;
use std::convert::TryInto;
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
    pub id: i64,
    pub last_block: i64,
    pub nonce: i64,
    pub address: Vec<u8>,
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
}

#[derive(Debug, Insertable, Queryable, QueryableByName)]
#[table_name = "account_creates"]
struct StorageAccountCreation {
    account_id: i64,
    is_create: bool,
    block_number: i64,
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
    block_index: Option<i32>,
}

impl NewExecutedTransaction {
    fn prepare_stored_tx(exec_tx: &ExecutedTx, block: BlockNumber) -> Self {
        Self {
            block_number: i64::from(block),
            tx_hash: exec_tx.tx.hash(),
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
struct StoredExecutedPriorityOperation {
    id: i32,
    block_number: i64,
    block_index: i32,
    operation: Value,
    priority_op_serialid: i64,
    deadline_block: i64,
    eth_fee: BigDecimal,
    eth_hash: Vec<u8>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct TxReceiptResponse {
    tx_hash: Vec<u8>,
    block_number: i64,
    success: bool,
    verified: bool,
    fail_reason: Option<String>,
    prover_run: Option<ProverRun>,
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

    fn block_number(&self) -> i64 {
        *match self {
            StorageAccountDiff::BalanceUpdate(StorageAccountUpdate { block_number, .. }) => {
                block_number
            }
            StorageAccountDiff::Create(StorageAccountCreation { block_number, .. }) => block_number,
            StorageAccountDiff::Delete(StorageAccountCreation { block_number, .. }) => block_number,
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
    pub tx_hash: String,
    pub confirmed: bool,
}

#[derive(Debug, Insertable)]
#[table_name = "eth_operations"]
struct NewETHOperation {
    op_id: i64,
    nonce: i64,
    deadline_block: i64,
    gas_price: BigDecimal,
    tx_hash: String,
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
#[derive(Debug, Insertable, Queryable, QueryableByName, Serialize, Deserialize)]
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
#[table_name = "franklin_ops"]
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
#[table_name = "franklin_ops"]
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
pub struct StoredFranklinOpsBlock {
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

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "mempool"]
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
    #[fail(display = "Tx is incorrect")]
    IncorrectTx,
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
                root_hash: block.new_root_hash.to_hex(),
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
                    .filter(operations::action_type.eq("Commit"))
                    .first::<StoredOperation>(self.conn())
                    .optional()?;

                let confirm = operations::table
                    .filter(operations::block_number.eq(stored_executed_prior_op.block_number))
                    .filter(operations::action_type.eq("Verify"))
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
                    operation["tx"]["eth_address"]
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
                block_number: block_number,
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
                block_number: block_number,
            }));
        };

        Ok(None)
    }

    pub fn get_account_transactions_history(
        &self,
        address: &AccountAddress,
        offset: i64,
        limit: i64,
    ) -> QueryResult<Vec<TransactionsHistoryItem>> {
        // TODO: txs are not ordered
        let query = format!(
            "
            select
                encode(hash, 'hex') as hash,
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
                        hash,
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
                        encode(primary_account_address, 'hex') = '{address}'
                        or 
                        tx->>'to' = '0x{address}'
                    union all
                    select 
                        operation as tx,
                        eth_hash as hash,
                        priority_op_serialid as pq_id,
                        null as success,
                        null as fail_reason,
                        block_number
                    from 
                        executed_priority_operations
                    where 
                        operation->'priority_op'->>'account' = '0x{address}') t
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
            address = hex::encode(address.data),
            offset = offset,
            limit = limit
        );

        diesel::sql_query(query).load::<TransactionsHistoryItem>(self.conn())
    }

    pub fn get_account_transactions(
        &self,
        address: &AccountAddress,
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
                    if operation.action_type == "Commit" {
                        res.committed = operation.confirmed;
                    } else {
                        res.verified = operation.confirmed;
                    }
                }
                if let Some((_mempool_tx, _executed_tx, operation)) = group_iter.next() {
                    if let Some(operation) = operation {
                        if operation.action_type == "Commit" {
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
            new_root_hash: Fr::from_hex(&stored_block.root_hash).expect("Unparsable root hash"),
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
            for (id, upd) in accounts_updated.iter() {
                debug!(
                    "Committing state update for account {} in block {}",
                    id, block_number
                );
                match *upd {
                    AccountUpdate::Create { ref address, nonce } => {
                        diesel::insert_into(account_creates::table)
                            .values(&StorageAccountCreation {
                                account_id: i64::from(*id),
                                is_create: true,
                                block_number: i64::from(block_number),
                                address: address.data.to_vec(),
                                nonce: i64::from(nonce),
                            })
                            .execute(self.conn())?;
                    }
                    AccountUpdate::Delete { ref address, nonce } => {
                        diesel::insert_into(account_creates::table)
                            .values(&StorageAccountCreation {
                                account_id: i64::from(*id),
                                is_create: false,
                                block_number: i64::from(block_number),
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
                        .gt(&(i64::from(start_block)))
                        .and(
                            account_balance_updates::block_number
                                .le(&(i64::from(end_block)))
                                .or(unbounded),
                        ),
                )
                .load::<StorageAccountUpdate>(self.conn())?;
            let account_creation_diff = account_creates::table
                .filter(
                    account_creates::block_number
                        .gt(&(i64::from(start_block)))
                        .and(
                            account_creates::block_number
                                .le(&(i64::from(end_block)))
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
            		eth_operations.tx_hash,
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
            	commited.block_number = blocks.number and commited.action_type = 'Commit'
            left join eth_ops verified on
            	verified.block_number = blocks.number and verified.action_type = 'Verify'
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
            		eth_operations.tx_hash,
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
            	commited.block_number = blocks.number and commited.action_type = 'Commit'
            left join eth_ops verified on
            	verified.block_number = blocks.number and verified.action_type = 'Verify'
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
        let op = self.load_commit_op(block_number);
        op.and_then(|r| Some(r.block))
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
    ) -> QueryResult<()> {
        insert_into(eth_operations::table)
            .values(&NewETHOperation {
                op_id,
                nonce: i64::from(nonce),
                deadline_block: deadline_block as i64,
                gas_price,
                tx_hash: format!("{:#x}", hash),
            })
            .execute(self.conn())
            .map(drop)
    }

    pub fn confirm_eth_tx(&self, hash: &H256) -> QueryResult<()> {
        self.conn().transaction(|| {
            update(
                eth_operations::table.filter(eth_operations::tx_hash.eq(format!("{:#x}", hash))),
            )
            .set(eth_operations::confirmed.eq(true))
            .execute(self.conn())
            .map(drop)?;
            let (op, _) = operations::table
                .inner_join(eth_operations::table.on(eth_operations::op_id.eq(operations::id)))
                .filter(eth_operations::tx_hash.eq(format!("{:#x}", hash)))
                .first::<(StoredOperation, StorageETHOperation)>(self.conn())?;

            update(operations::table.filter(operations::id.eq(op.id)))
                .set(operations::confirmed.eq(true))
                .execute(self.conn())
                .map(drop)
        })
    }

    pub fn load_unverified_commitments(&self) -> QueryResult<Vec<Operation>> {
        self.conn().transaction(|| {
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

    pub fn tx_receipt(&self, hash: &[u8]) -> QueryResult<Option<TxReceiptResponse>> {
        self.conn().transaction(|| {
            let tx = executed_transactions::table
                .filter(executed_transactions::tx_hash.eq(hash))
                .first::<StoredExecutedTransaction>(self.conn())
                .optional()?;

            if let Some(tx) = tx {
                let confirm = operations::table
                    .filter(operations::block_number.eq(tx.block_number))
                    .filter(operations::action_type.eq("Verify"))
                    .first::<StoredOperation>(self.conn())
                    .optional()?;

                let prover_run: Option<ProverRun> = prover_runs::table
                    .filter(prover_runs::block_number.eq(tx.block_number))
                    .first::<ProverRun>(self.conn())
                    .optional()?;

                Ok(Some(TxReceiptResponse {
                    tx_hash: hash.to_vec(),
                    block_number: tx.block_number,
                    success: tx.success,
                    verified: confirm.is_some(),
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
            .filter(action_type.eq(ACTION_COMMIT))
            .get_result::<Option<i64>>(self.conn())
            .map(|max| max.unwrap_or(0) as BlockNumber)
    }

    pub fn get_last_verified_block(&self) -> QueryResult<BlockNumber> {
        use crate::schema::operations::dsl::*;
        operations
            .select(max(block_number))
            .filter(action_type.eq(ACTION_VERIFY))
            .get_result::<Option<i64>>(self.conn())
            .map(|max| max.unwrap_or(0) as BlockNumber)
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
                //     block_number: i64::from(block_number),
                //     worker: worker.to_string(),
                // };
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

    pub fn save_events_state(&self, events: &[NewBlockEvent]) -> QueryResult<()> {
        for event in events.iter() {
            diesel::insert_into(events_state::table)
                .values(event)
                .execute(self.conn())?;
        }
        Ok(())
    }

    pub fn delete_events_state(&self) -> QueryResult<()> {
        diesel::delete(events_state::table).execute(self.conn())?;
        Ok(())
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

    pub fn save_storage_state(&self, state: &NewStorageState) -> QueryResult<()> {
        diesel::insert_into(storage_state_update::table)
            .values(state)
            .execute(self.conn())?;
        Ok(())
    }

    pub fn delete_data_restore_storage_state_status(&self) -> QueryResult<()> {
        diesel::delete(storage_state_update::table).execute(self.conn())?;
        Ok(())
    }

    pub fn load_storage_state(&self) -> QueryResult<StoredStorageState> {
        storage_state_update::table.first(self.conn())
    }

    pub fn save_last_watched_block_number(
        &self,
        number: &NewLastWatchedEthBlockNumber,
    ) -> QueryResult<()> {
        diesel::insert_into(data_restore_last_watched_eth_block::table)
            .values(number)
            .execute(self.conn())?;
        Ok(())
    }

    pub fn delete_last_watched_block_number(&self) -> QueryResult<()> {
        diesel::delete(data_restore_last_watched_eth_block::table).execute(self.conn())?;
        Ok(())
    }

    pub fn load_last_watched_block_number(&self) -> QueryResult<StoredLastWatchedEthBlockNumber> {
        data_restore_last_watched_eth_block::table.first(self.conn())
    }

    pub fn save_franklin_ops_block(
        &self,
        ops: &[FranklinOp],
        block_num: BlockNumber,
        fee_account: AccountId,
    ) -> QueryResult<()> {
        for op in ops.iter() {
            let stored_op = NewFranklinOp::prepare_stored_op(&op, block_num, fee_account);
            diesel::insert_into(franklin_ops::table)
                .values(&stored_op)
                .execute(self.conn())?;
        }
        Ok(())
    }

    pub fn delete_franklin_ops(&self) -> QueryResult<()> {
        diesel::delete(franklin_ops::table).execute(self.conn())?;
        Ok(())
    }

    pub fn load_franklin_ops_blocks(&self) -> QueryResult<Vec<StoredFranklinOpsBlock>> {
        let stored_operations = franklin_ops::table
            .order(franklin_ops::id.asc())
            .load::<StoredFranklinOp>(self.conn())?;
        let ops_blocks: Vec<StoredFranklinOpsBlock> = stored_operations
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
                StoredFranklinOpsBlock {
                    block_num: block_num as u32,
                    ops,
                    fee_account: fee_account as u32,
                }
            })
            .collect();
        Ok(ops_blocks)
    }

    pub fn update_tree_state(
        &self,
        block_number: BlockNumber,
        updates: &[(u32, AccountUpdate)],
    ) -> QueryResult<()> {
        self.commit_state_update(block_number, &updates)?;
        self.apply_state_update(block_number)?;
        Ok(())
    }

    pub fn load_tree_state(&self) -> QueryResult<(u32, AccountMap)> {
        Ok(self.load_verified_state()?)
    }

    pub fn delete_tree_state(&self) -> QueryResult<()> {
        diesel::delete(balances::table).execute(self.conn())?;
        diesel::delete(accounts::table).execute(self.conn())?;
        diesel::delete(account_creates::table).execute(self.conn())?;
        diesel::delete(account_balance_updates::table).execute(self.conn())?;
        Ok(())
    }

    pub fn store_token(&self, id: TokenId, address: &str, symbol: Option<&str>) -> QueryResult<()> {
        let new_token = Token {
            id: i32::from(id),
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

        if !tx.check_correctness() {
            return Ok(Err(TxAddError::IncorrectTx));
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
        let commited_nonce = commited_state
            .map(|a| i64::from(a.nonce))
            .unwrap_or_default();

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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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

    //    fn dummy_op(_action: Action, _block_number: BlockNumber) -> Operation {
    //        unimplemented!()
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
    //    }
}
