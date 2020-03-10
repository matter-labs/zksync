// External imports
use diesel::Connection;
// Workspace imports
use models::EncodedProof;
// Local imports
use crate::{prover::ProverSchema, ConnectionPool};

#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn test_store_proof() {
    let pool = ConnectionPool::new();
    let conn = pool.access_storage().unwrap();
    conn.conn().begin_test_transaction().unwrap(); // this will revert db after test

    assert!(ProverSchema(&conn).load_proof(1).is_err());

    let proof = EncodedProof::default();
    assert!(ProverSchema(&conn).store_proof(1, &proof).is_ok());

    let loaded = ProverSchema(&conn).load_proof(1).expect("must load proof");
    assert_eq!(loaded, proof);
}
