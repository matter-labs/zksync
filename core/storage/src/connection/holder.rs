// Built-in deps
use std::fmt;
// External imports
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, PooledConnection};
// Workspace imports
// Local imports
use crate::connection::recoverable_connection::RecoverableConnection;

pub enum ConnectionHolder {
    Pooled(PooledConnection<ConnectionManager<RecoverableConnection<PgConnection>>>),
    Direct(RecoverableConnection<PgConnection>),
}

impl fmt::Debug for ConnectionHolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pooled(_) => write!(f, "Pooled connection"),
            Self::Direct(_) => write!(f, "Direct connection"),
        }
    }
}
