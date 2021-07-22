//! Block sizes test is used to create blocks of all available sizes, make proofs of them and verify onchain

use std::time::Instant;
use structopt::StructOpt;
use vlog::info;
use web3::transports::Http;

use zksync_circuit::witness::utils::build_block_witness;
use zksync_config::ZkSyncConfig;
use zksync_crypto::circuit::CircuitAccountTree;
use zksync_crypto::params::account_tree_depth;
use zksync_prover_utils::aggregated_proofs::{gen_aggregate_proof, prepare_proof_data};
use zksync_prover_utils::{PlonkVerificationKey, SetupForStepByStepProver};
use zksync_testkit::eth_account::EthereumAccount;
use zksync_testkit::external_commands::{deploy_contracts, get_test_accounts};
use zksync_testkit::zksync_account::{ZkSyncAccount, ZkSyncETHAccountData};
use zksync_testkit::{
    genesis_state, spawn_state_keeper, AccountSet, ETHAccountId, TestSetup, TestkitConfig, Token,
    ZKSyncAccountId,
};
use zksync_types::{aggregated_operations::BlocksProofOperation, DepositOp, Nonce, TokenId};

#[derive(Debug, StructOpt)]
#[structopt(name = "ZkSync block sizes test", author = "Matter Labs")]
struct Opt {
    #[structopt(long)]
    block_chunks_sizes: Option<Vec<usize>>,

    #[structopt(long)]
    skip_single_block_checks: bool,

    #[structopt(long)]
    aggregated_proof_sizes: Option<Vec<usize>>,
}

