// External imports
use diesel::Connection;
// Workspace imports
use crypto_exports::rand::{SeedableRng, XorShiftRng};
use models::node::{apply_updates, AccountMap};
use models::Action;
// Local imports
use self::utils::{acc_create_random_updates, get_operation};
use crate::{
    chain::{block::BlockSchema, state::StateSchema},
    prover::ProverSchema,
    ConnectionPool,
};

mod utils;

// Here we create updates for blocks 1,2,3 (commit 3 blocks)
// We apply updates for blocks 1,2 (verify 2 blocks)
// Make sure that we can get state for all blocks.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn test_commit_rewind() {
    let _ = env_logger::try_init();

    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);

    let pool = ConnectionPool::new();
    let conn = pool.access_storage().unwrap();
    conn.conn().begin_test_transaction().unwrap(); // this will revert db after test

    let (accounts_block_1, updates_block_1) = {
        let mut accounts = AccountMap::default();
        let updates = {
            let mut updates = Vec::new();
            updates.extend(acc_create_random_updates(&mut rng));
            updates.extend(acc_create_random_updates(&mut rng));
            updates.extend(acc_create_random_updates(&mut rng));
            updates
        };
        apply_updates(&mut accounts, updates.clone());
        (accounts, updates)
    };

    let (accounts_block_2, updates_block_2) = {
        let mut accounts = accounts_block_1.clone();
        let updates = {
            let mut updates = Vec::new();
            updates.extend(acc_create_random_updates(&mut rng));
            updates.extend(acc_create_random_updates(&mut rng));
            updates.extend(acc_create_random_updates(&mut rng));
            updates
        };
        apply_updates(&mut accounts, updates.clone());
        (accounts, updates)
    };
    let (accounts_block_3, updates_block_3) = {
        let mut accounts = accounts_block_2.clone();
        let updates = {
            let mut updates = Vec::new();
            updates.extend(acc_create_random_updates(&mut rng));
            updates.extend(acc_create_random_updates(&mut rng));
            updates.extend(acc_create_random_updates(&mut rng));
            updates
        };
        apply_updates(&mut accounts, updates.clone());
        (accounts, updates)
    };

    BlockSchema(&conn)
        .execute_operation(get_operation(1, Action::Commit, updates_block_1))
        .expect("Commit block 1");
    BlockSchema(&conn)
        .execute_operation(get_operation(2, Action::Commit, updates_block_2))
        .expect("Commit block 2");
    BlockSchema(&conn)
        .execute_operation(get_operation(3, Action::Commit, updates_block_3))
        .expect("Commit block 3");

    let (block, state) = StateSchema(&conn).load_committed_state(Some(1)).unwrap();
    assert_eq!((block, &state), (1, &accounts_block_1));

    let (block, state) = StateSchema(&conn).load_committed_state(Some(2)).unwrap();
    assert_eq!((block, &state), (2, &accounts_block_2));

    let (block, state) = StateSchema(&conn).load_committed_state(Some(3)).unwrap();
    assert_eq!((block, &state), (3, &accounts_block_3));

    ProverSchema(&conn)
        .store_proof(1, &Default::default())
        .expect("Store proof block 1");
    BlockSchema(&conn)
        .execute_operation(get_operation(
            1,
            Action::Verify {
                proof: Default::default(),
            },
            Vec::new(),
        ))
        .expect("Verify block 1");
    ProverSchema(&conn)
        .store_proof(2, &Default::default())
        .expect("Store proof block 2");
    BlockSchema(&conn)
        .execute_operation(get_operation(
            2,
            Action::Verify {
                proof: Default::default(),
            },
            Vec::new(),
        ))
        .expect("Verify block 2");

    let (block, state) = StateSchema(&conn).load_committed_state(Some(1)).unwrap();
    assert_eq!((block, &state), (1, &accounts_block_1));

    let (block, state) = StateSchema(&conn).load_committed_state(Some(2)).unwrap();
    assert_eq!((block, &state), (2, &accounts_block_2));

    let (block, state) = StateSchema(&conn).load_committed_state(Some(3)).unwrap();
    assert_eq!((block, &state), (3, &accounts_block_3));

    let (block, state) = StateSchema(&conn).load_committed_state(None).unwrap();
    assert_eq!((block, &state), (3, &accounts_block_3));
}
