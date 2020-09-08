// External imports
// Workspace imports
use models::{
    node::{apply_updates, AccountMap},
    Action, ActionType,
};
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
    StorageProcessor,
};

/// Performs low-level checks for the state workflow.
/// Here we avoid using `BlockSchema` to perform operations, and instead modify state and
/// operations tables manually just to check `commit_state_update` / `apply_state_update`
/// methods. It means that not all the tables are updated, and, for example,
/// `load_committed_state(None)` will not work (since this method will attempt to
/// look into `blocks` table to get the most recent block number.)
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn low_level_commit_verify_state() {
    let mut rng = create_rng();

    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Create the input data for three blocks.
        // Data for the next block is based on previous block data.
        let (accounts_block_1, updates_block_1) =
            apply_random_updates(AccountMap::default(), &mut rng);
        let (accounts_block_2, updates_block_2) =
            apply_random_updates(accounts_block_1.clone(), &mut rng);
        let (accounts_block_3, updates_block_3) =
            apply_random_updates(accounts_block_2.clone(), &mut rng);

        // Store the states in schema.
        StateSchema(&conn).commit_state_update(1, &updates_block_1)?;
        StateSchema(&conn).commit_state_update(2, &updates_block_2)?;
        StateSchema(&conn).commit_state_update(3, &updates_block_3)?;

        // We have to store the operations as well (and for verify below too).
        for block_number in 1..=3 {
            OperationsSchema(&conn).store_operation(NewOperation {
                block_number,
                action_type: ActionType::COMMIT.to_string(),
            })?;
        }

        // Check that they are stored in state.
        let (block, state) = StateSchema(&conn).load_committed_state(Some(1)).unwrap();
        assert_eq!((block, &state), (1, &accounts_block_1));

        let (block, state) = StateSchema(&conn).load_committed_state(Some(2)).unwrap();
        assert_eq!((block, &state), (2, &accounts_block_2));

        let (block, state) = StateSchema(&conn).load_committed_state(Some(3)).unwrap();
        assert_eq!((block, &state), (3, &accounts_block_3));

        // Apply one state.
        StateSchema(&conn).apply_state_update(1)?;
        OperationsSchema(&conn).store_operation(NewOperation {
            block_number: 1,
            action_type: ActionType::VERIFY.to_string(),
        })?;

        // Check that the verified state is now equals to the committed state.
        let committed_1 = StateSchema(&conn).load_committed_state(Some(1)).unwrap();
        let verified_1 = StateSchema(&conn).load_verified_state().unwrap();
        assert_eq!(committed_1, verified_1);

        // Apply the rest of states and check that `load_verified_state` updates as well.
        StateSchema(&conn).apply_state_update(2)?;
        OperationsSchema(&conn).store_operation(NewOperation {
            block_number: 2,
            action_type: ActionType::VERIFY.to_string(),
        })?;
        let committed_2 = StateSchema(&conn).load_committed_state(Some(2)).unwrap();
        let verified_2 = StateSchema(&conn).load_verified_state().unwrap();
        assert_eq!(verified_2, committed_2);

        StateSchema(&conn).apply_state_update(3)?;
        OperationsSchema(&conn).store_operation(NewOperation {
            block_number: 3,
            action_type: ActionType::VERIFY.to_string(),
        })?;
        let committed_3 = StateSchema(&conn).load_committed_state(Some(3)).unwrap();
        let verified_3 = StateSchema(&conn).load_verified_state().unwrap();
        assert_eq!(verified_3, committed_3);

        Ok(())
    });
}

#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn state_diff() {
    fn check_diff_applying(
        conn: &StorageProcessor,
        start_block: u32,
        end_block: Option<u32>,
    ) -> diesel::QueryResult<()> {
        let (block, updates) = StateSchema(conn)
            .load_state_diff(start_block, end_block)?
            .expect("Can't load the diff");
        if let Some(end_block) = end_block {
            assert_eq!(end_block, block);
        }
        let (_, expected_state) = StateSchema(conn).load_committed_state(end_block)?;
        let (_, mut obtained_state) = StateSchema(conn).load_committed_state(Some(start_block))?;
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
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        let mut accounts_map = AccountMap::default();
        let blocks_amount = 5;

        // Create and apply several blocks to work with.
        for block_number in 1..=blocks_amount {
            let (new_accounts_map, updates) = apply_random_updates(accounts_map.clone(), &mut rng);
            accounts_map = new_accounts_map;

            BlockSchema(&conn).execute_operation(get_operation(
                block_number,
                Action::Commit,
                updates,
                block_size,
            ))?;

            ProverSchema(&conn).store_proof(block_number, &Default::default())?;
            BlockSchema(&conn).execute_operation(get_operation(
                block_number,
                Action::Verify {
                    proof: Default::default(),
                },
                Vec::new(),
                block_size,
            ))?;
        }

        // Now let's load some diffs and apply them.
        check_diff_applying(&conn, 1, Some(2))?;
        check_diff_applying(&conn, 2, Some(3))?;
        check_diff_applying(&conn, 1, Some(3))?;

        // Go in the reverse order.
        check_diff_applying(&conn, 2, Some(1))?;
        check_diff_applying(&conn, 3, Some(1))?;

        // Apply diff with uncertain end target.
        check_diff_applying(&conn, 1, None)?;

        Ok(())
    });
}
