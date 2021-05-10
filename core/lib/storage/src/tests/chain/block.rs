// External imports
// Workspace imports
use zksync_crypto::{convert::FeConvert, rand::XorShiftRng};
use zksync_types::{
    aggregated_operations::AggregatedActionType, helpers::apply_updates, tx::ChangePubKeyType,
    AccountId, AccountMap, AccountUpdate, AccountUpdates, BlockNumber, TokenId, H256,
};
// Local imports
use crate::{
    chain::{
        block::{records::StorageBlockDetails, BlockSchema},
        operations::OperationsSchema,
        state::StateSchema,
    },
    ethereum::EthereumSchema,
    test_data::{
        dummy_ethereum_tx_hash, gen_acc_random_updates, gen_sample_block,
        gen_unique_aggregated_operation, BLOCK_SIZE_CHUNKS,
    },
    tests::{create_rng, db_test},
    QueryResult, StorageProcessor,
};

/// Creates several random updates for the provided account map,
/// and returns the resulting account map together with the list
/// of generated updates.
pub fn apply_random_updates(
    mut accounts: AccountMap,
    rng: &mut XorShiftRng,
) -> (AccountMap, Vec<(AccountId, AccountUpdate)>) {
    let updates = (0..3)
        .flat_map(|_| gen_acc_random_updates(rng))
        .collect::<AccountUpdates>();
    apply_updates(&mut accounts, updates.clone());
    (accounts, updates)
}

/// Here we create updates for blocks 1,2,3 (commit 3 blocks)
/// We apply updates for blocks 1,2 (verify 2 blocks)
/// Make sure that we can get state for all blocks.
#[db_test]
async fn test_commit_rewind(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut rng = create_rng();

    // Create the input data for three blocks.
    // Data for the next block is based on previous block data.
    let (accounts_block_1, updates_block_1) = apply_random_updates(AccountMap::default(), &mut rng);
    let (accounts_block_2, updates_block_2) =
        apply_random_updates(accounts_block_1.clone(), &mut rng);
    let (accounts_block_3, updates_block_3) =
        apply_random_updates(accounts_block_2.clone(), &mut rng);

    // Execute and commit these blocks.
    // Also store account updates.
    BlockSchema(&mut storage)
        .save_block(gen_sample_block(
            BlockNumber(1),
            BLOCK_SIZE_CHUNKS,
            Default::default(),
        ))
        .await?;
    StateSchema(&mut storage)
        .commit_state_update(BlockNumber(1), &updates_block_1, 0)
        .await?;
    BlockSchema(&mut storage)
        .save_block(gen_sample_block(
            BlockNumber(2),
            BLOCK_SIZE_CHUNKS,
            Default::default(),
        ))
        .await?;
    StateSchema(&mut storage)
        .commit_state_update(BlockNumber(2), &updates_block_2, 0)
        .await?;
    BlockSchema(&mut storage)
        .save_block(gen_sample_block(
            BlockNumber(3),
            BLOCK_SIZE_CHUNKS,
            Default::default(),
        ))
        .await?;
    StateSchema(&mut storage)
        .commit_state_update(BlockNumber(3), &updates_block_3, 0)
        .await?;

    // Check that they are stored in state.
    let (block, state) = StateSchema(&mut storage)
        .load_committed_state(Some(BlockNumber(1)))
        .await?;
    assert_eq!((block, &state), (BlockNumber(1), &accounts_block_1));

    let (block, state) = StateSchema(&mut storage)
        .load_committed_state(Some(BlockNumber(2)))
        .await?;
    assert_eq!((block, &state), (BlockNumber(2), &accounts_block_2));

    let (block, state) = StateSchema(&mut storage)
        .load_committed_state(Some(BlockNumber(3)))
        .await?;
    assert_eq!((block, &state), (BlockNumber(3), &accounts_block_3));

    // Add proofs for the first two blocks.
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            BlockNumber(1),
            AggregatedActionType::ExecuteBlocks,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            BlockNumber(2),
            AggregatedActionType::ExecuteBlocks,
            BLOCK_SIZE_CHUNKS,
        ))
        .await?;

    // Check that we still can get the state for these blocks.
    let (block, state) = StateSchema(&mut storage)
        .load_committed_state(Some(BlockNumber(1)))
        .await?;
    assert_eq!((block, &state), (BlockNumber(1), &accounts_block_1));

    let (block, state) = StateSchema(&mut storage)
        .load_committed_state(Some(BlockNumber(2)))
        .await?;
    assert_eq!((block, &state), (BlockNumber(2), &accounts_block_2));

    let (block, state) = StateSchema(&mut storage)
        .load_committed_state(Some(BlockNumber(3)))
        .await?;
    assert_eq!((block, &state), (BlockNumber(3), &accounts_block_3));

    // Check that with no id provided, the latest state is loaded.
    let (block, state) = StateSchema(&mut storage).load_committed_state(None).await?;
    assert_eq!((block, &state), (BlockNumber(3), &accounts_block_3));

    Ok(())
}