#[tokio::main]
async fn main() {
    let _sentry_guard = vlog::init();

    let opt = Opt::from_args();

    let block_chunks_sizes = if !opt.skip_single_block_checks {
        if let Some(block_chunks_sizes) = opt.block_chunks_sizes {
            let available_sizes = ZkSyncConfig::from_env()
                .chain
                .circuit
                .supported_block_chunks_sizes;
            for chunk in &block_chunks_sizes {
                available_sizes
                    .iter()
                    .find(|available_chunk| *available_chunk == chunk)
                    .expect("Block chunk size is not found in available sizes");
            }
            block_chunks_sizes
        } else {
            ZkSyncConfig::from_env()
                .chain
                .circuit
                .supported_block_chunks_sizes
        }
    } else {
        Vec::new()
    };

    let aggregated_proof_sizes = if let Some(aggregated_proof_sizes) = opt.aggregated_proof_sizes {
        let available_sizes = ZkSyncConfig::from_env()
            .chain
            .circuit
            .supported_aggregated_proof_sizes;
        for aggregated_size in &aggregated_proof_sizes {
            available_sizes
                .iter()
                .find(|available_size| *available_size == aggregated_size)
                .expect("Aggregates size is not found in available sizes");
        }
        aggregated_proof_sizes
    } else {
        ZkSyncConfig::from_env()
            .chain
            .circuit
            .supported_aggregated_proof_sizes
    };

    info!(
        "Checking proofs for block sizes: {:?}, aggregated sizes: {:?}",
        block_chunks_sizes, aggregated_proof_sizes
    );

    let available_block_chunk_sizes = ZkSyncConfig::from_env()
        .chain
        .circuit
        .supported_block_chunks_sizes;
    let available_aggregated_proof_sizes = ZkSyncConfig::from_env()
        .chain
        .circuit
        .supported_aggregated_proof_sizes_with_setup_pow();

    let testkit_config = TestkitConfig::from_env();

    let fee_account = ZkSyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address, genesis_state(&fee_account.address));

    let genesis_root = genesis_state(&fee_account.address).tree.root_hash();

    let contracts = deploy_contracts(true, genesis_root);

    let transport = Http::new(&testkit_config.web3_url).expect("http transport start");
    let (test_accounts_info, commit_account_info) = get_test_accounts();
    let commit_account = EthereumAccount::new(
        commit_account_info.private_key,
        commit_account_info.address,
        transport.clone(),
        contracts.contract,
        testkit_config.chain_id,
        testkit_config.gas_price_factor,
    );
    let eth_accounts = test_accounts_info
        .into_iter()
        .map(|test_eth_account| {
            EthereumAccount::new(
                test_eth_account.private_key,
                test_eth_account.address,
                transport.clone(),
                contracts.contract,
                testkit_config.chain_id,
                testkit_config.gas_price_factor,
            )
        })
        .collect::<Vec<_>>();

    let zksync_accounts = {
        let mut zksync_accounts = vec![fee_account];
        zksync_accounts.extend(eth_accounts.iter().map(|eth_account| {
            let rng_zksync_key = ZkSyncAccount::rand().private_key;
            ZkSyncAccount::new(
                rng_zksync_key,
                Nonce(0),
                eth_account.address,
                ZkSyncETHAccountData::EOA {
                    eth_private_key: eth_account.private_key,
                },
            )
        }));
        zksync_accounts
    };

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts,
        fee_account_id: ZKSyncAccountId(0),
    };

    let mut test_setup = TestSetup::new(
        sk_channels,
        accounts,
        &contracts,
        commit_account,
        genesis_root,
        None,
    );

    let account_state = test_setup.get_accounts_state().await;
    let mut circuit_account_tree = CircuitAccountTree::new(account_tree_depth());
    for (id, account) in account_state {
        circuit_account_tree.insert(*id, account.into());
    }

    let block_chunk_sizes = ZkSyncConfig::from_env()
        .chain
        .state_keeper
        .block_chunk_sizes;
    info!(
        "Checking keys and onchain verification for block sizes: {:?}",
        block_chunk_sizes
    );

    for block_size in block_chunk_sizes {
        info!("Checking keys for block size: {}", block_size);

        test_setup.start_block();
        for _ in 1..=(block_size / DepositOp::CHUNKS) {
            let (receipts, _) = test_setup
                .deposit(
                    ETHAccountId(1),
                    ZKSyncAccountId(2),
                    Token(TokenId(0)),
                    1u32.into(),
                )
                .await;
            receipts
                .last()
                .cloned()
                .expect("At least one receipt is expected for deposit");
        }
        let mut block = test_setup.execute_commit_block().await;
        assert!(block.block_chunks_size <= block_size);
        // complete block to the correct size with noops
        block.block_chunks_size = block_size;

        let timer = Instant::now();
        let witness = build_block_witness(&mut circuit_account_tree, &block)
            .expect("failed to build block witness");
        assert_eq!(
            witness.root_after_fees.unwrap(),
            block.new_root_hash,
            "witness root hash is incorrect"
        );
        info!("Witness done in {} ms", timer.elapsed().as_millis());

        let circuit = witness.into_circuit_instance();

        let timer = Instant::now();
        let prover_setup =
            SetupForStepByStepProver::prepare_setup_for_step_by_step_prover(circuit.clone(), false)
                .expect("failed to prepare setup for plonk prover");
        info!("Setup done in {} s", timer.elapsed().as_secs());

        let vk = PlonkVerificationKey::read_verification_key_for_main_circuit(block_size)
            .expect("Failed to get vk");
        let timer = Instant::now();
        let proof = prover_setup
            .gen_step_by_step_proof_using_prepared_setup(circuit, &vk)
            .expect("Failed to gen proof");
        info!("Proof done in {} s", timer.elapsed().as_secs());

        let mut proofs = Vec::new();
        for _ in 0..1 {
            proofs.push((proof.clone(), block_size));
        }
        let (vks, proof_data) = prepare_proof_data(&available_block_chunk_sizes, proofs);
        let aggreagated_proof =
            gen_aggregate_proof(vks, proof_data, &available_aggregated_proof_sizes, false)
                .expect("Failed to generate aggreagated proof");

        let proof_op = BlocksProofOperation {
            blocks: vec![block],
            proof: aggreagated_proof.serialize_aggregated_proof(),
        };
        test_setup
            .execute_verify_commitments(proof_op)
            .await
            .expect_success();
    }

    for aggregated_proof_size in aggregated_proof_sizes {
        let block_size = *available_block_chunk_sizes.first().unwrap();
        info!("Checking recursive keys for block size: {}", block_size);

        let mut blocks = Vec::new();
        let mut proofs = Vec::new();

        for _ in 0..aggregated_proof_size {
            test_setup.start_block();
            for _ in 1..=(block_size / DepositOp::CHUNKS) {
                let (receipts, _) = test_setup
                    .deposit(
                        ETHAccountId(1),
                        ZKSyncAccountId(2),
                        Token(TokenId(0)),
                        1u32.into(),
                    )
                    .await;
                receipts
                    .last()
                    .cloned()
                    .expect("At least one receipt is expected for deposit");
            }
            let mut block = test_setup.execute_commit_block().await;
            assert!(block.block_chunks_size <= block_size);
            // complete block to the correct size with noops
            block.block_chunks_size = block_size;

            let timer = Instant::now();
            let witness = build_block_witness(&mut circuit_account_tree, &block)
                .expect("failed to build block witness");
            assert_eq!(
                witness.root_after_fees.unwrap(),
                block.new_root_hash,
                "witness root hash is incorrect"
            );
            info!("Witness done in {} ms", timer.elapsed().as_millis());

            let circuit = witness.into_circuit_instance();

            let timer = Instant::now();
            let prover_setup = SetupForStepByStepProver::prepare_setup_for_step_by_step_prover(
                circuit.clone(),
                false,
            )
            .expect("failed to prepare setup for plonk prover");
            info!("Setup done in {} s", timer.elapsed().as_secs());

            let vk = PlonkVerificationKey::read_verification_key_for_main_circuit(block_size)
                .expect("Failed to get vk");
            let timer = Instant::now();
            let proof = prover_setup
                .gen_step_by_step_proof_using_prepared_setup(circuit, &vk)
                .expect("Failed to gen proof");
            info!("Proof done in {} s", timer.elapsed().as_secs());

            proofs.push((proof.clone(), block_size));
            blocks.push(block);
        }

        let (vks, proof_data) = prepare_proof_data(&available_block_chunk_sizes, proofs);
        let aggregated_proof =
            gen_aggregate_proof(vks, proof_data, &available_aggregated_proof_sizes, false)
                .expect("Failed to generate aggregated proof");

        let proof_op = BlocksProofOperation {
            blocks,
            proof: aggregated_proof.serialize_aggregated_proof(),
        };
        let tx_receipt = test_setup
            .execute_verify_commitments(proof_op)
            .await
            .expect_success();
        info!(
            "Aggregated proof, size: {}, gas cost: {}",
            aggregated_proof_size,
            tx_receipt.gas_used.expect("Gas used empty")
        );
    }

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}
