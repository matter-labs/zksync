//! This module contains the structures that represent the contents
//! of the tables. Each structure is associated with one of the tables
//! used in project and is used to interact with the database.

use bigdecimal::BigDecimal;
use chrono::prelude::*;

use models::node::block::{ExecutedPriorityOp, ExecutedTx};
use models::node::{AccountId, BlockNumber, FranklinOp, FranklinTx, PriorityOp};
use models::{Action, ActionType, Operation};
use serde_derive::{Deserialize, Serialize};

use crate::schema::*;

use diesel::prelude::*;

use serde_json::value::Value;

use diesel::sql_types::{BigInt, Bool, Int4, Jsonb, Nullable, Text, Timestamp};

// TODO this module should not know about storage processor.
use super::StorageProcessor;

#[derive(Identifiable, Insertable, QueryableByName, Queryable)]
#[table_name = "accounts"]
pub struct StorageAccount {
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
pub struct StorageBalance {
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
    pub symbol: String,
}

#[derive(Debug, Insertable)]
#[table_name = "account_balance_updates"]
pub struct StorageAccountUpdateInsert {
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
pub struct StorageAccountUpdate {
    pub balance_update_id: i32,
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
pub struct StorageAccountPubkeyUpdateInsert {
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
pub struct StorageAccountPubkeyUpdate {
    pub pubkey_update_id: i32,
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
pub struct StorageAccountCreation {
    pub account_id: i64,
    pub is_create: bool,
    pub block_number: i64,
    pub address: Vec<u8>,
    pub nonce: i64,
    pub update_order_id: i32,
}

#[derive(Debug, Insertable)]
#[table_name = "executed_transactions"]
pub struct NewExecutedTransaction {
    pub block_number: i64,
    pub tx_hash: Vec<u8>,
    pub operation: Option<Value>,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub block_index: Option<i32>,
}

impl NewExecutedTransaction {
    pub fn prepare_stored_tx(exec_tx: &ExecutedTx, block: BlockNumber) -> Self {
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
pub struct StoredExecutedTransaction {
    pub id: i32,
    pub block_number: i64,
    pub tx_hash: Vec<u8>,
    pub operation: Option<Value>,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub block_index: Option<i32>,
}

impl StoredExecutedTransaction {
    pub fn into_executed_tx(self, stored_tx: Option<ReadTx>) -> Result<ExecutedTx, failure::Error> {
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
            failure::bail!("Unsuccessful tx was lost from db.");
        }
    }
}

#[derive(Debug, Insertable)]
#[table_name = "executed_priority_operations"]
pub struct NewExecutedPriorityOperation {
    pub block_number: i64,
    pub block_index: i32,
    pub operation: Value,
    pub priority_op_serialid: i64,
    pub deadline_block: i64,
    pub eth_fee: BigDecimal,
    pub eth_hash: Vec<u8>,
}

impl NewExecutedPriorityOperation {
    pub fn prepare_stored_priority_op(
        exec_prior_op: &ExecutedPriorityOp,
        block: BlockNumber,
    ) -> Self {
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
    pub committed: bool,
    pub verified: bool,
    pub prover_run: Option<ProverRun>,
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

#[derive(Debug, Insertable)]
#[table_name = "operations"]
pub struct NewOperation {
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
pub struct NewETHOperation {
    pub op_id: i64,
    pub nonce: i64,
    pub deadline_block: i64,
    pub gas_price: BigDecimal,
    pub tx_hash: Vec<u8>,
    pub raw_tx: Vec<u8>,
}

#[derive(Debug, Insertable, Queryable)]
#[table_name = "blocks"]
pub struct StorageBlock {
    pub number: i64,
    pub root_hash: String,
    pub fee_account_id: i64,
    pub unprocessed_prior_op_before: i64,
    pub unprocessed_prior_op_after: i64,
    pub block_size: i64,
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
    pub block_size: i64,
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

    #[sql_type = "BigInt"]
    pub block_size: i64,

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
    pub fn prepare_stored_op(
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
    pub fn into_franklin_op(self) -> FranklinOp {
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
pub struct InsertTx {
    pub hash: Vec<u8>,
    pub primary_account_address: Vec<u8>,
    pub nonce: i64,
    pub tx: Value,
}

#[derive(Debug, Queryable)]
pub struct ReadTx {
    pub hash: Vec<u8>,
    pub primary_account_address: Vec<u8>,
    pub nonce: i64,
    pub tx: Value,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountTransaction {
    pub tx: Value,
    pub tx_hash: String,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub committed: bool,
    pub verified: bool,
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
