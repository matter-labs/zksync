// External imports
// Workspace imports
use crypto_exports::rand::XorShiftRng;
use models::node::{apply_updates, AccountMap, AccountUpdate};
use models::Action;
// Local imports
use self::utils::{acc_create_random_updates, get_operation};
use crate::tests::{create_rng, db_test};
use crate::{
    chain::{block::BlockSchema, state::StateSchema},
    prover::ProverSchema,
    StorageProcessor,
};

mod operations;
pub mod utils;

/// Creates several random updates for the provided account map,
/// and returns the resulting account map together with the list
/// of generated updates.
fn apply_random_updates(
    mut accounts: AccountMap,
    rng: &mut XorShiftRng,
) -> (AccountMap, Vec<(u32, AccountUpdate)>) {
    let updates = {
        let mut updates = Vec::new();
        updates.extend(acc_create_random_updates(rng));
        updates.extend(acc_create_random_updates(rng));
        updates.extend(acc_create_random_updates(rng));
        updates
    };
    apply_updates(&mut accounts, updates.clone());
    (accounts, updates)
}

// Here we create updates for blocks 1,2,3 (commit 3 blocks)
// We apply updates for blocks 1,2 (verify 2 blocks)
// Make sure that we can get state for all blocks.
#[test]
fn test_commit_rewind() {
    let _ = env_logger::try_init();
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

        // Execute and commit these blocks.
        BlockSchema(&conn).execute_operation(get_operation(1, Action::Commit, updates_block_1))?;
        BlockSchema(&conn).execute_operation(get_operation(2, Action::Commit, updates_block_2))?;
        BlockSchema(&conn).execute_operation(get_operation(3, Action::Commit, updates_block_3))?;

        // Check that they are stored in state.
        let (block, state) = StateSchema(&conn).load_committed_state(Some(1)).unwrap();
        assert_eq!((block, &state), (1, &accounts_block_1));

        let (block, state) = StateSchema(&conn).load_committed_state(Some(2)).unwrap();
        assert_eq!((block, &state), (2, &accounts_block_2));

        let (block, state) = StateSchema(&conn).load_committed_state(Some(3)).unwrap();
        assert_eq!((block, &state), (3, &accounts_block_3));

        // Add proofs for the first two blocks.
        ProverSchema(&conn).store_proof(1, &Default::default())?;
        BlockSchema(&conn).execute_operation(get_operation(
            1,
            Action::Verify {
                proof: Default::default(),
            },
            Vec::new(),
        ))?;
        ProverSchema(&conn).store_proof(2, &Default::default())?;
        BlockSchema(&conn).execute_operation(get_operation(
            2,
            Action::Verify {
                proof: Default::default(),
            },
            Vec::new(),
        ))?;

        // Check that we still can get the state for these blocks.
        let (block, state) = StateSchema(&conn).load_committed_state(Some(1)).unwrap();
        assert_eq!((block, &state), (1, &accounts_block_1));

        let (block, state) = StateSchema(&conn).load_committed_state(Some(2)).unwrap();
        assert_eq!((block, &state), (2, &accounts_block_2));

        let (block, state) = StateSchema(&conn).load_committed_state(Some(3)).unwrap();
        assert_eq!((block, &state), (3, &accounts_block_3));

        // Check that with no id provided, the latest state is loaded.
        let (block, state) = StateSchema(&conn).load_committed_state(None).unwrap();
        assert_eq!((block, &state), (3, &accounts_block_3));

        Ok(())
    });
}
