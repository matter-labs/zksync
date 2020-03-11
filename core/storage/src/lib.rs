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
use crate::connection::{holder::ConnectionHolder, recoverable_connection::RecoverableConnection};

mod schema;
#[cfg(test)]
mod tests;

pub mod chain;
pub mod config;
pub mod connection;
pub mod data_restore;
pub mod diff;
pub mod ethereum;
pub mod prover;
pub mod tokens;

pub use crate::connection::ConnectionPool;

/// Storage processor is the main storage interaction point.
/// It holds down the connection (either direct or pooled) to the database
/// and provide methods to obtain different storage schemas.
#[derive(Debug)]
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

    /// Gains access to the `Chain` schemas.
    pub fn chain<'a>(&'a self) -> chain::ChainIntermediator<'a> {
        chain::ChainIntermediator(self)
    }

    /// Gains access to the `Config` schema.
    pub fn config_schema<'a>(&'a self) -> config::ConfigSchema<'a> {
        config::ConfigSchema(self)
    }

    /// Gains access to the `DataRestore` schema.
    pub fn data_restore_schema<'a>(&'a self) -> data_restore::DataRestoreSchema<'a> {
        data_restore::DataRestoreSchema(self)
    }

    /// Gains access to the `Ethereum` schema.
    pub fn ethereum_schema<'a>(&'a self) -> ethereum::EthereumSchema<'a> {
        ethereum::EthereumSchema(self)
    }

    /// Gains access to the `Prover` schema.
    pub fn prover_schema<'a>(&'a self) -> prover::ProverSchema<'a> {
        prover::ProverSchema(self)
    }

    /// Gains access to the `Tokens` schema.
    pub fn tokens_schema<'a>(&'a self) -> tokens::TokensSchema<'a> {
        tokens::TokensSchema(self)
    }

    fn conn(&self) -> &RecoverableConnection<PgConnection> {
        match self.conn {
            ConnectionHolder::Pooled(ref conn) => conn,
            ConnectionHolder::Direct(ref conn) => conn,
        }
    }
}
