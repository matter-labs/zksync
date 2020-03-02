use std::cell::RefCell;

use diesel::backend::UsesAnsiSavepointSyntax;
use diesel::connection::{AnsiTransactionManager, Connection, SimpleConnection};
use diesel::prelude::*;
use diesel::query_builder::{AsQuery, QueryFragment, QueryId};
use diesel::query_source::QueryableByName;
use diesel::types::HasSqlType;

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
        // Note: Since source is consumed by value and trait does not require `Clone`,
        // we cannot retry the query here.
        self.connection.borrow().query_by_index(source)
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
    fn exec_with_retries<F, T>(&self, f: F) -> QueryResult<T>
    where
        F: Fn() -> QueryResult<T>,
    {
        for _ in 0..10 {
            match f() {
                Ok(result) => {
                    log::info!("Successfully performed query");
                    return Ok(result);
                }
                Err(error) => {
                    log::warn!("Error connecting database ({}), retrying", error);
                    std::thread::sleep(std::time::Duration::from_millis(5000));
                    match Conn::establish(self.database_url.as_ref()) {
                        Ok(conn) => {
                            *self.connection.borrow_mut() = conn;
                        }
                        Err(_) => {
                            // TODO should we react?
                        }
                    }
                }
            }
        }

        panic!("Cannot connect to the database after several retries, it is probably down");
    }
}
