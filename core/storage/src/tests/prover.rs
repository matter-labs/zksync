/// Std imports
use std::time::Duration;
// External imports
// Workspace imports
use models::Action;
// Local imports
use crate::tests::{chain::utils::get_operation, db_test};
use crate::{chain::block::BlockSchema, prover::ProverSchema, StorageProcessor};
use models::params::block_chunk_sizes;
use models::prover_utils::EncodedProofPlonk;

/// Checks that the proof can be stored and loaded.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn test_store_proof() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Attempt to load the proof that was not stored should result in an error.
        assert!(ProverSchema(&conn).load_proof(1).is_err());

        // Store the proof.
        let proof = EncodedProofPlonk::default();
        assert!(ProverSchema(&conn).store_proof(1, &proof).is_ok());

        // Now load it.
        let loaded = ProverSchema(&conn)
            .load_proof(1)
            .expect("Cannot load the stored proof");
        assert_eq!(loaded, proof);

        Ok(())
    });
}

/// Checks the prover registration workflow, including
/// adding a new prover, stopping and resuming it.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn prover_registration() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Attempt to load the non-existent prover should result in an error.
        assert!(ProverSchema(&conn).prover_by_id(1).is_err());

        // Add the prover.
        let prover_name = "prover_10";
        let block_size = 10;
        let prover_id = ProverSchema(&conn)
            .register_prover(prover_name, block_size)
            .expect("Can't add a prover");

        // Check that prover is added to the database.
        let prover = ProverSchema(&conn)
            .prover_by_id(prover_id)
            .expect("Can't obtain the stored prover");

        assert_eq!(prover.id, prover_id);
        assert_eq!(prover.worker, prover_name);
        assert_eq!(prover.block_size, block_size as i64);
        assert_eq!(prover.stopped_at, None);

        // Stop the prover.
        ProverSchema(&conn)
            .record_prover_stop(prover_id)
            .expect("Can't stop a prover");

        // Check that it has been marked as stopped.
        let prover = ProverSchema(&conn)
            .prover_by_id(prover_id)
            .expect("Can't obtain the stored prover");
        assert!(prover.stopped_at.is_some());

        Ok(())
    });
}

/// Checks the workflow of registering a prover run.
/// - Register a prover.
/// - Create a block that is committed and not verified.
/// - Obtain a prover run for that block.
/// - Check that we won't create another prover run for that block.
/// - Store a proof.
/// - Check that we won't create another prover run for that block once it's verified.
/// - Create a new block & obtain a prover run for it.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn prover_run() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Add the prover.
        let prover_name = "prover_10";
        let block_size = block_chunk_sizes()[0]; //smallest block size
        let _prover_id = ProverSchema(&conn)
            .register_prover(prover_name, block_size)
            .expect("Can't add a prover");

        // Create a block.
        BlockSchema(&conn)
            .execute_operation(get_operation(1, Action::Commit, Vec::new()))
            .expect("Commit block 1");

        // Get a prover run.
        let maybe_run = ProverSchema(&conn)
            .prover_run_for_next_commit(prover_name, Duration::from_secs(1), block_size)
            .expect("Prover run query failed");
        let run = maybe_run.expect("Can't get a prover run with a block committed");

        assert_eq!(run.block_number, 1);
        assert_eq!(run.worker, Some(prover_name.into()));
        // Initially creation and update time should be equal.
        assert_eq!(run.created_at, run.updated_at);

        // Try to get another run.
        let maybe_run = ProverSchema(&conn)
            .prover_run_for_next_commit(prover_name, Duration::from_secs(1), block_size)
            .expect("Prover run query failed");
        assert!(
            maybe_run.is_none(),
            "There should be no run when one is already created"
        );

        // Create & store proof for the first block.
        let proof = EncodedProofPlonk::default();
        assert!(ProverSchema(&conn).store_proof(1, &proof).is_ok());

        // Try to get another run. There should be none, since there are no blocks to prover.
        let maybe_run = ProverSchema(&conn)
            .prover_run_for_next_commit(prover_name, Duration::from_secs(1), block_size)
            .expect("Prover run query failed");
        assert!(
            maybe_run.is_none(),
            "There should be no run when the only block is proved"
        );

        // Create one more block.
        BlockSchema(&conn)
            .execute_operation(get_operation(2, Action::Commit, Vec::new()))
            .expect("Commit block 2");

        // Now we should get a prover run for the second block.
        let maybe_run = ProverSchema(&conn)
            .prover_run_for_next_commit(prover_name, Duration::from_secs(1), block_size)
            .expect("Prover run query failed");
        let run = maybe_run.expect("Can't get a prover run with a block committed");

        assert_eq!(run.block_number, 2);
        assert_eq!(run.worker, Some(prover_name.into()));
        assert_eq!(run.created_at, run.updated_at);

        Ok(())
    });
}
