//! API testing helpers.

// Built-in uses

// External uses
use actix_web::{web, App, Scope};
use chrono::Utc;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_crypto::rand::{SeedableRng, XorShiftRng};
use zksync_storage::{
    chain::operations::records::NewExecutedPriorityOperation,
    chain::operations::OperationsSchema,
    prover::ProverSchema,
    test_data::{
        dummy_ethereum_tx_hash, gen_acc_random_updates, gen_sample_block,
        gen_unique_aggregated_operation_with_txs, get_sample_aggregated_proof,
        get_sample_single_proof, BLOCK_SIZE_CHUNKS,
    },
    ConnectionPool,
};
use zksync_test_account::ZkSyncAccount;
use zksync_types::{
    aggregated_operations::AggregatedActionType,
    helpers::{apply_updates, closest_packable_fee_amount, closest_packable_token_amount},
    operations::{ChangePubKeyOp, TransferToNewOp},
    prover::ProverJobType,
    tx::ChangePubKeyType,
    AccountId, AccountMap, Address, BlockNumber, Deposit, DepositOp, ExecutedOperations,
    ExecutedPriorityOp, ExecutedTx, FullExit, FullExitOp, MintNFTOp, Nonce, PriorityOp, Token,
    TokenId, Transfer, TransferOp, ZkSyncOp, ZkSyncTx, H256,
};

// Local uses
use super::Client;
use std::str::FromStr;
use zksync_storage::test_data::generate_nft;

/// Serial ID of the verified priority operation.
pub const VERIFIED_OP_SERIAL_ID: u64 = 10;
/// Serial ID of the committed priority operation.
pub const COMMITTED_OP_SERIAL_ID: u64 = 243;
/// Number of committed blocks.
pub const COMMITTED_BLOCKS_COUNT: u32 = 8;
/// Number of verified blocks.
pub const VERIFIED_BLOCKS_COUNT: u32 = 5;
/// Number of executed blocks.
pub const EXECUTED_BLOCKS_COUNT: u32 = 3;

#[derive(Debug, Clone)]
pub struct TestServerConfig {
    pub config: ZkSyncConfig,
    pub pool: ConnectionPool,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            config: ZkSyncConfig::from_env(),
            pool: ConnectionPool::new(Some(1)),
        }
    }
}

#[derive(Debug)]
pub struct TestTransactions {
    pub acc: ZkSyncAccount,
    pub txs: Vec<(ZkSyncTx, ExecutedOperations)>,
}

impl TestServerConfig {
    pub fn start_server_with_scope<F>(
        &self,
        scope: String,
        scope_factory: F,
    ) -> (Client, actix_web::test::TestServer)
    where
        F: Fn(&TestServerConfig) -> Scope + Clone + Send + 'static,
    {
        let this = self.clone();
        let server = actix_web::test::start(move || {
            App::new().service(web::scope(scope.as_ref()).service(scope_factory(&this)))
        });

        let url = server.url("").trim_end_matches('/').to_owned();

        let client = Client::new(url);
        (client, server)
    }

    pub fn start_server<F>(&self, scope_factory: F) -> (Client, actix_web::test::TestServer)
    where
        F: Fn(&TestServerConfig) -> Scope + Clone + Send + 'static,
    {
        self.start_server_with_scope(String::from("/api/v1"), scope_factory)
    }

    /// Creates several transactions and the corresponding executed operations.
    pub fn gen_zk_txs(fee: u64) -> TestTransactions {
        Self::gen_zk_txs_for_account(AccountId(0xdead), ZkSyncAccount::rand().address, fee)
    }

