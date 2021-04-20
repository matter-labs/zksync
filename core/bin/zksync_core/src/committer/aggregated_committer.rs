use chrono::{DateTime, Utc};
use std::cmp::max;
use std::time::Duration;
use zksync_config::ZkSyncConfig;
use zksync_crypto::proof::AggregatedProof;
use zksync_storage::chain::block::BlockSchema;
use zksync_storage::chain::operations::OperationsSchema;
use zksync_storage::prover::ProverSchema;
use zksync_storage::StorageProcessor;
use zksync_types::aggregated_operations::{
    AggregatedActionType, AggregatedOperation, BlocksCommitOperation, BlocksCreateProofOperation,
    BlocksExecuteOperation, BlocksProofOperation,
};
use zksync_types::{block::Block, gas_counter::GasCounter, BlockNumber, U256};

fn create_new_commit_operation(
    last_committed_block: &Block,
    new_blocks: &[Block],
    current_time: DateTime<Utc>,
    max_blocks_to_commit: usize,
    block_commit_deadline: Duration,
    max_gas_for_tx: U256,
    fast_processing: bool,
) -> Option<BlocksCommitOperation> {
    let new_blocks = new_blocks
        .iter()
        .cloned()
        .take(max_blocks_to_commit)
        .collect::<Vec<_>>();
    let any_block_commit_deadline_triggered = {
        let block_commit_deadline_seconds = block_commit_deadline.as_secs() as i64;
        new_blocks.iter().any(|block| {
            let seconds_since_block_created = max(
                current_time
                    // todo: block timestamp?
                    .signed_duration_since(block.timestamp_utc())
                    .num_seconds(),
                0,
            );
            seconds_since_block_created > block_commit_deadline_seconds
        })
    };

    let gas_limit_reached_for_blocks =
        GasCounter::commit_gas_limit_aggregated(&new_blocks) >= max_gas_for_tx;

    let should_commit_blocks = any_block_commit_deadline_triggered
        || gas_limit_reached_for_blocks
        || new_blocks.len() == max_blocks_to_commit
        || fast_processing;
    if !should_commit_blocks {
        return None;
    }

    let mut blocks_to_commit = Vec::new();
    let mut commit_tx_gas = U256::from(GasCounter::BASE_COMMIT_BLOCKS_TX_COST);
    for new_block in &new_blocks {
        if commit_tx_gas + new_block.commit_gas_limit > max_gas_for_tx {
            break;
        }
        blocks_to_commit.push(new_block.clone());
        commit_tx_gas += new_block.commit_gas_limit;
    }
    assert!(!blocks_to_commit.is_empty());

    Some(BlocksCommitOperation {
        last_committed_block: last_committed_block.clone(),
        blocks: blocks_to_commit,
    })
}

fn create_new_create_proof_operation(
    new_blocks_with_proofs: &[Block],
    available_aggregate_proof_sizes: &[usize],
    current_time: DateTime<Utc>,
    block_verify_deadline: Duration,
    _max_gas_for_tx: U256,
    fast_processing: bool,
) -> Option<BlocksCreateProofOperation> {
    let max_aggregate_size = available_aggregate_proof_sizes
        .last()
        .cloned()
        .expect("should have at least one aggregate proof size");

    let any_block_verify_deadline_triggered = {
        let block_verify_deadline = block_verify_deadline.as_secs() as i64;
        new_blocks_with_proofs
            .iter()
            .take(max_aggregate_size)
            .any(|block| {
                let seconds_since_block_created = max(
                    current_time
                        .signed_duration_since(block.timestamp_utc())
                        .num_seconds(),
                    0,
                );
                seconds_since_block_created > block_verify_deadline
            })
    };

    let can_create_max_aggregate_proof = new_blocks_with_proofs.len() >= max_aggregate_size;

    let should_create_aggregate_proof =
        any_block_verify_deadline_triggered || can_create_max_aggregate_proof || fast_processing;

    if !should_create_aggregate_proof {
        return None;
    }

    // get max possible aggregate size
    let aggregate_proof_size = available_aggregate_proof_sizes
        .iter()
        .rev()
        .find(|aggregate_size| {
            *aggregate_size >= &std::cmp::min(new_blocks_with_proofs.len(), max_aggregate_size)
        })
        .cloned()
        .expect("failed to find correct aggregate proof size");

    let blocks = new_blocks_with_proofs
        .iter()
        .take(aggregate_proof_size)
        .cloned()
        .collect::<Vec<_>>();

    let proofs_to_pad = aggregate_proof_size
        .checked_sub(blocks.len())
        .expect("incorrect aggregate proof size");

    Some(BlocksCreateProofOperation {
        blocks,
        proofs_to_pad,
    })
}

