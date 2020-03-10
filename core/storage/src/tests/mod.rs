// These tests require empty DB setup and ignored by default
// use `zksync db-test-no-reset`/`franklin db-test` script to run them

// External imports
use crypto_exports::rand::{SeedableRng, XorShiftRng};
use diesel::Connection;

mod chain;
mod config;
mod data_restore;
mod ethereum;
mod prover;
mod tokens;

/// Runs the database test content within the test transaction, which provides an isolation
/// for several tests running at the same time.
#[cfg(feature = "db_test")]
pub fn db_test<Conn, F, T>(conn: &Conn, f: F)
where
    Conn: Connection,
    F: FnOnce() -> diesel::QueryResult<T>,
{
    conn.test_transaction(f);
}

/// Without `db_test` attribute we don't want to run any tests, so we skip them.
#[cfg(not(feature = "db_test"))]
pub fn db_test<Conn, F, T>(_conn: &Conn, _f: F)
where
    Conn: Connection,
    F: FnOnce() -> diesel::QueryResult<T>,
{
    // Do nothing
}

/// Creates a fixed-seed RNG for tests.
pub fn create_rng() -> XorShiftRng {
    XorShiftRng::from_seed([0, 1, 2, 3])
}
