// Built-in deps
use std::fs::{remove_file, File};
use std::path::Path;
// Workspace deps
use zksync_circuit::account::AccountWitness;
use zksync_circuit::circuit::ZkSyncCircuit;
use zksync_circuit::exit_circuit::ZkSyncExitCircuit;
use zksync_circuit::operation::{
    Operation, OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
};
use zksync_config::configs::ChainConfig;
use zksync_crypto::bellman::plonk::{make_verification_key, setup, transpile_with_gates_count};
use zksync_crypto::bellman::Circuit;
use zksync_crypto::params;
use zksync_crypto::Engine;
use zksync_prover_utils::fs_utils::{
    get_block_verification_key_path, get_exodus_verification_key_path,
    get_universal_setup_monomial_form,
};

pub(crate) fn make_plonk_exodus_verify_key() {
    let key_path = get_exodus_verification_key_path();
    vlog::info!(
        "Generating exodus verification key into: {}",
        key_path.display()
    );
    generate_verification_key(exit_circuit(), key_path);
}

/// Generates and saves verification keys for given block sizes. (high of memory consumption)
pub(crate) fn make_plonk_blocks_verify_keys(config: ChainConfig) {
    for (block_chunks, setup_power) in config.circuit.supported_block_chunks_sizes.into_iter().zip(
        config
            .circuit
            .supported_block_chunks_sizes_setup_powers
            .into_iter(),
    ) {
        let key_path = get_block_verification_key_path(block_chunks);
        vlog::info!(
            "Generating block: {} verification key into: {}",
            block_chunks,
            key_path.display()
        );
        let result_setup_power = generate_verification_key(zksync_circuit(block_chunks), key_path);
        assert_eq!(
            result_setup_power, setup_power as u32,
            "setup power actually needed by circuit of size {} is not equal to that from SUPPORTED_BLOCK_CHUNKS_SIZES env variable", block_chunks
        );
    }
}

/// Creates instance of the exodus mode zkSync circuit.
fn exit_circuit() -> impl Circuit<Engine> + Clone {
    let empty_branch = OperationBranch {
        address: None,
        token: None,
        witness: OperationBranchWitness {
            account_witness: AccountWitness {
                nonce: None,
                pub_key_hash: None,
                address: None,
            },
            account_path: vec![None; params::account_tree_depth()],
            balance_value: None,
            balance_subtree_path: vec![None; params::balance_tree_depth()],
        },
    };
    ZkSyncExitCircuit::<'_, Engine> {
        params: &params::RESCUE_PARAMS,
        pub_data_commitment: None,
        root_hash: None,
        account_audit_data: empty_branch.clone(),
        special_account_audit_data: empty_branch.clone(),
        creator_account_audit_data: empty_branch,
        serial_id: None,
        content_hash: vec![None; params::CONTENT_HASH_WIDTH],
    }
}