fn create_publish_proof_operation(
    unpublished_create_proof_op: &BlocksCreateProofOperation,
    aggregated_proof: &AggregatedProof,
) -> BlocksProofOperation {
    BlocksProofOperation {
        blocks: unpublished_create_proof_op.blocks.clone(),
        proof: aggregated_proof.serialize_aggregated_proof(),
    }
}

fn create_execute_blocks_operation(
    proven_non_executed_block: &[Block],
    current_time: DateTime<Utc>,
    max_blocks_to_execute: usize,
    block_execute_deadline: Duration,
    max_gas_for_tx: U256,
    fast_processing: bool,
) -> Option<BlocksExecuteOperation> {
    let proven_non_executed_block = proven_non_executed_block
        .iter()
        .cloned()
        .take(max_blocks_to_execute)
        .collect::<Vec<_>>();
    let any_block_execute_deadline_triggered = {
        let block_execute_deadline_seconds = block_execute_deadline.as_secs() as i64;
        proven_non_executed_block.iter().any(|block| {
            let seconds_since_block_created = max(
                current_time
                    .signed_duration_since(block.timestamp_utc())
                    .num_seconds(),
                0,
            );
            seconds_since_block_created > block_execute_deadline_seconds
        })
    };

    let gas_limit_reached_for_blocks =
        GasCounter::execute_gas_limit_aggregated(&proven_non_executed_block) >= max_gas_for_tx;

    let should_execute_blocks = any_block_execute_deadline_triggered
        || gas_limit_reached_for_blocks
        || proven_non_executed_block.len() == max_blocks_to_execute
        || fast_processing;
    if !should_execute_blocks {
        return None;
    }

    let mut blocks_to_execute = Vec::new();
    let mut execute_tx_gas = U256::from(GasCounter::BASE_EXECUTE_BLOCKS_TX_COST);
    for block in &proven_non_executed_block {
        if execute_tx_gas + block.verify_gas_limit > max_gas_for_tx {
            break;
        }
        blocks_to_execute.push(block.clone());
        execute_tx_gas += block.verify_gas_limit;
    }
    assert!(!blocks_to_execute.is_empty());

    Some(BlocksExecuteOperation {
        blocks: blocks_to_execute,
    })
}

/// Checks if fast processing is required for any `Block`
async fn is_fast_processing_requested(
    storage: &mut StorageProcessor<'_>,
    blocks: &[Block],
) -> anyhow::Result<bool> {
    let mut fast_processing = false;
    for block in blocks {
        let fast_processing_for_current_block_requested = BlockSchema(storage)
            .get_block_metadata(block.block_number)
            .await?
            .map(|mdat| mdat.fast_processing)
            .unwrap_or(false);

        fast_processing = fast_processing || fast_processing_for_current_block_requested;
        if fast_processing {
            break;
        }
    }
    return Ok(fast_processing);
}

