// External imports
use anyhow::format_err;
// Workspace imports
use zksync_types::{
    prover::{ProverJob, ProverJobType},
    Action,
};
// Local imports
use crate::test_data::{gen_operation, get_sample_aggregated_proof, get_sample_single_proof};
use crate::tests::db_test;
use crate::{prover::ProverSchema, QueryResult, StorageProcessor};

async fn get_idle_job_from_queue(mut storage: &mut StorageProcessor<'_>) -> QueryResult<ProverJob> {
    let job = ProverSchema(&mut storage)
        .get_idle_prover_job_from_job_queue()
        .await?;

    job.ok_or(format_err!("expect idle job from job queue"))
}

/// Checks that the `prover_job_queue` correctly processes requests to it.
/// `prover_job_queue` table is locked when accessed, so it cannot be accessed simultaneously.
#[db_test]
async fn test_prover_job_queue(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    test_store_proof(&mut storage).await?;
    pending_jobs_count(&mut storage).await?;

    Ok(())
}

/// Checks that the single and aggregated proof can be stored and loaded.
async fn test_store_proof(mut storage: &mut StorageProcessor<'_>) -> QueryResult<()> {
    // Attempt to load the proof that was not stored should result in None.
    let (loaded_proof, loaded_aggregated_proof) = (
        ProverSchema(&mut storage)
            .load_proof(1)
            .await
            .expect("Error while obtaining proof"),
        ProverSchema(&mut storage)
            .load_aggregated_proof(1, 1)
            .await
            .expect("Error while obtaining proof"),
    );
    assert!(loaded_proof.is_none());
    assert!(loaded_aggregated_proof.is_none());

    // Attempt to store the proof for which there is no associated job in `job_prover_queue`.
    let (proof, aggregated_proof) = (get_sample_single_proof(), get_sample_aggregated_proof());

    let (stored_proof, stored_aggregated_proof) = (
        ProverSchema(&mut storage).store_proof(1, 1, &proof).await,
        ProverSchema(&mut storage)
            .store_aggregated_proof(1, 1, 1, &aggregated_proof)
            .await,
    );

    assert!(stored_proof
        .err()
        .unwrap()
        .to_string()
        .contains("Missing job for stored proof"));
    assert!(stored_aggregated_proof
        .err()
        .unwrap()
        .to_string()
        .contains("Missing job for stored aggregated proof"));

    // Add jobs to `job_prover_queue`.
    let job_data = serde_json::Value::default();
    let (stored_job, stored_aggregated_job) = (
        ProverSchema(&mut storage)
            .add_prover_job_to_job_queue(1, 1, job_data.clone(), 0, ProverJobType::SingleProof)
            .await,
        ProverSchema(&mut storage)
            .add_prover_job_to_job_queue(1, 1, job_data, 1, ProverJobType::AggregatedProof)
            .await,
    );
    assert!(stored_job.is_ok());
    assert!(stored_aggregated_job.is_ok());

    // Get job id.
    let (stored_job_id, stored_aggregated_job_id) = (
        get_idle_job_from_queue(&mut storage).await?.job_id,
        get_idle_job_from_queue(&mut storage).await?.job_id,
    );

    // Store proofs.
    let (stored_proof, stored_aggregated_proof) = (
        ProverSchema(&mut storage)
            .store_proof(stored_job_id, 1, &proof)
            .await,
        ProverSchema(&mut storage)
            .store_aggregated_proof(stored_aggregated_job_id, 1, 1, &aggregated_proof)
            .await,
    );
    assert!(stored_proof.is_ok());
    assert!(stored_aggregated_proof.is_ok());

    // Now load it.
    let (loaded_proof, loaded_aggregated_proof) = (
        ProverSchema(&mut storage).load_proof(1).await?,
        ProverSchema(&mut storage)
            .load_aggregated_proof(1, 1)
            .await?,
    );
    assert!(loaded_proof.is_some());
    assert!(loaded_aggregated_proof.is_some());

    Ok(())
}

