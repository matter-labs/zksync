//! API testing helpers.

// Built-in uses
use std::str::FromStr;

// External uses
use actix_web::{web, App, Scope};
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

// Workspace uses
use zksync_config::{ApiServerOptions, ConfigurationOptions};
use zksync_crypto::rand::{SeedableRng, XorShiftRng};
use zksync_storage::ConnectionPool;
use zksync_storage::{
    chain::operations::records::NewExecutedPriorityOperation,
    test_data::{
        dummy_ethereum_tx_hash, gen_acc_random_updates, gen_unique_operation,
        gen_unique_operation_with_txs, BLOCK_SIZE_CHUNKS,
    },
};
use zksync_test_account::ZkSyncAccount;
use zksync_types::{
    ethereum::OperationType, helpers::apply_updates, AccountMap, Action, BlockNumber, H256,
};
use zksync_types::{
    operations::{ChangePubKeyOp, TransferToNewOp},
    Address, ExecutedOperations, ExecutedTx, Token, ZkSyncOp, ZkSyncTx,
};

// Local uses
use super::client::Client;

/// Serial ID of the verified priority operation.
pub const VERIFIED_OP_SERIAL_ID: u64 = 10;
/// Serial ID of the committed priority operation.
pub const COMMITTED_OP_SERIAL_ID: u64 = 243;
/// Number of committed blocks.
pub const COMMITTED_BLOCKS_COUNT: BlockNumber = 8;
/// Number of verified blocks.
pub const VERIFIED_BLOCKS_COUNT: BlockNumber = 3;

#[derive(Debug, Clone)]
pub struct TestServerConfig {
    pub env_options: ConfigurationOptions,
    pub api_server_options: ApiServerOptions,
    pub pool: ConnectionPool,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            env_options: ConfigurationOptions::from_env(),
            api_server_options: ApiServerOptions::from_env(),
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
    pub fn start_server<F>(&self, scope_factory: F) -> (Client, actix_web::test::TestServer)
    where
        F: Fn(&TestServerConfig) -> Scope + Clone + Send + 'static,
    {
        let this = self.clone();
        let server = actix_web::test::start(move || {
            App::new().service(web::scope("/api/v1").service(scope_factory(&this)))
        });

        let mut url = server.url("");
        url.pop(); // Pop last '/' symbol.

        let client = Client::new(url);
        (client, server)
    }

    /// Creates several transactions and the corresponding executed operations.
    pub fn gen_zk_txs(fee: u64) -> TestTransactions {
        let from = ZkSyncAccount::rand();
        from.set_account_id(Some(0xdead));

        let to = ZkSyncAccount::rand();
        to.set_account_id(Some(0xf00d));

        let mut txs = Vec::new();

        // Sign change pubkey tx pair
        {
            let tx = from.sign_change_pubkey_tx(None, false, 0, fee.into(), false);

            let zksync_op = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
                tx: tx.clone(),
                account_id: from.get_account_id().unwrap(),
            }));

            let executed_tx = ExecutedTx {
                signed_tx: zksync_op.try_get_tx().unwrap().into(),
                success: true,
                op: Some(zksync_op),
                fail_reason: None,
                block_index: None,
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
                .sign_transfer(0, "ETH", 1_u64.into(), fee.into(), &to.address, None, false)
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
                block_index: None,
                created_at: chrono::Utc::now(),
                batch_id: None,
            };

            txs.push((
                ZkSyncTx::Transfer(Box::new(tx)),
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
        if storage.chain().block_schema().get_block(1).await?.is_some() {
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
            .store_token(Token::new(
                1,
                Address::from_str("38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7").unwrap(),
                "PHNX",
                18,
            ))
            .await?;
        // Insert Golem token with old symbol (from rinkeby).
        storage
            .tokens_schema()
            .store_token(Token::new(
                16,
                Address::from_str("d94e3dc39d4cad1dad634e7eb585a57a19dc7efe").unwrap(),
                "GNT",
                18,
            ))
            .await?;

        let mut accounts = AccountMap::default();

        // Create and apply several blocks to work with.
        for block_number in 1..=COMMITTED_BLOCKS_COUNT {
            let updates = (0..3)
                .map(|_| gen_acc_random_updates(&mut rng))
                .flatten()
                .collect::<Vec<_>>();
            apply_updates(&mut accounts, updates.clone());

            // Add transactions to every odd block.
            let txs = if block_number % 2 == 1 {
                Self::gen_zk_txs(1_000)
                    .txs
                    .into_iter()
                    .map(|(_tx, op)| op)
                    .collect()
            } else {
                vec![]
            };

            // Store the operation in the block schema.
            let operation = storage
                .chain()
                .block_schema()
                .execute_operation(gen_unique_operation_with_txs(
                    block_number,
                    Action::Commit,
                    BLOCK_SIZE_CHUNKS,
                    txs,
                ))
                .await?;
            storage
                .chain()
                .state_schema()
                .commit_state_update(block_number, &updates, 0)
                .await?;

            // Store & confirm the operation in the ethereum schema, as it's used for obtaining
            // commit/verify hashes.
            let ethereum_op_id = operation.id.unwrap() as i64;
            let eth_tx_hash = dummy_ethereum_tx_hash(ethereum_op_id);
            let response = storage
                .ethereum_schema()
                .save_new_eth_tx(
                    OperationType::Commit,
                    Some(ethereum_op_id),
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
            if block_number <= VERIFIED_BLOCKS_COUNT {
                storage
                    .prover_schema()
                    .store_proof(block_number, &Default::default())
                    .await?;
                let operation = storage
                    .chain()
                    .block_schema()
                    .execute_operation(gen_unique_operation(
                        block_number,
                        Action::Verify {
                            proof: Default::default(),
                        },
                        BLOCK_SIZE_CHUNKS,
                    ))
                    .await?;

                let ethereum_op_id = operation.id.unwrap() as i64;
                let eth_tx_hash = dummy_ethereum_tx_hash(ethereum_op_id);
                let response = storage
                    .ethereum_schema()
                    .save_new_eth_tx(
                        OperationType::Verify,
                        Some(ethereum_op_id),
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
                operation: Default::default(),
                from_account: Default::default(),
                to_account: Default::default(),
                priority_op_serialid: VERIFIED_OP_SERIAL_ID as i64,
                deadline_block: 100,
                eth_hash: H256::default().as_bytes().to_vec(),
                eth_block: 10,
                created_at: chrono::Utc::now(),
            },
            // Committed priority operation.
            NewExecutedPriorityOperation {
                block_number: VERIFIED_BLOCKS_COUNT as i64 + 1,
                block_index: 1,
                operation: Default::default(),
                from_account: Default::default(),
                to_account: Default::default(),
                priority_op_serialid: COMMITTED_OP_SERIAL_ID as i64,
                deadline_block: 200,
                eth_hash: H256::default().as_bytes().to_vec(),
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

        storage.commit().await?;
        // Storage has been inited, so we can safely drop this guard.
        drop(inited_guard);

        Ok(())
    }
}