async fn create_aggregated_commits_storage(
    storage: &mut StorageProcessor<'_>,
    config: &ZkSyncConfig,
) -> anyhow::Result<bool> {
    let mut transaction = storage.start_transaction().await?;
    let last_aggregate_committed_block = OperationsSchema(&mut transaction)
        .get_last_affected_block_by_aggregated_action(AggregatedActionType::CommitBlocks)
        .await?;
    let old_committed_block = BlockSchema(&mut transaction)
        .get_block(last_aggregate_committed_block)
        .await?
        .expect("Failed to get last committed block from db");

    let mut new_blocks = Vec::new();
    let mut block_number = last_aggregate_committed_block + 1;

    while let Some(block) = BlockSchema(&mut transaction)
        .get_block(block_number)
        .await?
    {
        new_blocks.push(block);
        block_number.0 += 1;
    }

    let fast_processing_requested =
        is_fast_processing_requested(&mut transaction, &new_blocks).await?;

    let commit_operation = create_new_commit_operation(
        &old_committed_block,
        &new_blocks,
        Utc::now(),
        config.chain.state_keeper.max_aggregated_blocks_to_commit,
        config.chain.state_keeper.block_commit_deadline(),
        config.chain.state_keeper.max_aggregated_tx_gas.into(),
        fast_processing_requested,
    );

    let result = if let Some(commit_operation) = commit_operation {
        let aggregated_op = commit_operation.into();
        log_aggregated_op_creation(&aggregated_op);
        OperationsSchema(&mut transaction)
            .store_aggregated_action(aggregated_op)
            .await?;
        Ok(true)
    } else {
        Ok(false)
    };

    transaction.commit().await?;
    result
}

async fn create_aggregated_prover_task_storage(
    storage: &mut StorageProcessor<'_>,
    config: &ZkSyncConfig,
) -> anyhow::Result<bool> {
    let mut transaction = storage.start_transaction().await?;
    let last_aggregate_committed_block = OperationsSchema(&mut transaction)
        .get_last_affected_block_by_aggregated_action(AggregatedActionType::CommitBlocks)
        .await?;
    let last_aggregate_create_proof_block = OperationsSchema(&mut transaction)
        .get_last_affected_block_by_aggregated_action(AggregatedActionType::CreateProofBlocks)
        .await?;
    if last_aggregate_committed_block <= last_aggregate_create_proof_block {
        return Ok(false);
    }

    let mut blocks_with_proofs = Vec::new();
    for block_number in last_aggregate_create_proof_block.0 + 1..=last_aggregate_committed_block.0 {
        let block_number = BlockNumber(block_number);
        let proof_exists = ProverSchema(&mut transaction)
            .load_proof(block_number)
            .await?
            .is_some();
        if proof_exists {
            let block = BlockSchema(&mut transaction)
                .get_block(block_number)
                .await?
                .expect("failed to fetch committed block from db");
            blocks_with_proofs.push(block);
        } else {
            break;
        }
    }

    let fast_processing_requested =
        is_fast_processing_requested(&mut transaction, &blocks_with_proofs).await?;

    let create_proof_operation = create_new_create_proof_operation(
        &blocks_with_proofs,
        &config.chain.state_keeper.aggregated_proof_sizes,
        Utc::now(),
        config.chain.state_keeper.block_prove_deadline(),
        config.chain.state_keeper.max_aggregated_tx_gas.into(),
        fast_processing_requested,
    );
    let result = if let Some(operation) = create_proof_operation {
        let aggregated_op = operation.into();
        log_aggregated_op_creation(&aggregated_op);
        OperationsSchema(&mut transaction)
            .store_aggregated_action(aggregated_op)
            .await?;
        Ok(true)
    } else {
        Ok(false)
    };

    transaction.commit().await?;
    result
}

