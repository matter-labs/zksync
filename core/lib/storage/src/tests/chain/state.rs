// External imports
// Workspace imports
use zksync_types::{helpers::apply_updates, AccountMap, Action, ActionType};
// Local imports
use super::{block::apply_random_updates, utils::get_operation};
use crate::tests::{create_rng, db_test};
use crate::{
    chain::{
        block::BlockSchema,
        operations::{records::NewOperation, OperationsSchema},
        state::StateSchema,
    },
    prover::ProverSchema,
    QueryResult, StorageProcessor,
};

/// Performs low-level checks for the state workflow.
/// Here we avoid using `BlockSchema` to perform operations, and instead modify state and
/// operations tables manually just to check `commit_state_update` / `apply_state_update`
/// methods. It means that not all the tables are updated, and, for example,
/// `load_committed_state(None)` will not work (since this method will attempt to
/// look into `blocks` table to get the most recent block number.)
#[db_test]
async fn low_level_commit_verify_state(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut rng = create_rng();

    // Create the input data for three blocks.
    // Data for the next block is based on previous block data.
    let (accounts_block_1, updates_block_1) = apply_random_updates(AccountMap::default(), &mut rng);
    let (accounts_block_2, updates_block_2) =
        apply_random_updates(accounts_block_1.clone(), &mut rng);
    let (accounts_block_3, updates_block_3) =
        apply_random_updates(accounts_block_2.clone(), &mut rng);

    // Store the states in schema.
    StateSchema(&mut storage)
        .commit_state_update(1, &updates_block_1, 0)
        .await?;
    StateSchema(&mut storage)
        .commit_state_update(2, &updates_block_2, 0)
        .await?;
    StateSchema(&mut storage)
        .commit_state_update(3, &updates_block_3, 0)
        .await?;

    // We have to store the operations as well (and for verify below too).
    for block_number in 1..=3 {
        OperationsSchema(&mut storage)
            .store_operation(NewOperation {
                block_number,
                action_type: ActionType::COMMIT.to_string(),
            })
            .await?;
    }

    // Check that they are stored in state.
    let (block, state) = StateSchema(&mut storage)
        .load_committed_state(Some(1))
        .await?;
    assert_eq!((block, &state), (1, &accounts_block_1));

    let (block, state) = StateSchema(&mut storage)
        .load_committed_state(Some(2))
        .await?;
    assert_eq!((block, &state), (2, &accounts_block_2));

    let (block, state) = StateSchema(&mut storage)
        .load_committed_state(Some(3))
        .await?;
    assert_eq!((block, &state), (3, &accounts_block_3));

    // Apply one state.
    StateSchema(&mut storage).apply_state_update(1).await?;
    OperationsSchema(&mut storage)
        .store_operation(NewOperation {
            block_number: 1,
            action_type: ActionType::VERIFY.to_string(),
        })
        .await?;
    OperationsSchema(&mut storage)
        .confirm_operation(1, ActionType::VERIFY)
        .await?;

    // Check that the verified state is now equals to the committed state.
    let committed_1 = StateSchema(&mut storage)
        .load_committed_state(Some(1))
        .await?;
    let verified_1 = StateSchema(&mut storage).load_verified_state().await?;
    assert_eq!(committed_1, verified_1);

    // Apply the rest of states and check that `load_verified_state` updates as well.
    StateSchema(&mut storage).apply_state_update(2).await?;
    OperationsSchema(&mut storage)
        .store_operation(NewOperation {
            block_number: 2,
            action_type: ActionType::VERIFY.to_string(),
        })
        .await?;
    OperationsSchema(&mut storage)
        .confirm_operation(2, ActionType::VERIFY)
        .await?;
    let committed_2 = StateSchema(&mut storage)
        .load_committed_state(Some(2))
        .await?;
    let verified_2 = StateSchema(&mut storage).load_verified_state().await?;
    assert_eq!(verified_2, committed_2);

    StateSchema(&mut storage).apply_state_update(3).await?;
    OperationsSchema(&mut storage)
        .store_operation(NewOperation {
            block_number: 3,
            action_type: ActionType::VERIFY.to_string(),
        })
        .await?;
    OperationsSchema(&mut storage)
        .confirm_operation(3, ActionType::VERIFY)
        .await?;
    let committed_3 = StateSchema(&mut storage)
        .load_committed_state(Some(3))
        .await?;
    let verified_3 = StateSchema(&mut storage).load_verified_state().await?;
    assert_eq!(verified_3, committed_3);

    Ok(())
}

#[db_test]
async fn state_diff(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    async fn check_diff_applying(
        storage: &mut StorageProcessor<'_>,
        start_block: u32,
        end_block: Option<u32>,
    ) -> QueryResult<()> {
        let (block, updates) = StateSchema(storage)
            .load_state_diff(start_block, end_block)
            .await?
            .expect("Can't load the diff");
        if let Some(end_block) = end_block {
            assert_eq!(end_block, block);
        }
        let (_, expected_state) = StateSchema(storage).load_committed_state(end_block).await?;
        let (_, mut obtained_state) = StateSchema(storage)
            .load_committed_state(Some(start_block))
            .await?;
        apply_updates(&mut obtained_state, updates);
        assert_eq!(
            obtained_state, expected_state,
            "Applying diff {} -> {:?} failed",
            start_block, end_block
        );
        Ok(())
    }

    let mut rng = create_rng();

    let block_size = 100;
    let mut accounts_map = AccountMap::default();
    let blocks_amount = 5;

    // Create and apply several blocks to work with.
    for block_number in 1..=blocks_amount {
        let (new_accounts_map, updates) = apply_random_updates(accounts_map.clone(), &mut rng);
        accounts_map = new_accounts_map;

        BlockSchema(&mut storage)
            .execute_operation(get_operation(block_number, Action::Commit, block_size))
            .await?;
        StateSchema(&mut storage)
            .commit_state_update(block_number, &updates, 0)
            .await?;

        ProverSchema(&mut storage)
            .store_proof(block_number, &Default::default())
            .await?;
        BlockSchema(&mut storage)
            .execute_operation(get_operation(
                block_number,
                Action::Verify {
                    proof: Default::default(),
                },
                block_size,
            ))
            .await?;
    }

    // Now let's load some diffs and apply them.
    check_diff_applying(&mut storage, 1, Some(2)).await?;
    check_diff_applying(&mut storage, 2, Some(3)).await?;
    check_diff_applying(&mut storage, 1, Some(3)).await?;

    // Go in the reverse order.
    check_diff_applying(&mut storage, 2, Some(1)).await?;
    check_diff_applying(&mut storage, 3, Some(1)).await?;

    // Apply diff with uncertain end target.
    check_diff_applying(&mut storage, 1, None).await?;

    Ok(())
}