    /// Creates several transactions and the corresponding executed operations for the
    /// specified account.
    pub fn gen_zk_txs_for_account(
        account_id: AccountId,
        address: Address,
        fee: u64,
    ) -> TestTransactions {
        let from = ZkSyncAccount::rand();
        from.set_account_id(Some(AccountId(0xf00d)));

        let mut to = ZkSyncAccount::rand();
        to.set_account_id(Some(account_id));
        to.address = address;

        let mut txs = Vec::new();

        // Sign change pubkey tx pair
        {
            let tx = from.sign_change_pubkey_tx(
                None,
                false,
                TokenId(0),
                fee.into(),
                ChangePubKeyType::ECDSA,
                Default::default(),
            );

            let zksync_op = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
                tx: tx.clone(),
                account_id: from.get_account_id().unwrap(),
            }));

            let executed_tx = ExecutedTx {
                signed_tx: zksync_op.try_get_tx().unwrap().into(),
                success: true,
                op: Some(zksync_op),
                fail_reason: None,
                block_index: Some(1),
                created_at: chrono::Utc::now(),
                batch_id: None,
            };

            txs.push((
                ZkSyncTx::ChangePubKey(Box::new(tx)),
                ExecutedOperations::Tx(Box::new(executed_tx)),
            ));
        }
        // Transfer tx pair
        {
            let tx = from
                .sign_transfer(
                    TokenId(0),
                    "ETH",
                    closest_packable_token_amount(&10_u64.into()),
                    closest_packable_fee_amount(&fee.into()),
                    &to.address,
                    None,
                    false,
                    Default::default(),
                )
                .0;

            let zksync_op = ZkSyncOp::TransferToNew(Box::new(TransferToNewOp {
                tx: tx.clone(),
                from: from.get_account_id().unwrap(),
                to: to.get_account_id().unwrap(),
            }));

            let executed_tx = ExecutedTx {
                signed_tx: zksync_op.try_get_tx().unwrap().into(),
                success: true,
                op: Some(zksync_op),
                fail_reason: None,
                block_index: Some(2),
                created_at: chrono::Utc::now(),
                batch_id: None,
            };

            txs.push((
                ZkSyncTx::Transfer(Box::new(tx)),
                ExecutedOperations::Tx(Box::new(executed_tx)),
            ));
        }
        // Failed transfer tx pair
        {
            let tx = from
                .sign_transfer(
                    TokenId(0),
                    "GLM",
                    1_u64.into(),
                    fee.into(),
                    &to.address,
                    None,
                    false,
                    Default::default(),
                )
                .0;

            let zksync_op = ZkSyncOp::TransferToNew(Box::new(TransferToNewOp {
                tx: tx.clone(),
                from: from.get_account_id().unwrap(),
                to: to.get_account_id().unwrap(),
            }));

            let executed_tx = ExecutedTx {
                signed_tx: zksync_op.try_get_tx().unwrap().into(),
                success: false,
                op: Some(zksync_op),
                fail_reason: Some("Unknown token".to_string()),
                block_index: None,
                created_at: chrono::Utc::now(),
                batch_id: None,
            };

            txs.push((
                ZkSyncTx::Transfer(Box::new(tx)),
                ExecutedOperations::Tx(Box::new(executed_tx)),
            ));
        }
        // Transfer back tx pair
        {
            let tx = Transfer::new(
                to.get_account_id().unwrap(),
                to.address,
                from.address,
                TokenId(0),
                2_u64.into(),
                fee.into(),
                Nonce(0),
                Default::default(),
                None,
            );

            let zksync_op = ZkSyncOp::Transfer(Box::new(TransferOp {
                tx: tx.clone(),
                from: to.get_account_id().unwrap(),
                to: from.get_account_id().unwrap(),
            }));

            let executed_tx = ExecutedTx {
                signed_tx: zksync_op.try_get_tx().unwrap().into(),
                success: true,
                op: Some(zksync_op),
                fail_reason: None,
                block_index: Some(3),
                created_at: chrono::Utc::now(),
                batch_id: None,
            };

            txs.push((
                ZkSyncTx::Transfer(Box::new(tx)),
                ExecutedOperations::Tx(Box::new(executed_tx)),
            ));
        }

        // Mint NFT
        {
            let tx = from
                .sign_mint_nft(
                    TokenId(0),
                    "ETH",
                    H256::random(),
                    closest_packable_fee_amount(&fee.into()),
                    &to.address,
                    None,
                    true,
                )
                .0;

            let zksync_op = ZkSyncOp::MintNFTOp(Box::new(MintNFTOp {
                tx: tx.clone(),
                creator_account_id: from.get_account_id().unwrap(),
                recipient_account_id: to.get_account_id().unwrap(),
            }));

            let executed_tx = ExecutedTx {
                signed_tx: zksync_op.try_get_tx().unwrap().into(),
                success: true,
                op: Some(zksync_op),
                fail_reason: None,
                block_index: Some(4),
                created_at: chrono::Utc::now(),
                batch_id: None,
            };

            txs.push((
                ZkSyncTx::MintNFT(Box::new(tx)),
                ExecutedOperations::Tx(Box::new(executed_tx)),
            ));
        }
        TestTransactions { acc: from, txs }
    }

    pub async fn fill_database(&self) -> anyhow::Result<()> {
        static INITED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

        // Hold this guard until transaction will be committed to avoid double init.
        let mut inited_guard = INITED.lock().await;
        if *inited_guard {
            return Ok(());
        }
        *inited_guard = true;

        let mut storage = self.pool.access_storage().await?;

        // Check if database is been already inited.
        if storage
            .chain()
            .block_schema()
            .get_block(BlockNumber(1))
            .await?
            .is_some()
        {
            return Ok(());
        }

        // Make changes atomic.
        let mut storage = storage.start_transaction().await?;

        // Below lies the initialization of the data for the test.
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);

        // Required since we use `EthereumSchema` in this test.
        storage.ethereum_schema().initialize_eth_data().await?;

        // Insert PHNX token
        storage
            .tokens_schema()
            .store_or_update_token(Token::new(
                TokenId(1),
                Address::from_str("38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7").unwrap(),
                "PHNX",
                18,
            ))
            .await?;
        // Insert Golem token with old symbol (from rinkeby).
        storage
            .tokens_schema()
            .store_or_update_token(Token::new(
                TokenId(16),
                Address::from_str("d94e3dc39d4cad1dad634e7eb585a57a19dc7efe").unwrap(),
                "GNT",
                18,
            ))
            .await?;

        let mut accounts = AccountMap::default();

        // Create and apply several blocks to work with.
        for block_number in 1..=COMMITTED_BLOCKS_COUNT {
            let block_number = BlockNumber(block_number);
            let mut updates = (0..3)
                .flat_map(|_| gen_acc_random_updates(&mut rng))
                .collect::<Vec<_>>();

            accounts
                .iter()
                .enumerate()
                .for_each(|(id, (account_id, account))| {
                    updates.append(&mut generate_nft(
                        *account_id,
                        account,
                        block_number.0 * accounts.len() as u32 + id as u32,
                    ));
                });
            apply_updates(&mut accounts, updates.clone());

            // Add transactions to every odd block.
            let txs = if *block_number % 2 == 1 {
                let (&id, account) = accounts.iter().next().unwrap();

                Self::gen_zk_txs_for_account(id, account.address, 1_000)
                    .txs
                    .into_iter()
                    .map(|(_tx, op)| op)
                    .collect()
            } else {
                vec![]
            };

            storage
                .chain()
                .block_schema()
                .save_block(gen_sample_block(
                    block_number,
                    BLOCK_SIZE_CHUNKS,
                    txs.clone(),
                ))
                .await?;
            storage
                .chain()
                .state_schema()
                .commit_state_update(block_number, &updates, 0)
                .await?;

            // Store & confirm the operation in the ethereum schema, as it's used for obtaining
            // commit/verify/execute hashes.
            let aggregated_operation = gen_unique_aggregated_operation_with_txs(
                block_number,
                AggregatedActionType::CommitBlocks,
                BLOCK_SIZE_CHUNKS,
                txs.clone(),
            );
            OperationsSchema(&mut storage)
                .store_aggregated_action(aggregated_operation)
                .await?;
            let (id, op) = OperationsSchema(&mut storage)
                .get_aggregated_op_that_affects_block(
                    AggregatedActionType::CommitBlocks,
                    block_number,
                )
                .await?
                .unwrap();

            // Store the Ethereum transaction.
            let eth_tx_hash = dummy_ethereum_tx_hash(id);
            let response = storage
                .ethereum_schema()
                .save_new_eth_tx(
                    AggregatedActionType::CommitBlocks,
                    Some((id, op)),
                    100,
                    100u32.into(),
                    Default::default(),
                )
                .await?;
            storage
                .ethereum_schema()
                .add_hash_entry(response.id, &eth_tx_hash)
                .await?;
            storage
                .ethereum_schema()
                .confirm_eth_tx(&eth_tx_hash)
                .await?;

            // Add verification for the block if required.
            if *block_number <= VERIFIED_BLOCKS_COUNT {
                // Add jobs to `job_prover_queue`.
                let job_data = serde_json::Value::default();
                ProverSchema(&mut storage)
                    .add_prover_job_to_job_queue(
                        block_number,
                        block_number,
                        job_data.clone(),
                        0,
                        ProverJobType::SingleProof,
                    )
                    .await?;
                ProverSchema(&mut storage)
                    .add_prover_job_to_job_queue(
                        block_number,
                        block_number,
                        job_data,
                        1,
                        ProverJobType::AggregatedProof,
                    )
                    .await?;

                // Get job id.
                let stored_job_id = ProverSchema(&mut storage)
                    .get_idle_prover_job_from_job_queue()
                    .await?
                    .unwrap()
                    .job_id;
                let stored_aggregated_job_id = ProverSchema(&mut storage)
                    .get_idle_prover_job_from_job_queue()
                    .await?
                    .unwrap()
                    .job_id;

                // Store proofs.
                let proof = get_sample_single_proof();
                let aggregated_proof = get_sample_aggregated_proof();
                ProverSchema(&mut storage)
                    .store_proof(stored_job_id, block_number, &proof)
                    .await?;
                ProverSchema(&mut storage)
                    .store_aggregated_proof(
                        stored_aggregated_job_id,
                        block_number,
                        block_number,
                        &aggregated_proof,
                    )
                    .await?;

                let aggregated_operation = gen_unique_aggregated_operation_with_txs(
                    block_number,
                    AggregatedActionType::PublishProofBlocksOnchain,
                    BLOCK_SIZE_CHUNKS,
                    txs.clone(),
                );
                OperationsSchema(&mut storage)
                    .store_aggregated_action(aggregated_operation)
                    .await?;
                let (id, op) = OperationsSchema(&mut storage)
                    .get_aggregated_op_that_affects_block(
                        AggregatedActionType::PublishProofBlocksOnchain,
                        block_number,
                    )
                    .await?
                    .unwrap();

                let response = storage
                    .ethereum_schema()
                    .save_new_eth_tx(
                        AggregatedActionType::PublishProofBlocksOnchain,
                        Some((id, op)),
                        100,
                        100u32.into(),
                        Default::default(),
                    )
                    .await?;
                let eth_tx_hash = dummy_ethereum_tx_hash(id);
                storage
                    .ethereum_schema()
                    .add_hash_entry(response.id, &eth_tx_hash)
                    .await?;
                storage
                    .ethereum_schema()
                    .confirm_eth_tx(&eth_tx_hash)
                    .await?;
            }

            if *block_number <= EXECUTED_BLOCKS_COUNT {
                let aggregated_operation = gen_unique_aggregated_operation_with_txs(
                    block_number,
                    AggregatedActionType::ExecuteBlocks,
                    BLOCK_SIZE_CHUNKS,
                    txs.clone(),
                );
                OperationsSchema(&mut storage)
                    .store_aggregated_action(aggregated_operation)
                    .await?;
                let (id, op) = OperationsSchema(&mut storage)
                    .get_aggregated_op_that_affects_block(
                        AggregatedActionType::ExecuteBlocks,
                        block_number,
                    )
                    .await?
                    .unwrap();

                // Store the Ethereum transaction.
                let eth_tx_hash = dummy_ethereum_tx_hash(id);
                let response = storage
                    .ethereum_schema()
                    .save_new_eth_tx(
                        AggregatedActionType::ExecuteBlocks,
                        Some((id, op)),
                        100,
                        100u32.into(),
                        Default::default(),
                    )
                    .await?;
                storage
                    .ethereum_schema()
                    .add_hash_entry(response.id, &eth_tx_hash)
                    .await?;
                storage
                    .ethereum_schema()
                    .confirm_eth_tx(&eth_tx_hash)
                    .await?;
            }
        }

        // Store priority operations for some tests.
        let ops = vec![
            // Verified priority operation.
            NewExecutedPriorityOperation {
                block_number: 2,
                block_index: 2,
                operation: serde_json::to_value(
                    dummy_deposit_op(Address::default(), AccountId(1), VERIFIED_OP_SERIAL_ID, 2).op,
                )
                .unwrap(),
                from_account: Default::default(),
                to_account: Default::default(),
                priority_op_serialid: VERIFIED_OP_SERIAL_ID as i64,
                deadline_block: 100,
                eth_hash: dummy_ethereum_tx_hash(VERIFIED_OP_SERIAL_ID as i64)
                    .as_bytes()
                    .to_vec(),
                eth_block: 10,
                created_at: chrono::Utc::now(),
            },
            // Committed priority operation.
            NewExecutedPriorityOperation {
                block_number: EXECUTED_BLOCKS_COUNT as i64 + 1,
                block_index: 1,
                operation: serde_json::to_value(
                    dummy_full_exit_op(AccountId(1), Address::default(), COMMITTED_OP_SERIAL_ID, 3)
                        .op,
                )
                .unwrap(),
                from_account: Default::default(),
                to_account: Default::default(),
                priority_op_serialid: COMMITTED_OP_SERIAL_ID as i64,
                deadline_block: 200,
                eth_hash: dummy_ethereum_tx_hash(COMMITTED_OP_SERIAL_ID as i64)
                    .as_bytes()
                    .to_vec(),
                eth_block: 14,
                created_at: chrono::Utc::now(),
            },
        ];

        for op in ops {
            storage
                .chain()
                .operations_schema()
                .store_executed_priority_op(op)
                .await?;
        }

        // Get the accounts by their IDs.
        for (account_id, _account) in accounts {
            let account_state = storage
                .chain()
                .account_schema()
                .account_state_by_id(account_id)
                .await?;

            // Check that committed state is available.
            assert!(
                account_state.committed.is_some(),
                "No committed state for account"
            );
        }

        storage.commit().await?;
        // Storage has been inited, so we can safely drop this guard.
        drop(inited_guard);

        Ok(())
    }
}

