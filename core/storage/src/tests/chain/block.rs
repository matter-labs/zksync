// External imports
use web3::types::H256;
// Workspace imports
use crypto_exports::rand::XorShiftRng;
use models::node::{apply_updates, block::Block, AccountMap, AccountUpdate, BlockNumber, Fr};
use models::{Action, Operation};
// Local imports
use super::utils::{acc_create_random_updates, get_operation};
use crate::tests::{create_rng, db_test};
use crate::{
    chain::{
        block::{records::BlockDetails, BlockSchema},
        state::StateSchema,
    },
    ethereum::EthereumSchema,
    prover::ProverSchema,
    StorageProcessor,
};

/// Creates several random updates for the provided account map,
/// and returns the resulting account map together with the list
/// of generated updates.
pub fn apply_random_updates(
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

/// Here we create updates for blocks 1,2,3 (commit 3 blocks)
/// We apply updates for blocks 1,2 (verify 2 blocks)
/// Make sure that we can get state for all blocks.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
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

/// Creates an unique new root hash for the block based on its number.
fn root_hash_for_block(block_number: BlockNumber) -> Fr {
    Fr::from_hex(format!("{:064x}", block_number).as_ref()).unwrap()
}

/// Creates an unique ethereum operation hash based on its number.
fn ethereum_tx_hash(ethereum_op_id: i64) -> H256 {
    H256::from_low_u64_ne(ethereum_op_id as u64)
}

/// Creates an operation with an unique hash.
fn get_unique_operation(
    block_number: BlockNumber,
    action: Action,
    accounts_updated: Vec<(u32, AccountUpdate)>,
) -> Operation {
    Operation {
        id: None,
        action,
        block: Block {
            block_number,
            new_root_hash: root_hash_for_block(block_number),
            fee_account: 0,
            block_transactions: Vec::new(),
            processed_priority_ops: (0, 0),
        },
        accounts_updated,
    }
}

/*
/// Checks that `find_block_by_height_or_hash` method allows
/// to load the block details by either its height, hash of the included
/// transaction, or the root hash of the block.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn find_block_by_height_or_hash() {
    let mut rng = create_rng();

    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        let mut accounts_map = AccountMap::default();
        let n_committed = 5;
        let n_verified = n_committed - 2;

        let mut expected_outcome: Vec<BlockDetails> = Vec::new();

        // Create and apply several blocks to work with.
        for block_number in 1..=n_committed {
            // Create blanked block detail object which we will fill
            // with the relevant data and use for the comparison later.
            let mut current_block_detail = BlockDetails {
                block_number: 0,
                new_state_root: Default::default(),
                block_size: 0,
                commit_tx_hash: None,
                verify_tx_hash: None,
                committed_at: chrono::NaiveDateTime::from_timestamp(0, 0),
                verified_at: None,
            };

            let (new_accounts_map, updates) = apply_random_updates(accounts_map.clone(), &mut rng);
            accounts_map = new_accounts_map;

            let operation = BlockSchema(&conn).execute_operation(get_unique_operation(
                block_number,
                Action::Commit,
                updates,
            ))?;

            let ethereum_op_id = operation.id.unwrap() as i64;
            let eth_tx_hash = ethereum_tx_hash(ethereum_op_id);
            EthereumSchema(&conn).save_operation_eth_tx(
                ethereum_op_id,
                eth_tx_hash,
                100,
                100,
                100.into(),
                Default::default(),
            )?;
            EthereumSchema(&conn).confirm_eth_tx(&eth_tx_hash)?;

            current_block_detail.block_number = operation.block.block_number as i64;
            current_block_detail.new_state_root = operation.block.new_root_hash.to_hex();
            current_block_detail.block_size = operation.block.block_transactions.len() as i64;
            current_block_detail.commit_tx_hash = Some(eth_tx_hash.to_string());

            if block_number <= n_verified {
                ProverSchema(&conn).store_proof(block_number, &Default::default())?;
                let verify_operation =
                    BlockSchema(&conn).execute_operation(get_unique_operation(
                        block_number,
                        Action::Verify {
                            proof: Default::default(),
                        },
                        Vec::new(),
                    ))?;

                let ethereum_op_id = verify_operation.id.unwrap() as i64;
                let eth_tx_hash = ethereum_tx_hash(ethereum_op_id);
                EthereumSchema(&conn).save_operation_eth_tx(
                    ethereum_op_id,
                    eth_tx_hash,
                    100,
                    100,
                    100.into(),
                    Default::default(),
                )?;

                // Do not add an ethereum confirmation for the last operation.
                if block_number != n_verified {
                    EthereumSchema(&conn).confirm_eth_tx(&eth_tx_hash)?;
                    current_block_detail.verify_tx_hash = Some(eth_tx_hash.to_string());
                }
            }

            expected_outcome.push(current_block_detail);
        }

        for expected_block_detail in expected_outcome {
            let mut queries = vec![
                expected_block_detail.block_number.to_string(),
                expected_block_detail.new_state_root.clone(),
                expected_block_detail
                    .commit_tx_hash
                    .as_ref()
                    .unwrap()
                    .clone(),
            ];
            if let Some(verify_tx_hash) = expected_block_detail.verify_tx_hash.as_ref() {
                queries.push(verify_tx_hash.clone());
            }

            for query in queries {
                let actual_block_detail = BlockSchema(&conn)
                    .find_block_by_height_or_hash(query.clone())
                    .unwrap_or_else(|| {
                        panic!(format!(
                            "Can't load the existing block with the index {} using query {}",
                            expected_block_detail.block_number, query
                        ))
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
        }

        Ok(())
    });
}
*/

/// Checks that `load_block_range` method loads the range of blocks correctly.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn block_range() {
    /// Loads the block range and checks that every block in the response is
    /// equal to the one obtained from `find_block_by_height_or_hash` method.
    fn check_block_range(
        conn: &StorageProcessor,
        expected_outcome: &[BlockDetails],
        max_block: BlockNumber,
        limit: u32,
    ) -> diesel::QueryResult<()> {
        let start_block = if max_block >= limit {
            (max_block - limit) + 1
        } else {
            1
        };
        let block_range = BlockSchema(conn).load_block_range(max_block, limit)?;
        // Go in the reversed order, since the blocks themselves are ordered backwards.
        for (idx, block_number) in (start_block..=max_block).rev().enumerate() {
            let got = &block_range[idx];
            let expected = &expected_outcome[block_number as usize - 1];
            assert_eq!(got.block_number, expected.block_number);
            assert_eq!(got.new_state_root, expected.new_state_root);
            assert_eq!(got.commit_tx_hash, expected.commit_tx_hash);
            assert_eq!(got.verify_tx_hash, expected.verify_tx_hash);
        }

        Ok(())
    }

    let mut rng = create_rng();

    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        let mut accounts_map = AccountMap::default();
        let n_committed = 5;
        let n_verified = n_committed - 2;

        let mut expected_outcome: Vec<BlockDetails> = Vec::new();

        // Create and apply several blocks to work with.
        for block_number in 1..=n_committed {
            // Create blanked block detail object which we will fill
            // with the relevant data and use for the comparison later.
            let mut current_block_detail = BlockDetails {
                block_number: 0,
                new_state_root: Default::default(),
                block_size: 0,
                commit_tx_hash: None,
                verify_tx_hash: None,
                committed_at: chrono::NaiveDateTime::from_timestamp(0, 0),
                verified_at: None,
            };

            let (new_accounts_map, updates) = apply_random_updates(accounts_map.clone(), &mut rng);
            accounts_map = new_accounts_map;

            let operation = BlockSchema(&conn).execute_operation(get_unique_operation(
                block_number,
                Action::Commit,
                updates,
            ))?;
            let ethereum_op_id = operation.id.unwrap() as i64;
            let eth_tx_hash = ethereum_tx_hash(ethereum_op_id);
            EthereumSchema(&conn).save_operation_eth_tx(
                ethereum_op_id,
                eth_tx_hash,
                100,
                100,
                100.into(),
                Default::default(),
            )?;

            current_block_detail.block_number = operation.block.block_number as i64;
            current_block_detail.new_state_root =
                format!("sync-bl:{}", operation.block.new_root_hash.to_hex());
            current_block_detail.block_size = operation.block.block_transactions.len() as i64;
            current_block_detail.commit_tx_hash = Some(format!("0x{}", hex::encode(eth_tx_hash)));

            if block_number <= n_verified {
                ProverSchema(&conn).store_proof(block_number, &Default::default())?;
                let operation = BlockSchema(&conn).execute_operation(get_unique_operation(
                    block_number,
                    Action::Verify {
                        proof: Default::default(),
                    },
                    Vec::new(),
                ))?;
                let ethereum_op_id = operation.id.unwrap() as i64;
                let eth_tx_hash = ethereum_tx_hash(ethereum_op_id);
                EthereumSchema(&conn).save_operation_eth_tx(
                    ethereum_op_id,
                    eth_tx_hash,
                    100,
                    100,
                    100.into(),
                    Default::default(),
                )?;
                EthereumSchema(&conn).confirm_eth_tx(&eth_tx_hash)?;
                current_block_detail.verify_tx_hash =
                    Some(format!("0x{}", hex::encode(eth_tx_hash)));
            }

            expected_outcome.push(current_block_detail);
        }

        // Check the block range method given the various combinations of the limit and the end block.
        let test_vector = vec![
            (n_committed as BlockNumber, n_committed),
            (n_verified as BlockNumber, n_verified),
            (n_committed as BlockNumber, n_verified),
            (n_verified as BlockNumber, 1),
            (n_committed as BlockNumber, 1),
            (n_committed as BlockNumber, 0),
            (n_committed as BlockNumber, 100),
        ];

        for (max_block, limit) in test_vector {
            check_block_range(&conn, &expected_outcome, max_block, limit)?;
        }

        Ok(())
    });
}