async fn create_aggregated_publish_proof_operation_storage(
    storage: &mut StorageProcessor<'_>,
) -> anyhow::Result<bool> {
    let mut transaction = storage.start_transaction().await?;
    let last_aggregate_create_proof_block = OperationsSchema(&mut transaction)
        .get_last_affected_block_by_aggregated_action(AggregatedActionType::CreateProofBlocks)
        .await?;
    let last_aggregate_publish_proof_block = OperationsSchema(&mut transaction)
        .get_last_affected_block_by_aggregated_action(
            AggregatedActionType::PublishProofBlocksOnchain,
        )
        .await?;
    if last_aggregate_create_proof_block <= last_aggregate_publish_proof_block {
        return Ok(false);
    }

    let last_unpublished_create_proof_operation = {
        let (_, aggregated_operation) = OperationsSchema(&mut transaction)
            .get_aggregated_op_that_affects_block(
                AggregatedActionType::CreateProofBlocks,
                last_aggregate_publish_proof_block + 1,
            )
            .await?
            .expect("Unpublished create proof operation should exist");
        if let AggregatedOperation::CreateProofBlocks(create_proof_blocks) = aggregated_operation {
            create_proof_blocks
        } else {
            panic!("Incorrect aggregate operation type")
        }
    };

    let aggregated_proof = {
        assert!(
            !last_unpublished_create_proof_operation.blocks.is_empty(),
            "should have 1 block"
        );
        let first_block = last_unpublished_create_proof_operation
            .blocks
            .first()
            .map(|b| b.block_number)
            .unwrap();
        let last_block = last_unpublished_create_proof_operation
            .blocks
            .last()
            .map(|b| b.block_number)
            .unwrap();
        transaction
            .prover_schema()
            .load_aggregated_proof(first_block, last_block)
            .await?
    };

    let result = if let Some(proof) = aggregated_proof {
        let operation =
            create_publish_proof_operation(&last_unpublished_create_proof_operation, &proof);
        let aggregated_op = operation.into();
        log_aggregated_op_creation(&aggregated_op);
        OperationsSchema(&mut transaction)
            .store_aggregated_action(aggregated_op)
            .await?;
        Ok(true)
    } else {
        Ok(false)
    };

    transaction.commit().await?;
    result
}

async fn create_aggregated_execute_operation_storage(
    storage: &mut StorageProcessor<'_>,
    config: &ZkSyncConfig,
) -> anyhow::Result<bool> {
    let mut transaction = storage.start_transaction().await?;
    let last_aggregate_executed_block = OperationsSchema(&mut transaction)
        .get_last_affected_block_by_aggregated_action(AggregatedActionType::ExecuteBlocks)
        .await?;
    let last_aggregate_publish_proof_block = OperationsSchema(&mut transaction)
        .get_last_affected_block_by_aggregated_action(
            AggregatedActionType::PublishProofBlocksOnchain,
        )
        .await?;

    if last_aggregate_publish_proof_block <= last_aggregate_executed_block {
        return Ok(false);
    }

    let mut blocks = Vec::new();
    for block_number in last_aggregate_executed_block.0 + 1..=last_aggregate_publish_proof_block.0 {
        let block = BlockSchema(&mut transaction)
            .get_block(BlockNumber(block_number))
            .await?
            .expect("Failed to get block that should be committed");
        blocks.push(block);
    }

    let fast_processing_requested = is_fast_processing_requested(&mut transaction, &blocks).await?;

    let execute_operation = create_execute_blocks_operation(
        &blocks,
        Utc::now(),
        config.chain.state_keeper.max_aggregated_blocks_to_execute,
        config.chain.state_keeper.block_execute_deadline(),
        config.chain.state_keeper.max_aggregated_tx_gas.into(),
        fast_processing_requested,
    );

    let result = if let Some(operation) = execute_operation {
        let aggregated_op = operation.into();
        log_aggregated_op_creation(&aggregated_op);
        OperationsSchema(&mut transaction)
            .store_aggregated_action(aggregated_op)
            .await?;
        Ok(true)
    } else {
        Ok(false)
    };

    transaction.commit().await?;
    result
}

pub async fn create_aggregated_operations_storage(
    storage: &mut StorageProcessor<'_>,
    config: &ZkSyncConfig,
) -> anyhow::Result<()> {
    while create_aggregated_commits_storage(storage, config).await? {}
    while create_aggregated_prover_task_storage(storage, config).await? {}
    while create_aggregated_publish_proof_operation_storage(storage).await? {}
    while create_aggregated_execute_operation_storage(storage, config).await? {}

    Ok(())
}

fn log_aggregated_op_creation(aggregated_op: &AggregatedOperation) {
    let (first, last) = aggregated_op.get_block_range();
    vlog::info!(
        "Created aggregated operation: {}, blocks: [{},{}]",
        aggregated_op.get_action_type().to_string(),
        first,
        last
    );
}
