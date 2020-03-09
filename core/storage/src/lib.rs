//! # Representation of the sidechain state in the DB:
//!
//! Saving state is done in two steps
//! 1) When we commit block we save all state updates (tables: `account_creates`, `account_balance_updates`)
//! 2) When we verify block we apply this updates to stored state snapshot (tables: `accounts`, `balances`)
//!
//! This way we have the following advantages:
//! 1) Easy access to state for any block (useful for provers which work on different blocks)
//! 2) We can rewind any `committed` state (which is not final)

#[macro_use]
extern crate diesel;

// Built-in deps
use std::env;
// External imports
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
// Workspace imports
// Local imports
use crate::recoverable_connection::RecoverableConnection;

mod recoverable_connection;
mod schema;
#[cfg(test)]
mod tests;

pub mod connection_pool;
pub mod diff;
pub mod interfaces;

// TODO re-exports to deal with
pub use crate::connection_pool::ConnectionPool;
pub use crate::diff::StorageAccountDiff;

enum ConnectionHolder {
    Pooled(PooledConnection<ConnectionManager<RecoverableConnection<PgConnection>>>),
    Direct(RecoverableConnection<PgConnection>),
}

pub struct StorageProcessor {
    conn: ConnectionHolder,
}

impl StorageProcessor {
    pub fn establish_connection() -> ConnectionResult<Self> {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let connection = RecoverableConnection::establish(&database_url)?; //.expect(&format!("Error connecting to {}", database_url));
        Ok(Self {
            conn: ConnectionHolder::Direct(connection),
        })
    }

    pub fn from_pool(
        conn: PooledConnection<ConnectionManager<RecoverableConnection<PgConnection>>>,
    ) -> Self {
        Self {
            conn: ConnectionHolder::Pooled(conn),
        }
    }

    fn conn(&self) -> &RecoverableConnection<PgConnection> {
        match self.conn {
            ConnectionHolder::Pooled(ref conn) => conn,
            ConnectionHolder::Direct(ref conn) => conn,
        }
    }
}
