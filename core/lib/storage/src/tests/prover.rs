/// Std imports
use std::time::Duration;
// External imports
// Workspace imports
use zksync_types::{block::PendingBlock, Action};
// Local imports
use crate::tests::{chain::utils::get_operation, db_test};
use crate::{chain::block::BlockSchema, prover::ProverSchema, QueryResult, StorageProcessor};
use zksync_config::ConfigurationOptions;
use zksync_crypto::proof::EncodedProofPlonk;

/// Checks that the proof can be stored and loaded.
#[db_test]
async fn test_store_proof(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Attempt to load the proof that was not stored should result in an error.
    assert!(ProverSchema(&mut storage)
        .load_proof(1)
        .await
        .expect("Error while obtaining proof")
        .is_none());

    // Store the proof.
    let proof = EncodedProofPlonk::default();
    assert!(ProverSchema(&mut storage)
        .store_proof(1, &proof)
        .await
        .is_ok());

    // Now load it.
    let loaded = ProverSchema(&mut storage).load_proof(1).await?;
    assert_eq!(loaded, Some(proof));

    Ok(())
}

/// Checks the prover registration workflow, including
/// adding a new prover, stopping and resuming it.
#[db_test]
async fn prover_registration(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Attempt to load the non-existent prover should result in an error.
    assert!(ProverSchema(&mut storage).prover_by_id(1).await.is_err());

    // Add the prover.
    let prover_name = "prover_10";
    let block_size = 10;
    let prover_id = ProverSchema(&mut storage)
        .register_prover(prover_name, block_size)
        .await?;

    // Check that prover is added to the database.
    let prover = ProverSchema(&mut storage).prover_by_id(prover_id).await?;

    assert_eq!(prover.id, prover_id);
    assert_eq!(prover.worker, prover_name);
    assert_eq!(prover.block_size, block_size as i64);
    assert_eq!(prover.stopped_at, None);

    // Stop the prover.
    ProverSchema(&mut storage)
        .record_prover_stop(prover_id)
        .await?;

    // Check that it has been marked as stopped.
    let prover = ProverSchema(&mut storage).prover_by_id(prover_id).await?;
    assert!(prover.stopped_at.is_some());

    Ok(())
}

/// Checks the workflow of registering a prover run.
/// - Register a prover.
/// - Create a block that is committed and not verified.
/// - Obtain a prover run for that block.
/// - Check that we won't create another prover run for that block.
/// - Store a proof.
/// - Check that we won't create another prover run for that block once it's verified.
/// - Create a new block & obtain a prover run for it.
#[db_test]
async fn prover_run(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Add the prover.
    let prover_name = "prover_10";
    let block_size = ConfigurationOptions::from_env().available_block_chunk_sizes[0]; //smallest block size
    let _prover_id = ProverSchema(&mut storage)
        .register_prover(prover_name, block_size)
        .await?;

    // Create a block.
    BlockSchema(&mut storage)
        .execute_operation(get_operation(1, Action::Commit, block_size))
        .await?;

    // Get a prover run.
    let maybe_run = ProverSchema(&mut storage)
        .prover_run_for_next_commit(prover_name, Duration::from_secs(1), block_size)
        .await?;
    let run = maybe_run.expect("Can't get a prover run with a block committed");

    assert_eq!(run.block_number, 1);
    assert_eq!(run.worker, Some(prover_name.into()));
    // Initially creation and update time should be equal.
    assert_eq!(run.created_at, run.updated_at);

    // Try to get another run.
    let maybe_run = ProverSchema(&mut storage)
        .prover_run_for_next_commit(prover_name, Duration::from_secs(1), block_size)
        .await?;
    assert!(
        maybe_run.is_none(),
        "There should be no run when one is already created"
    );

    // Create & store proof for the first block.
    let proof = EncodedProofPlonk::default();
    assert!(ProverSchema(&mut storage)
        .store_proof(1, &proof)
        .await
        .is_ok());

    // Try to get another run. There should be none, since there are no blocks to prover.
    let maybe_run = ProverSchema(&mut storage)
        .prover_run_for_next_commit(prover_name, Duration::from_secs(1), block_size)
        .await?;
    assert!(
        maybe_run.is_none(),
        "There should be no run when the only block is proved"
    );

    // Create one more block.
    BlockSchema(&mut storage)
        .execute_operation(get_operation(2, Action::Commit, block_size))
        .await?;

    // Now we should get a prover run for the second block.
    let maybe_run = ProverSchema(&mut storage)
        .prover_run_for_next_commit(prover_name, Duration::from_secs(1), block_size)
        .await?;
    let run = maybe_run.expect("Can't get a prover run with a block committed");

    assert_eq!(run.block_number, 2);
    assert_eq!(run.worker, Some(prover_name.into()));
    assert_eq!(run.created_at, run.updated_at);

    Ok(())
}

