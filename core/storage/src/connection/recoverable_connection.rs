// Built-in deps
use std::cell::{Cell, RefCell};
use std::time::Duration;
// External uses
use diesel::backend::UsesAnsiSavepointSyntax;
use diesel::connection::{AnsiTransactionManager, Connection, SimpleConnection};
use diesel::prelude::*;
use diesel::query_builder::{AsQuery, QueryFragment, QueryId};
use diesel::query_source::QueryableByName;
use diesel::types::HasSqlType;

const RETRIES_AMOUNT: usize = 10;
const RETRY_QUANTILE: Duration = Duration::from_millis(200);

/// `RecoverableConnection` is a generic wrapper over Diesel's connection types
/// which is capable of reestablishment of the connection and retrying the same
/// query multiple times in case of database being unavailable for short periods
/// of time.
///
/// # Retrying Functionality
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
///
/// # Enabling or Disabling
///
/// The retrying functionality can be either turned on or off, so the existing connection
/// can modify its behavior while being used as a part of the connections pool.
///
/// More than that, retrying functionality is disabled *by default*, since the Diesel
/// connection pool can re-create connections on its own, and we don't want the connection
/// to retry the operation unless it was explicitly enabled. Instead, the `ConnectionPool`
/// structure manages this setting upon every storage access request.
pub struct RecoverableConnection<Conn: Connection> {
    database_url: String,
    connection: RefCell<Conn>,
    transaction_manager: AnsiTransactionManager,
    retrying_enabled: Cell<bool>,
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
            retrying_enabled: Cell::new(false),
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
        // If retrying is not enabled, we simply execute the task once.
        if !self.retrying_enabled.get() {
            return f();
        }

        for attempt in 1..=RETRIES_AMOUNT {
            match f() {
                Ok(result) => {
                    return Ok(result);
                }
                Err(error) => {
                    // In case of error encountered, we're gonna retry the connection, since
                    // most likely the error is caused by some issue with the database server.
                    log::warn!(
                        "Error while interacting with database ({}), retry attempt #{}",
                        error,
                        attempt
                    );

                    std::thread::sleep(scale_retry_period(attempt));
                    if let Ok(conn) = Conn::establish(self.database_url.as_ref()) {
                        log::info!(
                            "Connection with the database reestablished after {} retries",
                            attempt
                        );
                        *self.connection.borrow_mut() = conn;
                    }
                }
            }
        }

        // At this point we are sure that database is down, we cannot work without a database.
        panic!("Cannot connect to the database after several retries, it is probably down");
    }

    /// Disables the retrying functionality (effectively making the connection
    /// the equivalent of the underlying connection).
    pub fn disable_retrying(&self) {
        self.retrying_enabled.set(false);
    }

    /// Enables the retrying functionality.
    pub fn enable_retrying(&self) {
        self.retrying_enabled.set(true);
    }
}

// Scales the retry interval, so that we will have smaller retry intervals in the beginning
// (hoping that the connection will be restored almost immediately), but then we will wait longer
// not to spam the (hopefully) initializing database with many requests.
fn scale_retry_period(n_attempt: usize) -> Duration {
    RETRY_QUANTILE * n_attempt as u32
}
