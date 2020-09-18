// Built-in deps
use std::fmt;
// External imports
// use diesel::r2d2::{ConnectionManager, PooledConnection};
use sqlx::{pool::PoolConnection, postgres::Postgres, PgConnection, Transaction};
// Workspace imports
// Local imports

/// Connection holder unifies the type of underlying connection, which
/// can be either pooled or direct.
pub enum ConnectionHolder<'a> {
    Pooled(PoolConnection<Postgres>),
    Direct(PgConnection),
    Transaction(Transaction<'a, Postgres>),
}

impl<'a> fmt::Debug for ConnectionHolder<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pooled(_) => write!(f, "Pooled connection"),
            Self::Direct(_) => write!(f, "Direct connection"),
            Self::Transaction(_) => write!(f, "Database Transaction"),
        }
    }
}