/// Creates dummy deposit priority operation.
pub fn dummy_deposit_op(
    address: Address,
    account_id: AccountId,
    serial_id: u64,
    block_index: u32,
) -> ExecutedPriorityOp {
    let deposit_op = ZkSyncOp::Deposit(Box::new(DepositOp {
        priority_op: Deposit {
            from: address,
            token: TokenId(0),
            amount: 1_u64.into(),
            to: address,
        },
        account_id,
    }));

    ExecutedPriorityOp {
        priority_op: PriorityOp {
            serial_id,
            data: deposit_op.try_get_priority_op().unwrap(),
            deadline_block: 0,
            eth_hash: H256::default(),
            eth_block: 10,
        },
        op: deposit_op,
        block_index,
        created_at: Utc::now(),
    }
}

/// Creates dummy full exit priority operation.
pub fn dummy_full_exit_op(
    account_id: AccountId,
    eth_address: Address,
    serial_id: u64,
    block_index: u32,
) -> ExecutedPriorityOp {
    let deposit_op = ZkSyncOp::FullExit(Box::new(FullExitOp {
        priority_op: FullExit {
            account_id,
            eth_address,
            token: TokenId(0),
        },
        withdraw_amount: None,
        creator_account_id: None,
        creator_address: None,
        serial_id: None,
        content_hash: None,
    }));

    ExecutedPriorityOp {
        priority_op: PriorityOp {
            serial_id,
            data: deposit_op.try_get_priority_op().unwrap(),
            deadline_block: 0,
            eth_hash: H256::default(),
            eth_block: 10,
        },
        op: deposit_op,
        block_index,
        created_at: Utc::now(),
    }
}
