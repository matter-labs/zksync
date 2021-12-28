//! API testing helpers.

// Built-in uses
use std::collections::HashMap;
use std::str::FromStr;

// External uses
use actix_web::{web, App, Scope};
use anyhow::Error;
use bigdecimal::{BigDecimal, Zero};
use chrono::Utc;
use futures::{channel::mpsc, StreamExt};
use num::{rational::Ratio, BigUint};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

// Workspace uses
use zksync_api_client::rest::client::Client;
use zksync_api_types::v02::Response;
use zksync_config::ZkSyncConfig;
use zksync_crypto::rand::{Rng, SeedableRng, XorShiftRng};
use zksync_storage::{
    chain::operations::records::NewExecutedPriorityOperation,
    chain::operations::OperationsSchema,
    prover::ProverSchema,
    test_data::{
        dummy_ethereum_tx_hash, gen_acc_random_updates, gen_sample_block,
        gen_unique_aggregated_operation_with_txs, generate_nft, get_sample_aggregated_proof,
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
    AccountId, AccountMap, AccountUpdate, Address, BlockNumber, Deposit, DepositOp,
    ExecutedOperations, ExecutedPriorityOp, ExecutedTx, FullExit, FullExitOp, MintNFTOp, Nonce,
    PriorityOp, Token, TokenId, TokenKind, TokenLike, TokenPrice, Transfer, TransferOp, ZkSyncOp,
    ZkSyncTx, H256, NFT,
};
use zksync_utils::{big_decimal_to_ratio, scaled_u64_to_ratio, UnsignedRatioSerializeAsDecimal};

// Local uses
use crate::fee_ticker::{
    tests::TestToken,
    ticker_info::BlocksInFutureAggregatedOperations,
    validator::{cache::TokenInMemoryCache, FeeTokenValidator},
    {FeeTicker, FeeTickerInfo, GasOperationsCost, PriceError, TickerConfig},
};
use crate::signature_checker::{VerifiedTx, VerifySignatureRequest};
use std::any::Any;

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
    pub fn start_server_with_scope<F, D>(
        &self,
        scope: String,
        scope_factory: F,
        shared_data: Option<D>,
    ) -> (Client, actix_test::TestServer)
    where
        F: Fn(&TestServerConfig) -> Scope + Clone + Send + 'static,
        D: Clone + Send + 'static,
    {
        let this = self.clone();

        let server = actix_test::start(move || {
            let app = App::new();
            let shared_data = shared_data.clone();
            let app = if let Some(shared_data) = shared_data {
                app.app_data(web::Data::new(shared_data))
            } else {
                app
            };
            app.service(web::scope(scope.as_ref()).service(scope_factory(&this)))
        });

        let url = server.url("").trim_end_matches('/').to_owned();

        let client = Client::new(url);
        (client, server)
    }

    pub fn start_server<F, D>(
        &self,
        scope_factory: F,
        shared_data: Option<D>,
    ) -> (Client, actix_test::TestServer)
    where
        F: Fn(&TestServerConfig) -> Scope + Clone + Send + 'static,
        D: Clone + Send + 'static,
    {
        self.start_server_with_scope(String::from("/api/v0.2"), scope_factory, shared_data)
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

        let default_factory_address =
            Address::from_str("1111111111111111111111111111111111111111").unwrap();
        storage
            .config_schema()
            .store_config(
                Default::default(),
                Default::default(),
                default_factory_address,
            )
            .await?;

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
                TokenKind::ERC20,
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
                TokenKind::ERC20,
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
                        &mut rng,
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

            let mut mint_nft_updates = Vec::new();
            for (i, tx) in txs.iter().enumerate() {
                if let Some(tx) = tx.get_executed_tx() {
                    if let ZkSyncTx::MintNFT(tx) = &tx.signed_tx.tx {
                        let nft_address: Address = rng.gen::<[u8; 20]>().into();
                        let content_hash: H256 = rng.gen::<[u8; 32]>().into();
                        let token = NFT::new(
                            TokenId(80000 + block_number.0 * 100 + i as u32),
                            0,
                            tx.creator_id,
                            tx.creator_address,
                            nft_address,
                            None,
                            content_hash,
                        );
                        let update = (
                            tx.creator_id,
                            AccountUpdate::MintNFT {
                                token,
                                nonce: Nonce(0),
                            },
                        );
                        mint_nft_updates.push(update);
                    }
                }
            }
            updates.extend(mint_nft_updates);

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
                storage
                    .chain()
                    .state_schema()
                    .apply_state_update(block_number)
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
                    dummy_deposit_op(Address::default(), AccountId(3), VERIFIED_OP_SERIAL_ID, 2).op,
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
                eth_block_index: Some(1),
                tx_hash: dummy_ethereum_tx_hash(VERIFIED_OP_SERIAL_ID as i64)
                    .as_bytes()
                    .to_vec(),
                affected_accounts: vec![Default::default()],
                token: 0,
            },
            // Committed priority operation.
            NewExecutedPriorityOperation {
                block_number: EXECUTED_BLOCKS_COUNT as i64 + 1,
                block_index: 1,
                operation: serde_json::to_value(
                    dummy_full_exit_op(AccountId(3), Address::default(), COMMITTED_OP_SERIAL_ID, 3)
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
                eth_block_index: Some(1),
                tx_hash: dummy_ethereum_tx_hash(COMMITTED_OP_SERIAL_ID as i64)
                    .as_bytes()
                    .to_vec(),
                affected_accounts: vec![Default::default()],
                token: 0,
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
            eth_block_index: Some(1),
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
            is_legacy: false,
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
            eth_block_index: Some(1),
        },
        op: deposit_op,
        block_index,
        created_at: Utc::now(),
    }
}

