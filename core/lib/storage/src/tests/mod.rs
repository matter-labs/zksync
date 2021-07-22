//! Tests for storage crate.
//!
//! These tests require an empty DB setup and ignored by default.
//! To run them, use `zk test db` command. Be sure to have Postgres running locally.
//!
//! All the tests in this module do roughly follow the same pattern, e.g.:
//!
//! ```ignore
//! #[db_test]
//! async fn test_something(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
//!     // Actual code that uses `storage`.
//! }
//! ```
//!
//! This procedure macro implicitly creates a database connection and starts
//! new transactions which will be dropped when the method returns.
//!
//! The file hierarchy is designed to mirror the actual project structure.

// External imports
use once_cell::sync::Lazy;
use tokio::sync::Mutex;
// Workspace imports
use zksync_crypto::rand::{SeedableRng, XorShiftRng};

pub(crate) mod chain;
mod config;
mod data_restore;
mod ethereum;
mod event;
mod forced_exit_requests;
mod prover;
mod tokens;

pub use db_test_macro::test as db_test;

/// Creates a fixed-seed RNG for tests.
pub fn create_rng() -> XorShiftRng {
    XorShiftRng::from_seed([0, 1, 2, 3])
}

/// Mutex that's used to avoid database deadlock when accessing
/// accounts state concurrently in tests.
static ACCOUNT_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
