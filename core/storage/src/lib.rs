//! Storage crate provides the interfaces to interact with the database.
//! The backend database used in this crate is `Postgres`, and interaction
//! with it is based on the `diesel` crate.
//!
//! The essential structure of this crate is the `StorageProcessor`, which
//! holds down the connection to the database and provides abstract interfaces
//! to modify it (called `Schema`s).
//!
//! # Crate Architecture Overview
//!
//! This crate can be divided into three logical parts:
//! - Connection utilities. Tools to establish connections to the database,
//!   stored in the `connection` module.
//! - `Schema`s. Schema is a logically extracted access to the part of
//!   the database, e.g. `ethereum` (which contains methods to store the
//!   information about interaction with the Ethereum blockchain).
//! - `StorageProcessor`. A structure that connects the two points above
//!   into one user-friendly interface.
//!
//! Most of schema modules contain at least two files:
//! - `mod.rs`, which contains the schema itself.
//! - `records.rs`, which contains the representation of the associated database
//!   tables as structures.
//!
//! The latter ones usually don't contain any logic other than the structures
//! declarations, and all the logic is contained in either schema (for most
//! modules), or in an additional helper module (e.g. in the `chain/block` module).//!
//!
//! # Testing Approach
//!
//! Tests for the storage use the actual empty Postgres database.
//! Because of that, these tests are disabled by default, to run them you must use
//! `zksync db-test` (or `zksync db-test-no-reset`, if this is not a first run)
//! command, which will setup the database and enable the tests by passing a feature flag.
//!
//! Tests are implemented in a form of "test transactions", which are database transactions
//! that will never be committed. Thus it is not required to clear the database after running
//! tests. Also the database used for tests is different than the database used for `server`,
//! thus one should not fear to overwrite any important data by running the tests.

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
    /// Creates a `StorageProcessor` using an unique sole connection to the database.
    pub fn establish_connection() -> ConnectionResult<Self> {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let connection = RecoverableConnection::establish(&database_url)?; //.expect(&format!("Error connecting to {}", database_url));
        Ok(Self {
            conn: ConnectionHolder::Direct(connection),
        })
    }

    /// Creates a `StorageProcessor` using a pool of connections.
    /// This method borrows one of the connections from the pool, and releases it
    /// after `drop`.
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
