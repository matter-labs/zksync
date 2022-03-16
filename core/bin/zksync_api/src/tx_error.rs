use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Error)]
pub enum Toggle2FAError {
    #[error("Internal error")]
    Other,

    #[error("Database unavailable")]
    DbError,

    #[error("Can not change 2FA for a CREATE2 account")]
    CREATE2,

    #[error("Request to enable 2FA should not have PubKeyHash field set")]
    UnusedPubKeyHash,
}
