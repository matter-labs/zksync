//! Block sizes test is used to create blocks of all available sizes, make proofs of them and verify onchain

use log::info;
use std::time::Instant;
use web3::transports::Http;
use zksync_circuit::witness::utils::build_block_witness;
use zksync_config::AvailableBlockSizesConfig;
use zksync_crypto::circuit::CircuitAccountTree;
use zksync_crypto::params::account_tree_depth;
use zksync_prover_utils::{PlonkVerificationKey, SetupForStepByStepProver};
use zksync_testkit::eth_account::EthereumAccount;
use zksync_testkit::external_commands::{deploy_contracts, get_test_accounts};
use zksync_testkit::zksync_account::ZkSyncAccount;
use zksync_testkit::{
    genesis_state, get_testkit_config_from_env, spawn_state_keeper, AccountSet, ETHAccountId,
    TestSetup, Token, ZKSyncAccountId,
};
use zksync_types::DepositOp;

#[tokio::main]
async fn main() {
    env_logger::init();

    let testkit_config = get_testkit_config_from_env();

    let fee_account = ZkSyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address);

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
        let mut zksync_accounts = Vec::new();
        zksync_accounts.push(fee_account);
        zksync_accounts.extend(eth_accounts.iter().map(|eth_account| {
            let rng_zksync_key = ZkSyncAccount::rand().private_key;
            ZkSyncAccount::new(
                rng_zksync_key,
                0,
                eth_account.address,
                eth_account.private_key,
            )
        }));
        zksync_accounts
    };

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts,
        fee_account_id: ZKSyncAccountId(0),
    };

    let mut test_setup = TestSetup::new(sk_channels, accounts, &contracts, commit_account);

    let account_state = test_setup.get_accounts_state().await;
    let mut circuit_account_tree = CircuitAccountTree::new(account_tree_depth());
    for (id, account) in account_state {
        circuit_account_tree.insert(id, account.into());
    }

    let block_chunk_sizes = AvailableBlockSizesConfig::from_env().blocks_chunks;
    info!(
        "Checking keys and onchain verification for block sizes: {:?}",
        block_chunk_sizes
    );

    for block_size in block_chunk_sizes {
        info!("Checking keys for block size: {}", block_size);

        test_setup.start_block();
        for _ in 1..=(block_size / DepositOp::CHUNKS) {
            test_setup
                .deposit(ETHAccountId(1), ZKSyncAccountId(2), Token(0), 1u32.into())
                .await
                .last()
                .cloned()
                .expect("At least one receipt is expected for deposit");
        }
        let (_, mut block) = test_setup.execute_commit_block().await;
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

        test_setup
            .execute_verify_block(&block, proof)
            .await
            .expect_success();
    }

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}
