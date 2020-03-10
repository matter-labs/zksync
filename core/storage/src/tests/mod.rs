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

/// Makes all the changes performed in database
/// temporary, so they will be reverted right after test.
pub fn prepare_db_for_test<Conn: Connection>(conn: &Conn) {
    conn.begin_test_transaction().unwrap();
}

/// Creates a fixed-seed RNG for tests.
pub fn create_rng() -> XorShiftRng {
    XorShiftRng::from_seed([0, 1, 2, 3])
}