/// Checks that `find_block_by_height_or_hash` method allows
/// to load the block details by either its height, hash of the included
/// transaction, or the root hash of the block.
#[db_test]
async fn test_find_block_by_height_or_hash(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    /// The actual test check. It obtains the block details using
    /// the `find_block_by_height_or_hash` method with different types of query,
    /// and compares them against the provided sample.
    async fn check_find_block_by_height_or_hash(
        storage: &mut StorageProcessor<'_>,
        expected_block_detail: &StorageBlockDetails,
    ) -> QueryResult<()> {
        let mut queries = vec![
            expected_block_detail.block_number.to_string(),
            hex::encode(&expected_block_detail.new_state_root),
            hex::encode(&expected_block_detail.commit_tx_hash.as_ref().unwrap()),
        ];
        if let Some(verify_tx_hash) = expected_block_detail.verify_tx_hash.as_ref() {
            queries.push(hex::encode(&verify_tx_hash));
        }

        for query in queries {
            let actual_block_detail = BlockSchema(storage)
                .find_block_by_height_or_hash(query.clone())
                .await
                .unwrap_or_else(|| {
                    panic!(
                        "Can't load the existing block with the index {} using query {}",
                        expected_block_detail.block_number, query
                    )
                });
            assert_eq!(
                actual_block_detail.block_number,
                expected_block_detail.block_number
            );
            assert_eq!(
                actual_block_detail.new_state_root,
                expected_block_detail.new_state_root
            );
            assert_eq!(
                actual_block_detail.commit_tx_hash,
                expected_block_detail.commit_tx_hash
            );
            assert_eq!(
                actual_block_detail.verify_tx_hash,
                expected_block_detail.verify_tx_hash
            );
        }

        Ok(())
    }

    // Below the initialization of the data for the test and collecting
    // the reference block detail samples.

    let mut rng = create_rng();

    // Required since we use `EthereumSchema` in this test.
    EthereumSchema(&mut storage).initialize_eth_data().await?;

    let mut accounts_map = AccountMap::default();
    let n_committed = 5u32;
    let n_verified = n_committed - 2;

    let mut expected_outcome: Vec<StorageBlockDetails> = Vec::new();

    // Create and apply several blocks to work with.
    for block_number in 1..=n_committed {
        let block_number = BlockNumber(block_number);
        // Create blanked block detail object which we will fill
        // with the relevant data and use for the comparison later.
        let mut current_block_detail = StorageBlockDetails {
            block_number: 0,
            new_state_root: Default::default(),
            block_size: 0,
            commit_tx_hash: None,
            verify_tx_hash: None,
            committed_at: chrono::DateTime::from_utc(
                chrono::NaiveDateTime::from_timestamp(0, 0),
                chrono::Utc,
            ),
            verified_at: None,
        };

        let (new_accounts_map, updates) = apply_random_updates(accounts_map.clone(), &mut rng);
        accounts_map = new_accounts_map;

        // Store the block in the block schema.
        let block = gen_sample_block(block_number, BLOCK_SIZE_CHUNKS, Default::default());
        BlockSchema(&mut storage).save_block(block.clone()).await?;
        OperationsSchema(&mut storage)
            .store_aggregated_action(gen_unique_aggregated_operation(
                block_number,
                AggregatedActionType::CommitBlocks,
                BLOCK_SIZE_CHUNKS,
            ))
            .await?;
        let (id, op) = OperationsSchema(&mut storage)
            .get_aggregated_op_that_affects_block(AggregatedActionType::CommitBlocks, block_number)
            .await?
            .unwrap();

        StateSchema(&mut storage)
            .commit_state_update(block_number, &updates, 0)
            .await?;

        // Store & confirm the operation in the ethereum schema, as it's used for obtaining
        // commit/verify hashes.
        let eth_tx_hash = dummy_ethereum_tx_hash(id);

        let response = EthereumSchema(&mut storage)
            .save_new_eth_tx(
                AggregatedActionType::CommitBlocks,
                Some((id, op)),
                100,
                100u32.into(),
                Default::default(),
            )
            .await?;

        EthereumSchema(&mut storage)
            .add_hash_entry(response.id, &eth_tx_hash)
            .await?;
        EthereumSchema(&mut storage)
            .confirm_eth_tx(&eth_tx_hash)
            .await?;

        // Initialize reference sample fields.
        current_block_detail.block_number = *block.block_number as i64;
        current_block_detail.new_state_root = block.new_root_hash.to_bytes();
        current_block_detail.block_size = block.block_transactions.len() as i64;
        current_block_detail.commit_tx_hash = Some(eth_tx_hash.as_ref().to_vec());

        // Add verification for the block if required.
        if *block_number <= n_verified {
            OperationsSchema(&mut storage)
                .store_aggregated_action(gen_unique_aggregated_operation(
                    block_number,
                    AggregatedActionType::ExecuteBlocks,
                    BLOCK_SIZE_CHUNKS,
                ))
                .await?;
            let (id, op) = OperationsSchema(&mut storage)
                .get_aggregated_op_that_affects_block(
                    AggregatedActionType::ExecuteBlocks,
                    block_number,
                )
                .await?
                .unwrap();
            let eth_tx_hash = dummy_ethereum_tx_hash(id);

            // Do not add an ethereum confirmation for the last operation.
            if *block_number != n_verified {
                let response = EthereumSchema(&mut storage)
                    .save_new_eth_tx(
                        AggregatedActionType::CreateProofBlocks,
                        Some((id, op)),
                        100,
                        100u32.into(),
                        Default::default(),
                    )
                    .await?;
                EthereumSchema(&mut storage)
                    .add_hash_entry(response.id, &eth_tx_hash)
                    .await?;
                EthereumSchema(&mut storage)
                    .confirm_eth_tx(&eth_tx_hash)
                    .await?;
                current_block_detail.verify_tx_hash = Some(eth_tx_hash.as_ref().to_vec());
            }
        }

        // Store the sample.
        expected_outcome.push(current_block_detail);
    }
    // Run the tests against the collected data.
    for expected_block_detail in expected_outcome {
        check_find_block_by_height_or_hash(&mut storage, &expected_block_detail).await?;
    }

    // Also check that we get `None` for non-existing block.
    let query = 10000.to_string();
    assert!(BlockSchema(&mut storage)
        .find_block_by_height_or_hash(query)
        .await
        .is_none());

    Ok(())
}