/// Checks that `pending_jobs_count` method of schema returns the amount
/// of jobs for which proof is not generating (or generated) yet.
async fn pending_jobs_count(mut storage: &mut StorageProcessor<'_>) -> QueryResult<()> {
    // Initially there are no jobs.
    let jobs_count = ProverSchema(&mut storage).pending_jobs_count().await?;
    assert_eq!(jobs_count, 0);

    // Create a some jobs.
    ProverSchema(&mut storage)
        .add_prover_job_to_job_queue(2, 2, Default::default(), 1, ProverJobType::SingleProof)
        .await?;
    ProverSchema(&mut storage)
        .add_prover_job_to_job_queue(3, 3, Default::default(), 1, ProverJobType::SingleProof)
        .await?;
    ProverSchema(&mut storage)
        .add_prover_job_to_job_queue(2, 3, Default::default(), 0, ProverJobType::AggregatedProof)
        .await?;

    // We've created 3 jobs and no jobs were assigned yet.
    let jobs_count = ProverSchema(&mut storage).pending_jobs_count().await?;
    assert_eq!(jobs_count, 3);

    // Get a job and now the number of idle jobs must be 2.
    let first_job = get_idle_job_from_queue(&mut storage).await?;
    let jobs_count = ProverSchema(&mut storage).pending_jobs_count().await?;
    assert_eq!(jobs_count, 2);

    // Create next run & repeat checks.
    let second_job = get_idle_job_from_queue(&mut storage).await?;
    let jobs_count = ProverSchema(&mut storage).pending_jobs_count().await?;
    assert_eq!(jobs_count, 1);

    // And finally store the proof for the third block.
    let third_job = get_idle_job_from_queue(&mut storage).await?;
    let jobs_count = ProverSchema(&mut storage).pending_jobs_count().await?;
    assert_eq!(jobs_count, 0);

    // Record prover is working and stopped it.
    ProverSchema(&mut storage)
        .record_prover_is_working(first_job.job_id, "test_prover")
        .await?;
    ProverSchema(&mut storage)
        .record_prover_is_working(second_job.job_id, "test_prover")
        .await?;
    ProverSchema(&mut storage)
        .record_prover_is_working(third_job.job_id, "test_prover")
        .await?;

    // Store one proof and then turn off the prover.
    ProverSchema(&mut storage)
        .store_proof(
            third_job.job_id,
            third_job.first_block,
            &get_sample_single_proof(),
        )
        .await?;
    let jobs_count = ProverSchema(&mut storage).pending_jobs_count().await?;
    assert_eq!(jobs_count, 0);

    ProverSchema(&mut storage)
        .record_prover_stop("test_prover")
        .await?;

    let jobs_count = ProverSchema(&mut storage).pending_jobs_count().await?;
    assert_eq!(jobs_count, 2);

    Ok(())
}

/// Checks that the witness can be stored and loaded.
#[db_test]
async fn test_store_witness(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    const BLOCK_NUMBER: u32 = 1;
    const BLOCK_SIZE: usize = 100;
    // No witness stored for the block.
    assert!(storage
        .prover_schema()
        .get_witness(BLOCK_NUMBER)
        .await?
        .is_none());

    // FK constraint.
    storage
        .chain()
        .block_schema()
        .execute_operation(gen_operation(BLOCK_NUMBER, Action::Commit, BLOCK_SIZE))
        .await?;

    // Store the witness.
    let expected = String::from("test");
    let witness = serde_json::to_value(expected.clone()).unwrap();
    storage
        .prover_schema()
        .store_witness(BLOCK_NUMBER, witness)
        .await?;

    // Now load it.
    let loaded = storage
        .prover_schema()
        .get_witness(BLOCK_NUMBER)
        .await?
        .map(|value| serde_json::from_value(value).unwrap());
    assert_eq!(loaded.as_ref(), Some(&expected));

    // Do nothing on conflict.
    let not_expected = String::from("__test");
    let witness = serde_json::to_value(expected.clone()).unwrap();
    storage
        .prover_schema()
        .store_witness(BLOCK_NUMBER, witness)
        .await?;

    let loaded = storage
        .prover_schema()
        .get_witness(BLOCK_NUMBER)
        .await?
        .map(|value| serde_json::from_value(value).unwrap());
    assert_ne!(loaded, Some(not_expected));
    assert_eq!(loaded, Some(expected));

    Ok(())
}
