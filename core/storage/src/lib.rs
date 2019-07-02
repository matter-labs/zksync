#![allow(clippy::wrong_self_convention)] // All methods with name into_* must consume self.

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;

use bigdecimal::BigDecimal;
use chrono::prelude::*;
use diesel::dsl::*;
use models::plasma::account::{TokenId, ETH_TOKEN_ID};
use models::plasma::block::Block;
use models::plasma::block::BlockData;
use models::plasma::tx::TransactionType::{Deposit, Exit, Transfer};
use models::plasma::tx::{
    DepositTx, ExitTx, TransactionType, TransferTx, TxSignature, DEPOSIT_TX, EXIT_TX, TRANSFER_TX,
};
use models::plasma::{Account, AccountId, AccountMap, AccountUpdate, BlockNumber, Fr, Nonce};
use models::{Action, ActionType, EncodedProof, Operation, TxMeta, ACTION_COMMIT, ACTION_VERIFY};
use serde_derive::{Deserialize, Serialize};
use std::cmp;

mod schema;
use crate::schema::*;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError, PooledConnection};
use diesel::result::Error;

use serde_json::{to_value, value::Value};
use std::env;
use std::iter::FromIterator;

use ff::Field;

use diesel::sql_types::{Integer, Nullable, Text, Timestamp};
sql_function!(coalesce, Coalesce, (x: Nullable<Integer>, y: Integer) -> Integer);

mod mempool;

