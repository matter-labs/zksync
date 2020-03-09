// These tests require empty DB setup and ignored by default
// use `zksync db-test-no-reset`/`franklin db-test` script to run them

use super::*;

// External imports
use diesel::Connection;
use web3::types::Address;
// Workspace imports
use crypto_exports::rand::{Rng, SeedableRng, XorShiftRng};
use models::node::block::Block;
use models::node::{apply_updates, AccountMap, AccountUpdate, Fr, PubKeyHash};
use models::primitives::u128_to_bigdecimal;
use models::{Action, EncodedProof, Operation};
// Local imports
use crate::interfaces::{block::BlockInterface, state::StateInterface};

fn acc_create_random_updates<R: Rng>(rng: &mut R) -> impl Iterator<Item = (u32, AccountUpdate)> {
    let id: u32 = rng.gen();
    let balance = u128::from(rng.gen::<u64>());
    let nonce: u32 = rng.gen();
    let pub_key_hash = PubKeyHash { data: rng.gen() };
    let address: Address = rng.gen::<[u8; 20]>().into();

    let mut a = models::node::account::Account::default_with_address(&address);
    let old_nonce = nonce;
    a.nonce = old_nonce + 2;
    a.pub_key_hash = pub_key_hash;

    let old_balance = a.get_balance(0);
    a.set_balance(0, u128_to_bigdecimal(balance));
    let new_balance = a.get_balance(0);
    vec![
        (
            id,
            AccountUpdate::Create {
                nonce: old_nonce,
                address: a.address,
            },
        ),
        (
            id,
            AccountUpdate::ChangePubKeyHash {
                old_nonce,
                old_pub_key_hash: PubKeyHash::default(),
                new_nonce: old_nonce + 1,
                new_pub_key_hash: a.pub_key_hash,
            },
        ),
        (
            id,
            AccountUpdate::UpdateBalance {
                old_nonce: old_nonce + 1,
                new_nonce: old_nonce + 2,
                balance_update: (0, old_balance, new_balance),
            },
        ),
    ]
    .into_iter()
}

#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
// Here we create updates for blocks 1,2,3 (commit 3 blocks)
// We apply updates for blocks 1,2 (verify 2 blocks)
// Make sure that we can get state for all blocks.
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

    let get_operation = |block_number, action, accounts_updated| -> Operation {
        Operation {
            id: None,
            action,
            block: Block {
                block_number,
                new_root_hash: Fr::default(),
                fee_account: 0,
                block_transactions: Vec::new(),
                processed_priority_ops: (0, 0),
            },
            accounts_updated,
        }
    };

    conn.execute_operation(&get_operation(1, Action::Commit, updates_block_1))
        .expect("Commit block 1");
    conn.execute_operation(&get_operation(2, Action::Commit, updates_block_2))
        .expect("Commit block 2");
    conn.execute_operation(&get_operation(3, Action::Commit, updates_block_3))
        .expect("Commit block 3");

    let (block, state) = conn.load_committed_state(Some(1)).unwrap();
    assert_eq!((block, &state), (1, &accounts_block_1));

    let (block, state) = conn.load_committed_state(Some(2)).unwrap();
    assert_eq!((block, &state), (2, &accounts_block_2));

    let (block, state) = conn.load_committed_state(Some(3)).unwrap();
    assert_eq!((block, &state), (3, &accounts_block_3));

    conn.store_proof(1, &Default::default())
        .expect("Store proof block 1");
    conn.execute_operation(&get_operation(
        1,
        Action::Verify {
            proof: Default::default(),
        },
        Vec::new(),
    ))
    .expect("Verify block 1");
    conn.store_proof(2, &Default::default())
        .expect("Store proof block 2");
    conn.execute_operation(&get_operation(
        2,
        Action::Verify {
            proof: Default::default(),
        },
        Vec::new(),
    ))
    .expect("Verify block 2");

    let (block, state) = conn.load_committed_state(Some(1)).unwrap();
    assert_eq!((block, &state), (1, &accounts_block_1));

    let (block, state) = conn.load_committed_state(Some(2)).unwrap();
    assert_eq!((block, &state), (2, &accounts_block_2));

    let (block, state) = conn.load_committed_state(Some(3)).unwrap();
    assert_eq!((block, &state), (3, &accounts_block_3));

    let (block, state) = conn.load_committed_state(None).unwrap();
    assert_eq!((block, &state), (3, &accounts_block_3));
}

#[test]
#[ignore]
// TODO: Implement
fn test_eth_sender_storage() {}

#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn test_store_proof() {
    let pool = ConnectionPool::new();
    let conn = pool.access_storage().unwrap();
    conn.conn().begin_test_transaction().unwrap(); // this will revert db after test

    assert!(conn.load_proof(1).is_err());

    let proof = EncodedProof::default();
    assert!(conn.store_proof(1, &proof).is_ok());

    let loaded = conn.load_proof(1).expect("must load proof");
    assert_eq!(loaded, proof);
}
