/// Std imports
use std::time::Duration;
// External imports
// Workspace imports
use models::{node::block::PendingBlock, Action};
// Local imports
use crate::tests::{chain::utils::get_operation, db_test};
use crate::{chain::block::BlockSchema, prover::ProverSchema, StorageProcessor};
use models::config_options::ConfigurationOptions;
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
        let loaded = ProverSchema(&conn).load_proof(1)?;
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
        let prover_id = ProverSchema(&conn).register_prover(prover_name, block_size)?;

        // Check that prover is added to the database.
        let prover = ProverSchema(&conn).prover_by_id(prover_id)?;

        assert_eq!(prover.id, prover_id);
        assert_eq!(prover.worker, prover_name);
        assert_eq!(prover.block_size, block_size as i64);
        assert_eq!(prover.stopped_at, None);

        // Stop the prover.
        ProverSchema(&conn).record_prover_stop(prover_id)?;

        // Check that it has been marked as stopped.
        let prover = ProverSchema(&conn).prover_by_id(prover_id)?;
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
        let block_size = ConfigurationOptions::from_env().available_block_chunk_sizes[0]; //smallest block size
        let _prover_id = ProverSchema(&conn).register_prover(prover_name, block_size)?;

        // Create a block.
        BlockSchema(&conn).execute_operation(get_operation(
            1,
            Action::Commit,
            Vec::new(),
            block_size,
        ))?;

        // Get a prover run.
        let maybe_run = ProverSchema(&conn).prover_run_for_next_commit(
            prover_name,
            Duration::from_secs(1),
            block_size,
        )?;
        let run = maybe_run.expect("Can't get a prover run with a block committed");

        assert_eq!(run.block_number, 1);
        assert_eq!(run.worker, Some(prover_name.into()));
        // Initially creation and update time should be equal.
        assert_eq!(run.created_at, run.updated_at);

        // Try to get another run.
        let maybe_run = ProverSchema(&conn).prover_run_for_next_commit(
            prover_name,
            Duration::from_secs(1),
            block_size,
        )?;
        assert!(
            maybe_run.is_none(),
            "There should be no run when one is already created"
        );

        // Create & store proof for the first block.
        let proof = EncodedProofPlonk::default();
        assert!(ProverSchema(&conn).store_proof(1, &proof).is_ok());

        // Try to get another run. There should be none, since there are no blocks to prover.
        let maybe_run = ProverSchema(&conn).prover_run_for_next_commit(
            prover_name,
            Duration::from_secs(1),
            block_size,
        )?;
        assert!(
            maybe_run.is_none(),
            "There should be no run when the only block is proved"
        );

        // Create one more block.
        BlockSchema(&conn).execute_operation(get_operation(
            2,
            Action::Commit,
            Vec::new(),
            block_size,
        ))?;

        // Now we should get a prover run for the second block.
        let maybe_run = ProverSchema(&conn).prover_run_for_next_commit(
            prover_name,
            Duration::from_secs(1),
            block_size,
        )?;
        let run = maybe_run.expect("Can't get a prover run with a block committed");

        assert_eq!(run.block_number, 2);
        assert_eq!(run.worker, Some(prover_name.into()));
        assert_eq!(run.created_at, run.updated_at);

        Ok(())
    });
}

/// Checks that `unstarted_jobs_count` method of schema returns the amount
/// of blocks for which proof is not generating (or generated) yet.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn unstarted_prover_jobs_count() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Add the prover.
        let prover_name = "prover_10";
        let block_size = ConfigurationOptions::from_env().available_block_chunk_sizes[0]; //smallest block size
        let _prover_id = ProverSchema(&conn).register_prover(prover_name, block_size)?;

        // Initially there are no blocks to prove.
        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 0);

        // Create a some blocks.
        BlockSchema(&conn).execute_operation(get_operation(
            1,
            Action::Commit,
            Vec::new(),
            block_size,
        ))?;
        BlockSchema(&conn).execute_operation(get_operation(
            2,
            Action::Commit,
            Vec::new(),
            block_size,
        ))?;
        BlockSchema(&conn).execute_operation(get_operation(
            3,
            Action::Commit,
            Vec::new(),
            block_size,
        ))?;

        // We've created 3 blocks and no jobs were assigned yet.
        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 3);

        // Create a prover run.
        ProverSchema(&conn).prover_run_for_next_commit(
            prover_name,
            Duration::from_secs(1),
            block_size,
        )?;

        // Now, as the job started, the number of not started jobs must be 2.
        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 2);

        // Create & store proof for the first block.
        let proof = EncodedProofPlonk::default();
        assert!(ProverSchema(&conn).store_proof(1, &proof).is_ok());

        // After saving the block there still should be 2 not started jobs.
        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 2);

        // Create next run & repeat checks.
        ProverSchema(&conn).prover_run_for_next_commit(
            prover_name,
            Duration::from_secs(2),
            block_size,
        )?;

        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 1);
        let proof = EncodedProofPlonk::default();
        assert!(ProverSchema(&conn).store_proof(2, &proof).is_ok());
        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 1);

        // And finally store the proof for the third block.
        ProverSchema(&conn).prover_run_for_next_commit(
            prover_name,
            Duration::from_secs(3),
            block_size,
        )?;

        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 0);
        let proof = EncodedProofPlonk::default();
        assert!(ProverSchema(&conn).store_proof(3, &proof).is_ok());
        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 0);

        // Then, when all the blocks are verified, create on more commit and check
        // that amount is increased again.
        BlockSchema(&conn).execute_operation(get_operation(
            4,
            Action::Commit,
            Vec::new(),
            block_size,
        ))?;
        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 1);

        // Add pending block. Amount of blocks should increase.

        BlockSchema(&conn).save_pending_block(PendingBlock {
            number: 5,
            chunks_left: 0,
            unprocessed_priority_op_before: 0,
            pending_block_iteration: 1,
            success_operations: vec![],
            block_timestamp: 0u64.into(),
        })?;
        let blocks_count = ProverSchema(&conn).unstarted_jobs_count()?;
        assert_eq!(blocks_count, 2);

        Ok(())
    });
}