/// Checks that `unstarted_jobs_count` method of schema returns the amount
/// of blocks for which proof is not generating (or generated) yet.
#[db_test]
async fn unstarted_prover_jobs_count(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Add the prover.
    let prover_name = "prover_10";
    let block_size = ConfigurationOptions::from_env().available_block_chunk_sizes[0]; //smallest block size
    let _prover_id = ProverSchema(&mut storage)
        .register_prover(prover_name, block_size)
        .await?;

    // Initially there are no blocks to prove.
    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 0);

    // Create a some blocks.
    BlockSchema(&mut storage)
        .execute_operation(get_operation(1, Action::Commit, block_size))
        .await?;
    BlockSchema(&mut storage)
        .execute_operation(get_operation(2, Action::Commit, block_size))
        .await?;
    BlockSchema(&mut storage)
        .execute_operation(get_operation(3, Action::Commit, block_size))
        .await?;

    // We've created 3 blocks and no jobs were assigned yet.
    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 3);

    // Create a prover run.
    ProverSchema(&mut storage)
        .prover_run_for_next_commit(prover_name, Duration::from_secs(1), block_size)
        .await?;

    // Now, as the job started, the number of not started jobs must be 2.
    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 2);

    // Create & store proof for the first block.
    let proof = EncodedProofPlonk::default();
    assert!(ProverSchema(&mut storage)
        .store_proof(1, &proof)
        .await
        .is_ok());

    // After saving the block there still should be 2 not started jobs.
    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 2);

    // Create next run & repeat checks.
    ProverSchema(&mut storage)
        .prover_run_for_next_commit(prover_name, Duration::from_secs(2), block_size)
        .await?;

    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 1);
    let proof = EncodedProofPlonk::default();
    assert!(ProverSchema(&mut storage)
        .store_proof(2, &proof)
        .await
        .is_ok());
    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 1);

    // And finally store the proof for the third block.
    ProverSchema(&mut storage)
        .prover_run_for_next_commit(prover_name, Duration::from_secs(3), block_size)
        .await?;

    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 0);
    let proof = EncodedProofPlonk::default();
    assert!(ProverSchema(&mut storage)
        .store_proof(3, &proof)
        .await
        .is_ok());
    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 0);

    // Then, when all the blocks are verified, create on more commit and check
    // that amount is increased again.
    BlockSchema(&mut storage)
        .execute_operation(get_operation(4, Action::Commit, block_size))
        .await?;
    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 1);

    // Add pending block. Amount of blocks should increase.

    BlockSchema(&mut storage)
        .save_pending_block(PendingBlock {
            number: 5,
            chunks_left: 0,
            unprocessed_priority_op_before: 0,
            pending_block_iteration: 1,
            success_operations: vec![],
            failed_txs: Vec::new(),
        })
        .await?;
    let blocks_count = ProverSchema(&mut storage).unstarted_jobs_count().await?;
    assert_eq!(blocks_count, 2);

    Ok(())
}
