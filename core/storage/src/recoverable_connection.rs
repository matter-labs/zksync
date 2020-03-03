// Built-in deps
use std::cell::RefCell;
use std::time::Duration;
// External uses
use diesel::backend::UsesAnsiSavepointSyntax;
use diesel::connection::{AnsiTransactionManager, Connection, SimpleConnection};
use diesel::prelude::*;
use diesel::query_builder::{AsQuery, QueryFragment, QueryId};
use diesel::query_source::QueryableByName;
use diesel::result::Error as DieselError;
use diesel::types::HasSqlType;

const RETRIES_AMOUNT: usize = 10;
const RETRY_QUANTILE: Duration = Duration::from_millis(200);

/// `RecoverableConnection` is a generic wrapper over Diesel's connection types
/// which is capable of reestablishment of the connection and retrying the same
/// query multiple times in case of database being unavailable for short periods
/// of time.
///
/// The design goals of this wrappers are speed and robustness: since most of the times
/// database will be available, the overhead introduces by this wrapper should not be
/// big. On the other hand, if the database is unavailable, we only care about restoring
/// the connection, and thus the speed is not a highest priority.
///
/// In an actual implementation, this means that in normal conditions the only overhead
/// we have to deal with is introduced by using `RefCell` (and that's not much).
///
/// If the connection breaks, we will try to establish a new one (because Diesel can't
/// restore the existing connection: once it's broken, it's broken) in a loop with an
/// increasing intervals between attempts.
pub struct RecoverableConnection<Conn: Connection> {
    database_url: String,
    connection: RefCell<Conn>,
    transaction_manager: AnsiTransactionManager,
}

impl<Conn: Connection> SimpleConnection for RecoverableConnection<Conn> {
    fn batch_execute(&self, query: &str) -> QueryResult<()> {
        self.exec_with_retries(|| self.connection.borrow().batch_execute(query))
    }
}

impl<Conn> Connection for RecoverableConnection<Conn>
where
    Conn: Connection,
    Conn::Backend: UsesAnsiSavepointSyntax,
{
    type Backend = Conn::Backend;
    type TransactionManager = AnsiTransactionManager;

    fn establish(database_url: &str) -> ConnectionResult<Self> {
        Conn::establish(database_url).map(|connection| Self {
            database_url: database_url.into(),
            connection: RefCell::new(connection),
            transaction_manager: AnsiTransactionManager::new(),
        })
    }

    fn execute(&self, query: &str) -> QueryResult<usize> {
        self.exec_with_retries(|| self.connection.borrow().execute(query))
    }

    fn query_by_index<T, U>(&self, source: T) -> QueryResult<Vec<U>>
    where
        T: AsQuery,
        T::Query: QueryFragment<Self::Backend> + QueryId,
        Self::Backend: HasSqlType<T::SqlType>,
        U: Queryable<T::SqlType, Self::Backend>,
    {
        let query = source.as_query();
        self.exec_with_retries(|| self.connection.borrow().query_by_index(&query))
    }

    fn query_by_name<T, U>(&self, source: &T) -> QueryResult<Vec<U>>
    where
        T: QueryFragment<Self::Backend> + QueryId,
        U: QueryableByName<Self::Backend>,
    {
        self.exec_with_retries(|| self.connection.borrow().query_by_name(source))
    }

    fn execute_returning_count<T>(&self, source: &T) -> QueryResult<usize>
    where
        T: QueryFragment<Self::Backend> + QueryId,
    {
        self.exec_with_retries(|| self.connection.borrow().execute_returning_count(source))
    }

    fn transaction_manager(&self) -> &Self::TransactionManager {
        &self.transaction_manager
    }
}

impl<Conn> RecoverableConnection<Conn>
where
    Conn: Connection,
{
    /// Performs the query (represented as a closure) with a prior knowledge
    /// that the database can sometimes stop responding for short periods of time.
    ///
    /// In case of the database unavailability, the same request is repeated with
    /// increasing time intervals.
    fn exec_with_retries<F, T>(&self, f: F) -> QueryResult<T>
    where
        F: Fn() -> QueryResult<T>,
    {
        for attempt in 0..RETRIES_AMOUNT {
            match f() {
                Ok(result) => {
                    return Ok(result);
                }
                Err(error) if self.is_db_connection_error(&error) => {
                    log::warn!("Error connecting database ({:?}), retrying", error);
                    std::thread::sleep(scale_retry_period(attempt));
                    if let Ok(conn) = Conn::establish(self.database_url.as_ref()) {
                        *self.connection.borrow_mut() = conn;
                    }
                }
                Err(other) => {
                    // Not a connection issue, so it's none of our business.
                    // Just propagate it.
                    return Err(other);
                }
            }
        }

        // At this point we are sure that database is down, we cannot work without a database.
        panic!("Cannot connect to the database after several retries, it is probably down");
    }

    /// Checks whether occurred error is the database connection issue.
    fn is_db_connection_error(&self, error: &DieselError) -> bool {
        if let DieselError::DatabaseError(_kind, info) = error {
            let msg = info.message();

            // We have to compare the string message representation, because `Diesel` doesn't have
            // clear error kinds associated with these errors.
            msg.starts_with("server closed the connection unexpectedly")
                || msg.starts_with("no connection to the server")
        } else {
            false
        }
    }
}

// Scales the retry interval, so that we will have smaller retry intervals in the beginning
// (hoping that the connection will be restored almost immediately), but then we will wait longer
// not to spam the (hopefully) initializing database with many requests.
fn scale_retry_period(n_attempt: usize) -> Duration {
    RETRY_QUANTILE * (n_attempt + 1) as u32
}
