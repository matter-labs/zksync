//! Tests for storage crate.
//!
//! These tests require an empty DB setup and ignored by default.
//! To run them, use `zksync db-test-no-reset`/`franklin db-test` script
//! (or, if it's a first run, then `zksync db-test`, which will create all the required
//! test tables). Also be sure to have Postgres running locally.
//!
//! All the tests in this module do roughly follow the same pattern, e.g.:
//!
//! ```ignore
//! #[test]
//! #[cfg_attr(not(feature = "db_test"), ignore)]
//! fn some_test() {
//!     let conn = StorageProcessor::establish_connection().unwrap();
//!     db_test(conn.conn(), || {
//!         // Actual test code.
//!         Ok(())
//!     });
//! }
//! ```
//!
//! Executing the test in `db_test` function as a closure has 2 reasons:
//! 1. All the changes made there will be rolled back and won't affect other tests.
//! 2. Since closure should return a `QueryResult`, it is possible to use `?` in tests
//!    instead of `expect`/`unwrap` after each database interaction.
//!
//! The file hierarchy is designed to mirror the actual project structure.

// External imports
use zksync_crypto::rand::{SeedableRng, XorShiftRng};
// use diesel::Connection;

mod chain;
mod config;
mod data_restore;
mod ethereum;
mod prover;
mod tokens;

pub use db_test_macro::test as db_test;

// /// Runs the database test content within the test transaction, which provides an isolation
// /// for several tests running at the same time.
// #[cfg(feature = "db_test")]
// pub fn db_test<Conn, F, T>(conn: storage, f: F)
// where
//     Conn: Connection,
//     F: FnOnce() -> diesel::QueryResult<T>,
// {
//     // It seems that `test_transaction` not completely isolate the performed changes,
//     // since assigned ID can change between launches. Thus it is not recommended to compare
//     // against the object database ID in tests.
//     conn.test_transaction::<_, diesel::result::Error, _>(|| {
//         // We have to introduce an additional closure,
//         // since `test_transaction` panics upon encountering an error without
//         // displaying the occurred error.
//         f().expect("Test body returned an error:");
//         Ok(())
//     });
// }

// /// Without `db_test` attribute we don't want to run any tests, so we skip them.
// #[cfg(not(feature = "db_test"))]
// pub fn db_test<Conn, F, T>(_conn: storage, _f: F)
// where
//     Conn: Connection,
//     F: FnOnce() -> diesel::QueryResult<T>,
// {
//     // Do nothing
// }

/// Creates a fixed-seed RNG for tests.
pub fn create_rng() -> XorShiftRng {
    XorShiftRng::from_seed([0, 1, 2, 3])
}