pub use mempool::Mempool;

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

    pub fn access_mempool(&self) -> Result<Mempool, PoolError> {
        Ok(Mempool::from_db_connect_pool(self.pool.get()?))
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
    pub pk_x: Vec<u8>,
    pub pk_y: Vec<u8>,
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

#[derive(Insertable, QueryableByName, Queryable)]
#[table_name = "tokens"]
struct Token {
    pub id: i32,
    pub address: String,
}

#[derive(Insertable, Queryable, QueryableByName)]
#[table_name = "account_balance_updates"]
struct StorageAccountUpdate {
    pub account_id: i32,
    pub block_number: i32,
    pub coin_id: i32,
    pub old_balance: BigDecimal,
    pub new_balance: BigDecimal,
    pub nonce: i64,
    // TODO (Drogan) add nonce reconstruction.
}

#[derive(Insertable, Queryable, QueryableByName)]
#[table_name = "account_creates"]
struct StorageAccountCreation {
    account_id: i32,
    is_create: bool,
    block_number: i32,
    pk_x: Vec<u8>,
    pk_y: Vec<u8>,
    nonce: i64,
    // TODO (Drogan) add nonce reconstruction.
}

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

impl Into<AccountUpdate> for StorageAccountDiff {
    fn into(self) -> AccountUpdate {
        match self {
            StorageAccountDiff::BalanceUpdate(upd) => AccountUpdate::UpdateBalance {
                id: 0,
                nonce: upd.nonce as u32,
                balance_update: (upd.coin_id as TokenId, upd.old_balance, upd.new_balance),
            },
            StorageAccountDiff::Create(upd) => AccountUpdate::Create {
                id: 0,
                nonce: upd.nonce as u32,
                public_key_x: fr_from_bytes(upd.pk_x),
                public_key_y: fr_from_bytes(upd.pk_y),
            },
            StorageAccountDiff::Delete(upd) => AccountUpdate::Delete {
                id: 0,
                nonce: upd.nonce as u32,
                public_key_x: fr_from_bytes(upd.pk_x),
                public_key_y: fr_from_bytes(upd.pk_y),
            },
        }
    }
}

impl StorageAccountDiff {
    fn nonce(&self) -> i64 {
        *match self {
            StorageAccountDiff::BalanceUpdate(StorageAccountUpdate { nonce, .. }) => nonce,
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

        if op.accounts_updated.is_none() {
            let updates = conn.load_state_diff_for_block(op.block.block_number)?;
            op.accounts_updated = Some(updates);
        };

        Ok(op)
    }
}

#[derive(Insertable)]
#[table_name = "transactions"]
struct NewTx {
    pub tx_type: String, // 'transfer', 'deposit', 'exit'
    pub from_account: i32,
    pub to_account: Option<i32>, // only used for transfers
    pub nonce: Option<i32>,      // only used for transfers
    pub amount: i32,
    pub fee: i32,

    pub block_number: Option<i32>,
    pub state_root: Option<String>, // unique block id (for possible reorgs)
}

#[derive(Serialize, Deserialize, Debug, Clone, Queryable, QueryableByName)]
#[table_name = "transactions"]
pub struct StoredTx {
    pub id: i32,
    //pub data:           serde_json::Value,
    pub tx_type: String, // 'transfer', 'deposit', 'exit'
    pub from_account: i32,
    pub to_account: Option<i32>, // only used for transfers
    pub nonce: Option<i32>,      // only used for transfers
    pub amount: i32,
    pub fee: i32,

    pub block_number: Option<i32>,
    pub state_root: Option<String>, // unique block id (for possible reorgs)

    pub created_at: NaiveDateTime,
}

impl StoredTx {
    pub fn into_tx(&self) -> QueryResult<TransactionType> {
        let res = match &self.tx_type {
            t if t == TRANSFER_TX => Transfer {
                tx: Box::new(self.into_transfer_transaction()),
            },
            d if d == DEPOSIT_TX => Deposit {
                tx: self.into_deposit_transaction(),
            },
            e if e == EXIT_TX => Exit {
                tx: self.into_exit_transaction(),
            },
            _ => return Err(Error::NotFound),
        };
        Ok(res)
    }

    pub fn into_transfer_transaction(&self) -> TransferTx {
        TransferTx {
            from: self.from_account as u32,
            to: self.to_account.unwrap() as u32,
            amount: BigDecimal::from(self.amount),
            fee: BigDecimal::from(self.fee),
            nonce: 0,
            good_until_block: 0,
            signature: TxSignature::default(),
            cached_pub_key: None,
        }
    }

    pub fn into_deposit_transaction(&self) -> DepositTx {
        DepositTx {
            account: self.from_account as u32,
            amount: BigDecimal::from(self.amount),
            pub_x: Fr::zero(),
            pub_y: Fr::zero(),
        }
    }

    pub fn into_exit_transaction(&self) -> ExitTx {
        ExitTx {
            account: self.from_account as u32,
            amount: BigDecimal::from(self.amount),
        }
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

enum ConnectionHolder {
    Pooled(PooledConnection<ConnectionManager<PgConnection>>),
    Direct(PgConnection),
}

pub struct StorageProcessor {
    conn: ConnectionHolder,
}

// TODO: (Drogan) move this tmp methods
fn fr_to_bytes(fr: &Fr) -> Vec<u8> {
    use ff::{PrimeField, PrimeFieldRepr};
    let mut buff: Vec<u8> = Vec::new();
    fr.into_repr().write_be(&mut buff).unwrap();
    buff
}

// TODO: (Drogan) move this tmp methods
fn fr_from_bytes(mut bytes: Vec<u8>) -> Fr {
    use ff::{PrimeField, PrimeFieldRepr};
    let mut fr_repr = <Fr as PrimeField>::Repr::default();
    fr_repr.read_be(&*bytes).unwrap();
    Fr::from_repr(fr_repr).unwrap()
}

fn restore_account(
    stored_account: StorageAccount,
    stored_balances: Vec<StorageBalance>,
) -> (u32, Account) {
    let mut account = Account::default();
    for b in stored_balances.into_iter() {
        assert_eq!(b.account_id, stored_account.id);
        account.set_balance(b.coin_id as TokenId, &b.balance);
    }
    account.nonce = stored_account.nonce as u32;
    account.public_key_x = fr_from_bytes(stored_account.pk_x);
    account.public_key_y = fr_from_bytes(stored_account.pk_y);
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
                    self.commit_state_update(
                        op.block.block_number,
                        // TODO: (Drogan) make shure that this unwrap is ok(i.e. we always update accounts when commiting)
                        op.accounts_updated.as_ref().unwrap(),
                    )?;
                    self.save_transactions(op)?;
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

    fn save_transactions(&self, op: &Operation) -> QueryResult<()> {
        let block_data = &op.block.block_data;
        match block_data {
            BlockData::Transfer { transactions, .. } => {
                self.save_transfer_transactions(op, &transactions)?
            }
            BlockData::Deposit { transactions, .. } => {
                self.save_deposit_transactions(op, &transactions)?
            }
            BlockData::Exit { transactions, .. } => {
                self.save_exit_transactions(op, &transactions)?
            }
        }
        Ok(())
    }

    fn save_transfer_transactions(&self, op: &Operation, txs: &[TransferTx]) -> QueryResult<()> {
        for tx in txs.iter() {
            let inserted = diesel::insert_into(transactions::table)
                .values(&NewTx {
                    tx_type: String::from("transfer"),
                    from_account: tx.from as i32,
                    to_account: Some(tx.to as i32),
                    nonce: Some(tx.nonce as i32),
                    amount: tx
                        .amount
                        .as_bigint_and_exponent()
                        .0
                        .to_str_radix(10)
                        .as_str()
                        .parse()
                        .unwrap(),
                    fee: tx
                        .fee
                        .as_bigint_and_exponent()
                        .0
                        .to_str_radix(10)
                        .as_str()
                        .parse()
                        .unwrap(),
                    block_number: Some(op.block.block_number as i32),
                    state_root: Some(op.block.new_root_hash.to_hex()),
                })
                .execute(self.conn())?;
            if 0 == inserted {
                error!("Error: could not commit all new transactions!");
                return Err(Error::RollbackTransaction);
            }
        }
        Ok(())
    }

    fn save_deposit_transactions(&self, op: &Operation, txs: &[DepositTx]) -> QueryResult<()> {
        for tx in txs.iter() {
            let inserted = diesel::insert_into(transactions::table)
                .values(&NewTx {
                    tx_type: String::from("deposit"),
                    from_account: tx.account as i32,
                    to_account: None,
                    nonce: None,
                    amount: tx
                        .amount
                        .as_bigint_and_exponent()
                        .0
                        .to_str_radix(10)
                        .as_str()
                        .parse()
                        .unwrap(),
                    fee: 0,
                    block_number: Some(op.block.block_number as i32),
                    state_root: Some(op.block.new_root_hash.to_hex()),
                })
                .execute(self.conn())?;
            if 0 == inserted {
                error!("Error: could not commit all new transactions!");
                return Err(Error::RollbackTransaction);
            }
        }
        Ok(())
    }

    fn save_exit_transactions(&self, op: &Operation, txs: &[ExitTx]) -> QueryResult<()> {
        for tx in txs.iter() {
            let inserted = diesel::insert_into(transactions::table)
                .values(&NewTx {
                    tx_type: String::from("exit"),
                    from_account: tx.account as i32,
                    to_account: None,
                    nonce: None,
                    amount: tx
                        .amount
                        .as_bigint_and_exponent()
                        .0
                        .to_str_radix(10)
                        .as_str()
                        .parse()
                        .unwrap(),
                    fee: 0,
                    block_number: Some(op.block.block_number as i32),
                    state_root: Some(op.block.new_root_hash.to_hex()),
                })
                .execute(self.conn())?;
            if 0 == inserted {
                error!("Error: could not commit all new transactions!");
                return Err(Error::RollbackTransaction);
            }
        }
        Ok(())
    }

    fn commit_state_update(
        &self,
        block_number: u32,
        accounts_updated: &[AccountUpdate],
    ) -> QueryResult<()> {
        for upd in accounts_updated.iter() {
            unimplemented!();
            debug!(
                "Committing state update for account {} in block {}",
                upd.get_account_id(),
                block_number
            );
            match *upd {
                AccountUpdate::Create {
                    id,
                    public_key_x,
                    public_key_y,
                    nonce,
                } => {
                    diesel::insert_into(account_creates::table)
                        .values(&StorageAccountCreation {
                            account_id: id as i32,
                            is_create: true,
                            block_number: block_number as i32,
                            pk_x: fr_to_bytes(&public_key_x),
                            pk_y: fr_to_bytes(&public_key_y),
                            nonce: nonce as i64,
                        })
                        .execute(self.conn())?;
                }
                AccountUpdate::Delete {
                    id,
                    public_key_x,
                    public_key_y,
                    nonce,
                } => {
                    diesel::insert_into(account_creates::table)
                        .values(&StorageAccountCreation {
                            account_id: id as i32,
                            is_create: false,
                            block_number: block_number as i32,
                            pk_x: fr_to_bytes(&public_key_x),
                            pk_y: fr_to_bytes(&public_key_y),
                            nonce: nonce as i64,
                        })
                        .execute(self.conn())?;
                }
                AccountUpdate::UpdateBalance {
                    id,
                    balance_update: (token, old_balance, new_balance),
                    nonce,
                } => {
                    diesel::insert_into(account_balance_updates::table)
                        .values(&StorageAccountUpdate {
                            account_id: id as i32,
                            block_number: block_number as i32,
                            coin_id: token as i32,
                            old_balance: old_balance.clone(),
                            new_balance: new_balance.clone(),
                            nonce: nonce as i64,
                        })
                        .execute(self.conn())?;
                }
            }
            // TODO: (Drogan) was this check necessary?
            //             let inserted = ;
            //             if 0 == inserted {
            //                 error!("Error: could not commit all state updates!");
            //                 return Err(Error::RollbackTransaction);
            //             }
        }
        Ok(())
    }

    fn apply_state_update(&self, block_number: u32) -> QueryResult<()> {
        self.conn().transaction(|| {
            let account_balance_diff = account_balance_updates::table
                .filter(account_balance_updates::block_number.eq(&(block_number as i32)))
                .load::<StorageAccountUpdate>(self.conn())?;

            let account_creation_diff = account_creates::table
                .filter(account_creates::block_number.eq(&(block_number as i32)))
                .load::<StorageAccountCreation>(self.conn())?;

            let mut account_updates: Vec<StorageAccountDiff> = {
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

            for acc_update in account_updates.into_iter() {
                match acc_update {
                    StorageAccountDiff::BalanceUpdate(upd) => {
                        update(
                            balances::table
                                .filter(balances::coin_id.eq(upd.coin_id))
                                .filter(balances::account_id.eq(upd.account_id)),
                        )
                        .set(balances::balance.eq(upd.new_balance))
                        .execute(self.conn())?;

                        update(accounts::table.filter(accounts::id.eq(upd.account_id)))
                            .set((
                                accounts::last_block.eq(upd.block_number),
                                accounts::nonce.eq(upd.nonce),
                            ))
                            .execute(self.conn())?;
                    }

                    StorageAccountDiff::Create(upd) => {
                        let storage_account = StorageAccount {
                            id: upd.account_id,
                            last_block: upd.block_number,
                            nonce: upd.nonce,
                            pk_x: upd.pk_x,
                            pk_y: upd.pk_y,
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

    pub fn load_committed_state(&self) -> QueryResult<(u32, AccountMap)> {
        unimplemented!();
        //        const SELECT: &str = "
        //        WITH upd AS (
        //            WITH s AS (
        //                SELECT account_id as id, max(block_number) as last_block
        //                FROM account_updates u
        //                WHERE u.block_number > (SELECT COALESCE(max(last_block), 0) FROM accounts)
        //                GROUP BY account_id
        //            )
        //            SELECT u.account_id AS id, u.block_number AS last_block, u.data FROM s, account_updates u WHERE s.id = u.account_id AND u.block_number = s.last_block
        //        )
        //        SELECT COALESCE(u.id, a.id) AS id, COALESCE(u.last_block, a.last_block) AS last_block, COALESCE (u.data, a.data) AS data
        //        FROM upd u
        //        FULL JOIN accounts a ON a.id = u.id
        //        ORDER BY id";
        //
        //        self.load_state(SELECT)
    }

    pub fn load_verified_state(&self) -> QueryResult<(u32, AccountMap)> {
        unimplemented!();
        //        self.load_state("SELECT * FROM accounts a")
    }

    /// loads the state of accounts updated between two blocks
    pub fn load_state_diff(
        &self,
        from_block: u32,
        to_block: u32,
    ) -> QueryResult<Vec<AccountUpdate>> {
        let time_forward = from_block <= to_block;
        let start_block = cmp::min(from_block, to_block);
        let end_block = cmp::max(from_block, to_block);

        let account_balance_diff = account_balance_updates::table
            .filter(account_balance_updates::block_number.ge(&(start_block as i32)))
            .filter(account_balance_updates::block_number.lt(&(end_block as i32)))
            .load::<StorageAccountUpdate>(self.conn())?;

        let account_creation_diff = account_creates::table
            .filter(account_creates::block_number.ge(&(start_block as i32)))
            .filter(account_creates::block_number.lt(&(end_block as i32)))
            .load::<StorageAccountCreation>(self.conn())?;

        let mut account_updates: Vec<AccountUpdate> = {
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
            account_diff.into_iter().map(|d| d.into()).collect()
        };

        if !time_forward {
            account_updates.reverse();
            for acc_upd in account_updates.iter_mut() {
                acc_upd.reverse_update();
            }
        }

        Ok(account_updates)
    }

    /// loads the state of accounts updated in a specific block
    pub fn load_state_diff_for_block(&self, block_number: u32) -> QueryResult<Vec<AccountUpdate>> {
        self.load_state_diff(block_number, block_number + 1)
    }

    fn load_state(&self, query: &str) -> QueryResult<(u32, AccountMap)> {
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

    pub fn last_committed_state_for_account(
        &self,
        account_id: AccountId,
    ) -> QueryResult<Option<models::plasma::account::Account>> {
        let (last_block, account) = if let Some(account) = accounts::table
            .find(account_id as i32)
            .first::<StorageAccount>(self.conn())
            .optional()?
        {
            let balances: Vec<StorageBalance> =
                StorageBalance::belonging_to(&account).load(self.conn())?;

            let last_block = account.last_block;

            let (_, account) = restore_account(account, balances);
            (last_block, Some(account))
        } else {
            (0, None)
        };

        let account_balance_diff: Vec<StorageAccountUpdate> = {
            account_balance_updates::table
                .filter(account_balance_updates::account_id.eq(&(account_id as i32)))
                .filter(account_balance_updates::block_number.gt(&last_block))
                .load::<StorageAccountUpdate>(self.conn())?
        };

        let account_creation_diff: Vec<StorageAccountCreation> = {
            account_creates::table
                .filter(account_creates::account_id.eq(&(account_id as i32)))
                //                .filter( account_creates::block_number.gt( &last_block ) )
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
        };

        Ok(account_diff.into_iter().fold(account, |account, upd| {
            Account::apply_update(account, upd.into())
        }))
    }

    pub fn last_verified_state_for_account(
        &self,
        account_id: AccountId,
    ) -> QueryResult<Option<models::plasma::account::Account>> {
        use crate::schema::accounts::dsl::*;

        Ok(
            if let Some(account) = accounts
                .find(account_id as i32)
                .first::<StorageAccount>(self.conn())
                .optional()?
            {
                let balances: Vec<StorageBalance> =
                    StorageBalance::belonging_to(&account).load(self.conn())?;

                let (_, account) = restore_account(account, balances);
                Some(account)
            } else {
                None
            },
        )
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

    pub fn load_last_saved_transactions(&self, count: i32) -> Vec<StoredTx> {
        let query = format!(
            "
            SELECT * FROM transactions
            ORDER BY block_number DESC
            LIMIT {}
        ",
            count
        );
        diesel::sql_query(query)
            .load(self.conn())
            .unwrap_or_default()
    }

    pub fn load_tx_transactions_for_account(
        &self,
        account_id: AccountId,
        count: i32,
    ) -> Vec<StoredTx> {
        let query = format!(
            "
            SELECT * FROM transactions
            WHERE from_account = {}
            ORDER BY block_number DESC
            LIMIT {}
        ",
            account_id, count
        );
        diesel::sql_query(query)
            .load(self.conn())
            .unwrap_or_default()
    }

    pub fn load_rx_transactions_for_account(
        &self,
        account_id: AccountId,
        count: i32,
    ) -> Vec<StoredTx> {
        let query = format!(
            "
            SELECT * FROM transactions
            WHERE to_account = {}
            ORDER BY block_number DESC
            LIMIT {}
        ",
            account_id, count
        );
        diesel::sql_query(query)
            .load(self.conn())
            .unwrap_or_else(|_| vec![])
    }

    pub fn load_transaction_with_id(&self, tx_id: u32) -> Option<StoredTx> {
        let query = format!(
            "
            SELECT * FROM transactions
            WHERE id = {}
            DESC LIMIT 1
        ",
            tx_id as i32
        );
        diesel::sql_query(query).get_result(self.conn()).ok()
    }

    pub fn load_transactions_in_block(&self, block_number: u32) -> QueryResult<Vec<StoredTx>> {
        let query = format!(
            "
            SELECT * FROM transactions
            WHERE block_number = {}
            ORDER BY block_number
        ",
            block_number as i32
        );
        diesel::sql_query(query).load(self.conn())
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
}

#[cfg(test)]
mod test {

    use super::*;
    use bigdecimal::BigDecimal;
    use bigdecimal::Num;
    use diesel::Connection;
    use ff::Field;
    use models::plasma::account::ETH_TOKEN_ID;
    use web3::types::U256;
    //use diesel::RunQueryDsl;

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

    #[test]
    fn test_store_state() {
        let pool = ConnectionPool::new();
        let conn = pool.access_storage().unwrap();
        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test

        // uncomment below for debugging to generate initial state
        // diesel::sql_query("delete from accounts")
        // .execute(&conn.conn())
        // .expect("must work");
        // diesel::sql_query("delete from account_updates")
        // .execute(&conn.conn())
        // .expect("must work");

        let mut accounts = fnv::FnvHashMap::default();
        let acc = |balance| {
            let mut a = models::plasma::account::Account::default();
            a.set_balance(ETH_TOKEN_ID, &BigDecimal::from(balance));
            a
        };

        // commit initial state update
        accounts.insert(1, acc(1));
        accounts.insert(2, acc(2));
        accounts.insert(3, acc(3));
        conn.commit_state_update(1, &accounts).unwrap();

        let (_, state) = conn.load_verified_state().unwrap();
        assert_eq!(state.len(), 0);

        // committed state must be computed from updates
        let (last_block, state) = conn.load_committed_state().unwrap();
        assert_eq!(last_block, 1);
        assert_eq!(
            state
                .into_iter()
                .collect::<Vec<(u32, models::plasma::account::Account)>>(),
            accounts
                .clone()
                .into_iter()
                .collect::<Vec<(u32, models::plasma::account::Account)>>()
        );

        // now apply commitment
        conn.apply_state_update(1).expect("update must work");

        // verified state must be equal the commitment
        let (_, state) = conn.load_verified_state().unwrap();
        assert_eq!(
            state
                .into_iter()
                .collect::<Vec<(u32, models::plasma::account::Account)>>(),
            accounts
                .clone()
                .into_iter()
                .collect::<Vec<(u32, models::plasma::account::Account)>>()
        );

        let (_, state) = conn.load_state_diff(1, 2).expect("load_state_diff failed");
        assert_eq!(state.get(&2).unwrap(), &acc(2));

        let (_, reverse) = conn.load_state_diff(2, 1).unwrap();
        assert_eq!(reverse.len(), 0);

        // commit second state update
        let mut accounts2 = fnv::FnvHashMap::default();
        accounts2.insert(2, acc(23));
        accounts2.insert(4, acc(4));
        conn.commit_state_update(2, &accounts2).unwrap();

        assert_eq!(conn.load_verified_state().unwrap().1.len(), 3);
        assert_eq!(conn.load_committed_state().unwrap().1.len(), 4);

        let (_, state) = conn.load_state_diff(1, 2).unwrap();
        assert_eq!(state.get(&2).unwrap(), &acc(2));
        let (_, state) = conn.load_state_diff(1, 3).unwrap();
        assert_eq!(state.get(&2).unwrap(), &acc(23));
        let (_, state) = conn.load_state_diff(2, 3).unwrap();
        assert_eq!(state.get(&2).unwrap(), &acc(23));

        let (_, reverse) = conn.load_state_diff(3, 2).unwrap();
        assert_eq!(reverse.get(&2).unwrap(), &acc(2));
    }

    #[test]
    fn test_store_txs() {
        let pool = ConnectionPool::new();
        let conn = pool.access_storage().unwrap();
        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test
        conn.prepare_nonce_scheduling("0x0", 0).unwrap();

        let mut accounts = fnv::FnvHashMap::default();
        let acc = |balance| {
            let mut a = models::plasma::account::Account::default();
            a.set_balance(ETH_TOKEN_ID, &BigDecimal::from(balance));
            a
        };

        accounts.insert(3, acc(1));
        accounts.insert(5, acc(2));
        accounts.insert(7, acc(3));
        accounts.insert(8, acc(4));
        conn.execute_operation(&Operation {
            id: None,
            action: Action::Commit,
            block: Block {
                block_number: 1,
                new_root_hash: Fr::default(),
                block_data: BlockData::Deposit {
                    batch_number: 0,
                    transactions: vec![],
                },
            },
            accounts_updated: Some(accounts.clone()),
            tx_meta: None,
        })
        .unwrap();
        assert_eq!(conn.last_verified_state_for_account(5).unwrap(), None);
        assert_eq!(
            conn.last_committed_state_for_account(5)
                .unwrap()
                .unwrap()
                .get_balance(ETH_TOKEN_ID),
            &BigDecimal::from(2)
        );

        conn.execute_operation(&Operation {
            id: None,
            action: Action::Verify {
                proof: Box::new([U256::zero(); 8]),
            },
            block: Block {
                block_number: 1,
                new_root_hash: Fr::default(),
                block_data: BlockData::Deposit {
                    batch_number: 0,
                    transactions: vec![],
                },
            },
            accounts_updated: Some(accounts.clone()),
            tx_meta: None,
        })
        .unwrap();

        assert_eq!(
            conn.last_verified_state_for_account(7)
                .unwrap()
                .unwrap()
                .get_balance(ETH_TOKEN_ID),
            &BigDecimal::from(3)
        );
        assert_eq!(
            conn.last_committed_state_for_account(7)
                .unwrap()
                .unwrap()
                .get_balance(ETH_TOKEN_ID),
            &BigDecimal::from(3)
        );

        let pending = conn.load_unsent_ops(0).unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].tx_meta.as_ref().unwrap().nonce, 0);
        assert_eq!(pending[1].tx_meta.as_ref().unwrap().nonce, 1);

        let pending = conn.load_unsent_ops(1).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].tx_meta.as_ref().unwrap().nonce, 1);

        let pending = conn.load_unsent_ops(2).unwrap();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_store_proof_reqs() {
        let pool = ConnectionPool::new();
        let conn = pool.access_storage().unwrap();
        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test
        conn.prepare_nonce_scheduling("0x0", 0).unwrap();

        conn.execute_operation(&Operation {
            id: None,
            action: Action::Commit,
            block: Block {
                block_number: 1,
                new_root_hash: Fr::default(),
                block_data: BlockData::Deposit {
                    batch_number: 1,
                    transactions: vec![],
                },
            },
            accounts_updated: Some(fnv::FnvHashMap::default()),
            tx_meta: None,
        })
        .unwrap();

        let pending = conn.load_unverified_commitments().unwrap();
        assert_eq!(pending.len(), 1);

        conn.execute_operation(&Operation {
            id: None,
            action: Action::Verify {
                proof: Box::new([U256::zero(); 8]),
            },
            block: Block {
                block_number: 1,
                new_root_hash: Fr::default(),
                block_data: BlockData::Deposit {
                    batch_number: 1,
                    transactions: vec![],
                },
            },
            accounts_updated: Some(fnv::FnvHashMap::default()),
            tx_meta: None,
        })
        .unwrap();

        let pending = conn.load_unverified_commitments().unwrap();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_store_helpers() {
        let pool = ConnectionPool::new();
        let conn = pool.access_storage().unwrap();
        conn.conn().begin_test_transaction().unwrap(); // this will revert db after test

        assert_eq!(-1, conn.load_last_committed_deposit_batch().unwrap());
        assert_eq!(-1, conn.load_last_committed_exit_batch().unwrap());
        assert_eq!(0, conn.get_last_committed_block().unwrap());
        assert_eq!(0, conn.get_last_verified_block().unwrap());
        assert_eq!(conn.last_committed_state_for_account(9999).unwrap(), None);
        assert_eq!(conn.last_verified_state_for_account(9999).unwrap(), None);

        conn.execute_operation(&Operation {
            id: None,
            action: Action::Commit,
            block: Block {
                block_number: 1,
                new_root_hash: Fr::default(),
                block_data: BlockData::Deposit {
                    batch_number: 3,
                    transactions: vec![],
                },
            },
            accounts_updated: Some(fnv::FnvHashMap::default()),
            tx_meta: None,
        })
        .unwrap();
        assert_eq!(3, conn.load_last_committed_deposit_batch().unwrap());

        conn.execute_operation(&Operation {
            id: None,
            action: Action::Commit,
            block: Block {
                block_number: 1,
                new_root_hash: Fr::default(),
                block_data: BlockData::Exit {
                    batch_number: 2,
                    transactions: vec![],
                },
            },
            accounts_updated: Some(fnv::FnvHashMap::default()),
            tx_meta: None,
        })
        .unwrap();
        assert_eq!(2, conn.load_last_committed_exit_batch().unwrap());
    }

    #[test]
    fn test_store_txs_2() {
        let pool = ConnectionPool::new();
        let conn = pool.access_storage().unwrap();
        conn.conn().begin_test_transaction().unwrap();

        let deposit_tx: DepositTx = DepositTx {
            account: 1,
            amount: BigDecimal::from_str_radix(&format!("{}", 10000), 10).unwrap(),
            pub_x: Fr::zero(),
            pub_y: Fr::zero(),
        };

        let transfer_tx: TransferTx = TransferTx {
            from: 1,
            to: 2,
            amount: BigDecimal::from_str_radix(&format!("{}", 5000), 10).unwrap(),
            fee: BigDecimal::from_str_radix(&format!("{}", 0), 10).unwrap(),
            nonce: 1,
            good_until_block: 100_000,
            signature: TxSignature::default(),
            cached_pub_key: None,
        };

        let exit_tx: ExitTx = ExitTx {
            account: 1,
            amount: BigDecimal::from_str_radix(&format!("{}", 5000), 10).unwrap(),
        };

        conn.execute_operation(&Operation {
            id: None,
            action: Action::Commit,
            block: Block {
                block_number: 1,
                new_root_hash: Fr::default(),
                block_data: BlockData::Deposit {
                    batch_number: 1,
                    transactions: vec![deposit_tx.clone(), deposit_tx.clone()],
                },
            },
            accounts_updated: Some(fnv::FnvHashMap::default()),
            tx_meta: None,
        })
        .unwrap();

        conn.execute_operation(&Operation {
            id: None,
            action: Action::Commit,
            block: Block {
                block_number: 2,
                new_root_hash: Fr::default(),
                block_data: BlockData::Transfer {
                    total_fees: BigDecimal::from_str_radix(&format!("{}", 0), 10).unwrap(),
                    transactions: vec![transfer_tx.clone(), transfer_tx.clone()],
                },
            },
            accounts_updated: Some(fnv::FnvHashMap::default()),
            tx_meta: None,
        })
        .unwrap();

        conn.execute_operation(&Operation {
            id: None,
            action: Action::Commit,
            block: Block {
                block_number: 3,
                new_root_hash: Fr::default(),
                block_data: BlockData::Exit {
                    batch_number: 2,
                    transactions: vec![exit_tx.clone(), exit_tx.clone()],
                },
            },
            accounts_updated: Some(fnv::FnvHashMap::default()),
            tx_meta: None,
        })
        .unwrap();

        let txs = conn.load_last_saved_transactions(10);
        assert_eq!(txs.len(), 6);
    }

    fn dummy_op(action: Action, block_number: BlockNumber) -> Operation {
        Operation {
            id: None,
            action,
            block: Block {
                block_number,
                new_root_hash: Fr::default(),
                block_data: BlockData::Deposit {
                    batch_number: 1,
                    transactions: vec![],
                },
            },
            accounts_updated: Some(fnv::FnvHashMap::default()),
            tx_meta: None,
        }
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

    #[test]
    fn test_fr_binary_serialization() {
        use ff::{PrimeField, PrimeFieldRepr};

        let mut buff: Vec<u8> = Vec::new();
        Fr::one().into_repr().write_be(&mut buff).unwrap();
        println!("{:?}", buff);
        assert_eq!(32, buff.len());

        let mut buff: Vec<u8> = Vec::new();
        Fr::one().into_repr().write_le(&mut buff).unwrap();
        println!("{:?}", buff);
        assert_eq!(32, buff.len());

        let mut buff: Vec<u8> = Vec::new();
        Fr::zero().into_repr().write_be(&mut buff).unwrap();
        println!("{:?}", buff);
        assert_eq!(32, buff.len());

        let mut buff: Vec<u8> = Vec::new();
        Fr::zero().into_repr().write_le(&mut buff).unwrap();
        println!("{:?}", buff);
        assert_eq!(32, buff.len());
    }
}