/// Checks that `load_block_range` method loads the range of blocks correctly.
#[db_test]
async fn test_block_range(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    /// Loads the block range and checks that every block in the response is
    /// equal to the one obtained from `find_block_by_height_or_hash` method.
    async fn check_block_range(
        storage: &mut StorageProcessor<'_>,
        max_block: BlockNumber,
        limit: u32,
    ) -> QueryResult<()> {
        let start_block = if *max_block >= limit {
            (max_block - limit) + 1
        } else {
            BlockNumber(1)
        };
        let block_range = BlockSchema(storage)
            .load_block_range(max_block, limit)
            .await?;
        // Go in the reversed order, since the blocks themselves are ordered backwards.
        for (idx, block_number) in (*start_block..=*max_block).rev().enumerate() {
            let expected = BlockSchema(storage)
                .find_block_by_height_or_hash(block_number.to_string())
                .await
                .unwrap_or_else(|| {
                    panic!(
                        "Can't load the existing block with the index {}",
                        block_number
                    )
                });
            let got = &block_range[idx];
            assert_eq!(got, &expected);
        }

        Ok(())
    }

    // Below lies the initialization of the data for the test.

    let mut rng = create_rng();

    // Required since we use `EthereumSchema` in this test.
    EthereumSchema(&mut storage).initialize_eth_data().await?;

    let mut accounts_map = AccountMap::default();
    let n_committed = 5;
    let n_verified = n_committed - 2;

    // Create and apply several blocks to work with.
    for block_number in 1..=n_committed {
        let block_number = BlockNumber(block_number);
        let (new_accounts_map, updates) = apply_random_updates(accounts_map.clone(), &mut rng);
        accounts_map = new_accounts_map;

        // Store the operation in the block schema.
        BlockSchema(&mut storage)
            .save_block(gen_sample_block(
                block_number,
                BLOCK_SIZE_CHUNKS,
                Default::default(),
            ))
            .await?;
        OperationsSchema(&mut storage)
            .store_aggregated_action(gen_unique_aggregated_operation(
                block_number,
                AggregatedActionType::CommitBlocks,
                BLOCK_SIZE_CHUNKS,
            ))
            .await?;
        let (id, op) = OperationsSchema(&mut storage)
            .get_aggregated_op_that_affects_block(AggregatedActionType::CommitBlocks, block_number)
            .await?
            .unwrap();
        StateSchema(&mut storage)
            .commit_state_update(block_number, &updates, 0)
            .await?;

        // Store & confirm the operation in the ethereum schema, as it's used for obtaining
        // commit/verify hashes.
        let eth_tx_hash = dummy_ethereum_tx_hash(id);
        let response = EthereumSchema(&mut storage)
            .save_new_eth_tx(
                AggregatedActionType::CommitBlocks,
                Some((id, op)),
                100,
                100u32.into(),
                Default::default(),
            )
            .await?;
        EthereumSchema(&mut storage)
            .add_hash_entry(response.id, &eth_tx_hash)
            .await?;
        EthereumSchema(&mut storage)
            .confirm_eth_tx(&eth_tx_hash)
            .await?;

        // Add verification for the block if required.
        if *block_number <= n_verified {
            OperationsSchema(&mut storage)
                .store_aggregated_action(gen_unique_aggregated_operation(
                    block_number,
                    AggregatedActionType::ExecuteBlocks,
                    BLOCK_SIZE_CHUNKS,
                ))
                .await?;
            let (id, op) = OperationsSchema(&mut storage)
                .get_aggregated_op_that_affects_block(
                    AggregatedActionType::ExecuteBlocks,
                    block_number,
                )
                .await?
                .unwrap();
            let eth_tx_hash = dummy_ethereum_tx_hash(id);
            let response = EthereumSchema(&mut storage)
                .save_new_eth_tx(
                    AggregatedActionType::ExecuteBlocks,
                    Some((id, op)),
                    100,
                    100u32.into(),
                    Default::default(),
                )
                .await?;
            EthereumSchema(&mut storage)
                .add_hash_entry(response.id, &eth_tx_hash)
                .await?;
            EthereumSchema(&mut storage)
                .confirm_eth_tx(&eth_tx_hash)
                .await?;
        }
    }

    // Check the block range method given the various combinations of the limit and the end block.
    let n_commited_block_number = BlockNumber(n_committed);
    let n_verified_block_number = BlockNumber(n_verified);
    let test_vector = vec![
        (n_commited_block_number, n_committed),
        (n_verified_block_number, n_verified),
        (n_commited_block_number, n_verified),
        (n_verified_block_number, 1),
        (n_commited_block_number, 1),
        (n_commited_block_number, 0),
        (n_commited_block_number, 100),
    ];

    for (max_block, limit) in test_vector {
        check_block_range(&mut storage, max_block, limit).await?;
    }

    Ok(())
}