/// Creates instance of the main zkSync circuit with the given number chunks in block.
fn zksync_circuit(block_chunks: usize) -> impl Circuit<Engine> + Clone {
    let empty_operation = Operation {
        new_root: None,
        tx_type: None,
        chunk: None,
        pubdata_chunk: None,
        signer_pub_key_packed: vec![None; params::FR_BIT_WIDTH_PADDED],
        first_sig_msg: None,
        second_sig_msg: None,
        third_sig_msg: None,
        signature_data: SignatureData::init_empty(),
        args: OperationArguments {
            a: None,
            b: None,
            amount_packed: None,
            second_amount_packed: None,
            special_amounts: vec![None; 2],
            special_prices: vec![None; 4],
            special_nonces: vec![None; 3],
            special_tokens: vec![None; 3],
            special_accounts: vec![None; 5],
            special_eth_addresses: vec![None; 2],
            full_amount: None,
            fee: None,
            pub_nonce: None,
            new_pub_key_hash: None,
            eth_address: None,
            valid_from: None,
            valid_until: None,
            second_valid_from: None,
            second_valid_until: None,
            special_content_hash: vec![None; params::CONTENT_HASH_WIDTH],
            special_serial_id: None,
        },
        lhs: OperationBranch {
            address: None,
            token: None,
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: None,
                    pub_key_hash: None,
                    address: None,
                },
                account_path: vec![None; params::account_tree_depth()],
                balance_value: None,
                balance_subtree_path: vec![None; params::balance_tree_depth()],
            },
        },
        rhs: OperationBranch {
            address: None,
            token: None,
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: None,
                    pub_key_hash: None,
                    address: None,
                },
                account_path: vec![None; params::account_tree_depth()],
                balance_value: None,
                balance_subtree_path: vec![None; params::balance_tree_depth()],
            },
        },
    };

    ZkSyncCircuit::<'_, Engine> {
        rescue_params: &params::RESCUE_PARAMS,
        jubjub_params: &params::JUBJUB_PARAMS,
        old_root: None,
        initial_used_subtree_root: None,
        validator_address: None,
        block_number: None,
        block_timestamp: None,
        pub_data_commitment: None,
        validator_balances: vec![None; params::number_of_processable_tokens()],
        validator_audit_path: vec![None; params::account_tree_depth()],
        validator_non_processable_tokens_audit_before_fees: vec![
            None;
            params::balance_tree_depth()
                - params::PROCESSABLE_TOKENS_DEPTH
                    as usize
        ],
        validator_non_processable_tokens_audit_after_fees: vec![
            None;
            params::balance_tree_depth()
                - params::PROCESSABLE_TOKENS_DEPTH
                    as usize
        ],
        operations: vec![empty_operation; block_chunks],
        validator_account: AccountWitness {
            nonce: None,
            pub_key_hash: None,
            address: None,
        },
    }
}

/// Generates PLONK verification key for given circuit and saves key at the given path.
/// Returns used setup power of two. (e.g. 22)
fn generate_verification_key<C: Circuit<Engine> + Clone, P: AsRef<Path>>(
    circuit: C,
    path: P,
) -> u32 {
    let path = path.as_ref();
    assert!(
        !path.exists(),
        "path for saving verification key exists: {}",
        path.display()
    );
    {
        File::create(path).expect("can't create file at verification key path");
        remove_file(path).unwrap_or_default()
    }

    vlog::info!("Transpiling circuit");
    let (gates_count, transpilation_hints) =
        transpile_with_gates_count(circuit.clone()).expect("failed to transpile");
    let size_log2 = gates_count.next_power_of_two().trailing_zeros();
    assert!(
        size_log2 <= 26,
        "power of two too big {}, max: 26",
        size_log2
    );

    // exodus circuit is to small for the smallest setup
    let size_log2 = std::cmp::max(20, size_log2);
    vlog::info!(
        "Reading setup file, gates_count: {}, pow2: {}",
        gates_count,
        size_log2
    );

    let key_monomial_form =
        get_universal_setup_monomial_form(size_log2).expect("Failed to read setup file.");

    vlog::info!("Generating setup");
    let setup = setup(circuit, &transpilation_hints).expect("failed to make setup");
    vlog::info!("Generating verification key");
    let verification_key = make_verification_key(&setup, &key_monomial_form)
        .expect("failed to create verification key");
    verification_key
        .write(File::create(path).unwrap())
        .expect("Failed to write verification file."); // unwrap - checked at the function entry
    vlog::info!("Verification key successfully generated");
    size_log2
}

/// Transpile zkSync circuit to get gate count and universal setup power of two
fn gates_count_zksync_main_circuit(chunks: usize) -> (usize, u32) {
    let (gates_count, _) =
        transpile_with_gates_count(zksync_circuit(chunks)).expect("failed to transpile");
    let size_log2 = gates_count.next_power_of_two().trailing_zeros();

    (gates_count, size_log2)
}

/// Calculates max zkSync circuit size for universal setup power of 21..26
pub fn calculate_and_print_max_zksync_main_circuit_size() {
    vlog::info!("Counting max zkSync circuit size for setup");
    let mut chunks = 6;
    let mut setup_power = gates_count_zksync_main_circuit(chunks).1;
    while setup_power <= 26 {
        let new_chunks = chunks + 2;
        let (gate_count, power_2) = gates_count_zksync_main_circuit(new_chunks);
        if power_2 <= setup_power {
            chunks = new_chunks;
        } else {
            vlog::info!(
                "setup_size_log2: {}, chunks: {}, gate_count: {}",
                setup_power,
                chunks,
                gate_count
            );
            setup_power += 1;
            chunks *= 2;
        }
    }
}