pub fn deserialize_response_result<T: DeserializeOwned>(response: Response) -> anyhow::Result<T> {
    match response.result {
        Some(result) => {
            let result = serde_json::from_value(result)?;
            Ok(result)
        }
        None => {
            if response.error.is_some() {
                anyhow::bail!("Response returned error: {:?}", response);
            } else {
                let result = serde_json::from_str("null")?;
                Ok(result)
            }
        }
    }
}

pub fn dummy_sign_verifier() -> mpsc::Sender<VerifySignatureRequest> {
    let (sender, mut receiver) = mpsc::channel::<VerifySignatureRequest>(10);

    actix_rt::spawn(async move {
        while let Some(item) = receiver.next().await {
            let verified = VerifiedTx::unverified(item.data.get_tx_variant());
            item.response
                .send(Ok(verified))
                .expect("Unable to send response");
        }
    });

    sender
}

#[derive(Debug, Clone)]
pub struct DummyFeeTickerInfo {
    prices: HashMap<TokenLike, BigDecimal>,
}

#[async_trait::async_trait]
impl FeeTickerInfo for DummyFeeTickerInfo {
    async fn is_account_new(&self, _address: Address) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn blocks_in_future_aggregated_operations(
        &self,
    ) -> crate::fee_ticker::ticker_info::BlocksInFutureAggregatedOperations {
        BlocksInFutureAggregatedOperations {
            blocks_to_commit: 1,
            blocks_to_prove: 1,
            blocks_to_execute: 1,
        }
    }

    async fn remaining_chunks_in_pending_block(&self) -> Option<usize> {
        None
    }

    async fn get_last_token_price(&self, token: TokenLike) -> Result<TokenPrice, PriceError> {
        if let Some(price) = self.prices.get(&token) {
            Ok(TokenPrice {
                usd_price: big_decimal_to_ratio(price).unwrap(),
                last_updated: Utc::now(),
            })
        } else {
            Ok(TokenPrice {
                usd_price: Ratio::zero(),
                last_updated: Utc::now(),
            })
        }
    }

    async fn get_gas_price_wei(&self) -> Result<BigUint, Error> {
        Ok(BigUint::from(1u64))
    }

    async fn get_token(&self, token: TokenLike) -> Result<Token, Error> {
        Ok(match token {
            TokenLike::Id(id) => Token {
                id,
                ..Default::default()
            },
            TokenLike::Address(address) => Token {
                address,
                ..Default::default()
            },
            TokenLike::Symbol(symbol) => Token {
                symbol,
                ..Default::default()
            },
        })
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

const SUBSIDY_CPK_PRICE_USD_SCALED: u64 = 10000000; // 10 dollars
const TEST_FAST_WITHDRAW_COEFF: f64 = 10.0;

pub fn get_test_ticker_config() -> TickerConfig {
    TickerConfig {
        zkp_cost_chunk_usd: UnsignedRatioSerializeAsDecimal::deserialize_from_str_with_dot("0.001")
            .unwrap(),
        gas_cost_tx: GasOperationsCost::from_constants(TEST_FAST_WITHDRAW_COEFF),
        tokens_risk_factors: TestToken::all_tokens()
            .into_iter()
            .filter_map(|t| {
                let id = t.id;
                t.risk_factor.map(|risk| (id, risk))
            })
            .collect(),
        scale_fee_coefficient: Ratio::new(BigUint::from(150u32), BigUint::from(100u32)),
        max_blocks_to_aggregate: 5,
        subsidy_cpk_price_usd: scaled_u64_to_ratio(SUBSIDY_CPK_PRICE_USD_SCALED),
    }
}
pub fn dummy_fee_ticker(
    prices: &[(TokenLike, BigDecimal)],
    in_memory_cache: Option<TokenInMemoryCache>,
) -> FeeTicker {
    let prices: HashMap<_, _> = prices.iter().cloned().collect();
    let validator = FeeTokenValidator::new(
        in_memory_cache.unwrap_or_default(),
        chrono::Duration::seconds(100),
        BigDecimal::from(100),
        Default::default(),
    );

    FeeTicker::new(
        Box::new(DummyFeeTickerInfo { prices }),
        get_test_ticker_config(),
        validator,
    )
}