/// Checks the correctness of the processing of committed unconfirmed transactions.
#[db_test]
async fn unconfirmed_transaction(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Below lies the initialization of the data for the test.

    let mut rng = create_rng();

    // Required since we use `EthereumSchema` in this test.
    EthereumSchema(&mut storage).initialize_eth_data().await?;

    let mut accounts_map = AccountMap::default();

    let n_committed = 5;
    let n_commited_confirmed = 3;
    let n_verified = 2;

    // Create and apply several blocks to work with.
    for block_number in 1..=n_committed {
        let block_number = BlockNumber(block_number);
        let (new_accounts_map, updates) = apply_random_updates(accounts_map.clone(), &mut rng);
        accounts_map = new_accounts_map;

        // Store the block in the block schema.
        BlockSchema(&mut storage)
            .save_block(gen_sample_block(
                block_number,
                BLOCK_SIZE_CHUNKS,
                Default::default(),
            ))
            .await?;
        OperationsSchema(&mut storage)
            .store_aggregated_action(gen_unique_aggregated_operation(
                block_number,
                AggregatedActionType::CommitBlocks,
                BLOCK_SIZE_CHUNKS,
            ))
            .await?;
        let (id, op) = OperationsSchema(&mut storage)
            .get_aggregated_op_that_affects_block(AggregatedActionType::CommitBlocks, block_number)
            .await?
            .unwrap();
        StateSchema(&mut storage)
            .commit_state_update(block_number, &updates, 0)
            .await?;

        // Store & confirm the operation in the ethereum schema, as it's used for obtaining
        // commit/verify hashes.
        let eth_tx_hash = dummy_ethereum_tx_hash(id);
        let response = EthereumSchema(&mut storage)
            .save_new_eth_tx(
                AggregatedActionType::CommitBlocks,
                Some((id, op)),
                100,
                100u32.into(),
                Default::default(),
            )
            .await?;
        EthereumSchema(&mut storage)
            .add_hash_entry(response.id, &eth_tx_hash)
            .await?;

        if *block_number <= n_commited_confirmed {
            EthereumSchema(&mut storage)
                .confirm_eth_tx(&eth_tx_hash)
                .await?;
        }

        // Add verification for the block if required.
        if *block_number <= n_verified {
            OperationsSchema(&mut storage)
                .store_aggregated_action(gen_unique_aggregated_operation(
                    block_number,
                    AggregatedActionType::ExecuteBlocks,
                    BLOCK_SIZE_CHUNKS,
                ))
                .await?;
            let (id, op) = OperationsSchema(&mut storage)
                .get_aggregated_op_that_affects_block(
                    AggregatedActionType::ExecuteBlocks,
                    block_number,
                )
                .await?
                .unwrap();

            let eth_tx_hash = dummy_ethereum_tx_hash(id);
            let response = EthereumSchema(&mut storage)
                .save_new_eth_tx(
                    AggregatedActionType::ExecuteBlocks,
                    Some((id, op)),
                    100,
                    100u32.into(),
                    Default::default(),
                )
                .await?;
            EthereumSchema(&mut storage)
                .add_hash_entry(response.id, &eth_tx_hash)
                .await?;
            EthereumSchema(&mut storage)
                .confirm_eth_tx(&eth_tx_hash)
                .await?;
        }
    }

    assert!(BlockSchema(&mut storage)
        .find_block_by_height_or_hash(n_commited_confirmed.to_string())
        .await
        .is_some());

    assert!(BlockSchema(&mut storage)
        .find_block_by_height_or_hash((n_commited_confirmed + 1).to_string())
        .await
        .is_none());

    let block_range = BlockSchema(&mut storage)
        .load_block_range(BlockNumber(n_committed), 100)
        .await?;

    assert_eq!(block_range.len(), n_commited_confirmed as usize);

    Ok(())
}

/// Checks the pending block workflow:
/// - Transactions from the pending block are available for getting.
/// - `load_pending_block` loads the block correctly.
/// - Committing the final block causes pending block to be removed.
#[db_test]
async fn pending_block_workflow(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    use crate::chain::operations_ext::OperationsExtSchema;
    use zksync_test_account::ZkSyncAccount;
    use zksync_types::{
        block::PendingBlock,
        operations::{ChangePubKeyOp, TransferToNewOp},
        ExecutedOperations, ExecutedTx, ZkSyncOp, ZkSyncTx,
    };
    let _sentry_guard = vlog::init();

    let from_account_id = AccountId(0xbabe);
    let from_zksync_account = ZkSyncAccount::rand();
    from_zksync_account.set_account_id(Some(from_account_id));

    let to_account_id = AccountId(0xdcba);
    let to_zksync_account = ZkSyncAccount::rand();
    to_zksync_account.set_account_id(Some(to_account_id));

    let (tx_1, executed_tx_1) = {
        let tx = from_zksync_account.sign_change_pubkey_tx(
            None,
            false,
            TokenId(0),
            Default::default(),
            ChangePubKeyType::ECDSA,
            Default::default(),
        );

        let change_pubkey_op = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
            tx: tx.clone(),
            account_id: from_account_id,
        }));

        let executed_change_pubkey_op = ExecutedTx {
            signed_tx: change_pubkey_op.try_get_tx().unwrap().into(),
            success: true,
            op: Some(change_pubkey_op),
            fail_reason: None,
            block_index: None,
            created_at: chrono::Utc::now(),
            batch_id: None,
        };

        (
            ZkSyncTx::ChangePubKey(Box::new(tx)),
            ExecutedOperations::Tx(Box::new(executed_change_pubkey_op)),
        )
    };
    let (tx_2, executed_tx_2) = {
        let tx = from_zksync_account
            .sign_transfer(
                TokenId(0),
                "",
                1u32.into(),
                0u32.into(),
                &to_zksync_account.address,
                None,
                true,
                Default::default(),
            )
            .0;

        let transfer_to_new_op = ZkSyncOp::TransferToNew(Box::new(TransferToNewOp {
            tx: tx.clone(),
            from: from_account_id,
            to: to_account_id,
        }));

        let executed_transfer_to_new_op = ExecutedTx {
            signed_tx: transfer_to_new_op.try_get_tx().unwrap().into(),
            success: true,
            op: Some(transfer_to_new_op),
            fail_reason: None,
            block_index: None,
            created_at: chrono::Utc::now(),
            batch_id: None,
        };

        (
            ZkSyncTx::Transfer(Box::new(tx)),
            ExecutedOperations::Tx(Box::new(executed_transfer_to_new_op)),
        )
    };

    let txs_1 = vec![executed_tx_1];
    let txs_2 = vec![executed_tx_2];

    let block_1 = gen_sample_block(BlockNumber(1), BLOCK_SIZE_CHUNKS, txs_1.clone());
    let block_2 = gen_sample_block(BlockNumber(2), BLOCK_SIZE_CHUNKS, txs_2.clone());

    let pending_block_1 = PendingBlock {
        number: BlockNumber(1),
        chunks_left: 10,
        unprocessed_priority_op_before: 0,
        pending_block_iteration: 1,
        success_operations: txs_1,
        failed_txs: Vec::new(),
        previous_block_root_hash: H256::default(),
        timestamp: 0,
    };
    let pending_block_2 = PendingBlock {
        number: BlockNumber(2),
        chunks_left: 12,
        unprocessed_priority_op_before: 0,
        pending_block_iteration: 2,
        success_operations: txs_2,
        failed_txs: Vec::new(),
        previous_block_root_hash: H256::default(),
        timestamp: 0,
    };

    // Save pending block
    BlockSchema(&mut storage)
        .save_pending_block(pending_block_1.clone())
        .await?;

    // Load saved block and check its correctness.
    let pending_block = BlockSchema(&mut storage)
        .load_pending_block()
        .await?
        .expect("No pending block");
    assert_eq!(pending_block.number, pending_block_1.number);
    assert_eq!(pending_block.chunks_left, pending_block_1.chunks_left);
    assert_eq!(
        pending_block.unprocessed_priority_op_before,
        pending_block_1.unprocessed_priority_op_before
    );
    assert_eq!(
        pending_block.pending_block_iteration,
        pending_block_1.pending_block_iteration
    );
    assert_eq!(
        pending_block.success_operations.len(),
        pending_block_1.success_operations.len()
    );

    // Check that stored tx can already be loaded from the database.
    let pending_ops = BlockSchema(&mut storage)
        .get_block_executed_ops(BlockNumber(1))
        .await?;
    assert_eq!(pending_ops.len(), 1);

    // Also check that we can find the transaction by its hash.
    assert!(
        OperationsExtSchema(&mut storage)
            .get_tx_by_hash(&tx_1.hash().as_ref())
            .await?
            .is_some(),
        "Cannot find the pending transaction by hash"
    );

    // Finalize the block.
    BlockSchema(&mut storage).save_block(block_1).await?;

    // Ensure that pending block is no more available.
    assert!(
        BlockSchema(&mut storage)
            .load_pending_block()
            .await?
            .is_none(),
        "Pending block was not removed after commit"
    );

    // Repeat the checks with the second block. Now we'll check for
    // both committed (1st) and pending (2nd) blocks data to be available.
    BlockSchema(&mut storage)
        .save_pending_block(pending_block_2.clone())
        .await?;

    let pending_block = BlockSchema(&mut storage)
        .load_pending_block()
        .await?
        .expect("No pending block");
    assert_eq!(pending_block.number, pending_block_2.number);

    // Check that stored tx can already be loaded from the database.
    let committed_ops = BlockSchema(&mut storage)
        .get_block_executed_ops(BlockNumber(1))
        .await?;
    assert_eq!(committed_ops.len(), 1);
    let pending_ops = BlockSchema(&mut storage)
        .get_block_executed_ops(BlockNumber(2))
        .await?;
    assert_eq!(pending_ops.len(), 1);

    // Also check that we can find the transaction by its hash.
    assert!(
        OperationsExtSchema(&mut storage)
            .get_tx_by_hash(&tx_1.hash().as_ref())
            .await?
            .is_some(),
        "Cannot find the pending transaction by hash"
    );
    assert!(
        OperationsExtSchema(&mut storage)
            .get_tx_by_hash(&tx_2.hash().as_ref())
            .await?
            .is_some(),
        "Cannot find the pending transaction by hash"
    );

    // Finalize the block.
    BlockSchema(&mut storage).save_block(block_2).await?;

    // Ensure that pending block is no more available.
    assert!(
        BlockSchema(&mut storage)
            .load_pending_block()
            .await?
            .is_none(),
        "Pending block was not removed after commit"
    );

    Ok(())
}

/// Check that operations are counted correctly.
#[db_test]
async fn test_operations_counter(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Expect no operations stored.
    assert_eq!(
        storage
            .chain()
            .block_schema()
            .count_aggregated_operations(AggregatedActionType::CommitBlocks, false)
            .await?,
        0
    );
    assert_eq!(
        storage
            .chain()
            .block_schema()
            .count_aggregated_operations(AggregatedActionType::ExecuteBlocks, false)
            .await?,
        0
    );
    assert_eq!(
        storage
            .chain()
            .block_schema()
            .count_aggregated_operations(AggregatedActionType::CommitBlocks, true)
            .await?,
        0
    );
    assert_eq!(
        storage
            .chain()
            .block_schema()
            .count_aggregated_operations(AggregatedActionType::ExecuteBlocks, true)
            .await?,
        0
    );

    // Store new operations.
    for (block_number, action_type) in &[
        (BlockNumber(1), AggregatedActionType::CommitBlocks),
        (BlockNumber(2), AggregatedActionType::CommitBlocks),
        (BlockNumber(3), AggregatedActionType::CommitBlocks),
        (BlockNumber(4), AggregatedActionType::CommitBlocks),
        (BlockNumber(1), AggregatedActionType::ExecuteBlocks),
        (BlockNumber(2), AggregatedActionType::ExecuteBlocks),
    ] {
        storage
            .chain()
            .operations_schema()
            .store_aggregated_action(gen_unique_aggregated_operation(
                *block_number,
                *action_type,
                BLOCK_SIZE_CHUNKS,
            ))
            .await?;
    }

    // Set all of them confirmed except one.
    for (block_number, action_type) in &[
        (BlockNumber(1), AggregatedActionType::CommitBlocks),
        (BlockNumber(2), AggregatedActionType::CommitBlocks),
        (BlockNumber(3), AggregatedActionType::CommitBlocks),
        (BlockNumber(1), AggregatedActionType::ExecuteBlocks),
        (BlockNumber(2), AggregatedActionType::ExecuteBlocks),
    ] {
        storage
            .chain()
            .operations_schema()
            .confirm_aggregated_operations(*block_number, *block_number, *action_type)
            .await?;
    }

    // We have one unconfirmed COMMIT operation, the rest is confirmed.
    assert_eq!(
        storage
            .chain()
            .block_schema()
            .count_aggregated_operations(AggregatedActionType::CommitBlocks, false)
            .await?,
        1
    );
    assert_eq!(
        storage
            .chain()
            .block_schema()
            .count_aggregated_operations(AggregatedActionType::ExecuteBlocks, false)
            .await?,
        0
    );
    assert_eq!(
        storage
            .chain()
            .block_schema()
            .count_aggregated_operations(AggregatedActionType::CommitBlocks, true)
            .await?,
        3
    );
    assert_eq!(
        storage
            .chain()
            .block_schema()
            .count_aggregated_operations(AggregatedActionType::ExecuteBlocks, true)
            .await?,
        2
    );

    Ok(())
}

/// Check that blocks are removed correctly.
#[db_test]
async fn test_remove_blocks(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Insert 5 blocks.
    for block_number in 1..=5 {
        BlockSchema(&mut storage)
            .save_block(gen_sample_block(
                BlockNumber(block_number),
                BLOCK_SIZE_CHUNKS,
                Default::default(),
            ))
            .await?;
        OperationsSchema(&mut storage)
            .store_aggregated_action(gen_unique_aggregated_operation(
                BlockNumber(block_number),
                AggregatedActionType::CommitBlocks,
                BLOCK_SIZE_CHUNKS,
            ))
            .await?;
    }
    // Remove blocks with numbers greater than 2.
    BlockSchema(&mut storage)
        .remove_blocks(BlockNumber(2))
        .await?;

    // Check if the 2nd block is present, and the 3rd is not.
    assert!(BlockSchema(&mut storage)
        .get_block(BlockNumber(2))
        .await?
        .is_some());
    assert!(BlockSchema(&mut storage)
        .get_block(BlockNumber(3))
        .await?
        .is_none());

    Ok(())
}

/// Check that blocks are removed correctly.
#[db_test]
async fn test_remove_pending_block(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    use zksync_types::block::PendingBlock;

    let pending_block_1 = PendingBlock {
        number: BlockNumber(3),
        chunks_left: 10,
        unprocessed_priority_op_before: 0,
        pending_block_iteration: 1,
        success_operations: Vec::new(),
        failed_txs: Vec::new(),
        previous_block_root_hash: H256::default(),
        timestamp: 0,
    };

    BlockSchema(&mut storage)
        .save_pending_block(pending_block_1.clone())
        .await?;
    BlockSchema(&mut storage).remove_pending_block().await?;
    assert!(!BlockSchema(&mut storage).pending_block_exists().await?);

    Ok(())
}

/// Check that account tree cache is removed correctly.
#[db_test]
async fn test_remove_account_tree_cache(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Insert account tree cache for 5 blocks.
    for block_number in 1..=5 {
        BlockSchema(&mut storage)
            .save_block(gen_sample_block(
                BlockNumber(block_number),
                BLOCK_SIZE_CHUNKS,
                Default::default(),
            ))
            .await?;
        BlockSchema(&mut storage)
            .store_account_tree_cache(BlockNumber(block_number), serde_json::Value::default())
            .await?;
    }

    // Remove account tree cache for blocks with numbers greater than 2.
    BlockSchema(&mut storage)
        .remove_account_tree_cache(BlockNumber(2))
        .await?;

    // Check if account tree cache for the 2nd block is present, and for the 3rd is not.
    assert!(BlockSchema(&mut storage)
        .get_account_tree_cache_block(BlockNumber(2))
        .await?
        .is_some());
    assert!(BlockSchema(&mut storage)
        .get_account_tree_cache_block(BlockNumber(3))
        .await?
        .is_none());

    Ok(())
}
