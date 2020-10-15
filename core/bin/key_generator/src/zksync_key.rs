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
use zksync_config::AvailableBlockSizesConfig;
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
    log::info!(
        "Generating exodus verification key into: {}",
        key_path.display()
    );
    generate_verification_key(exit_circuit(), key_path);
}

/// Generates and saves verification keys for given block sizes. (high of memory consumption)
pub(crate) fn make_plonk_blocks_verify_keys(config: AvailableBlockSizesConfig) {
    for (block_chunks, setup_power) in config
        .blocks_chunks
        .into_iter()
        .zip(config.blocks_setup_power2.into_iter())
    {
        let key_path = get_block_verification_key_path(block_chunks);
        log::info!(
            "Generating block: {} verification key into: {}",
            block_chunks,
            key_path.display()
        );
        let result_setup_power = generate_verification_key(zksync_circuit(block_chunks), key_path);
        assert_eq!(
            result_setup_power, setup_power,
            "setup power actually needed by circuit of size {} is not equal to that from SUPPORTED_BLOCK_CHUNKS_SIZES env variable", block_chunks
        );
    }
}

/// Creates instance of the exodus mode zkSync circuit.
fn exit_circuit() -> impl Circuit<Engine> + Clone {
    ZkSyncExitCircuit::<'_, Engine> {
        params: &params::RESCUE_PARAMS,
        pub_data_commitment: None,
        root_hash: None,
        account_audit_data: OperationBranch {
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
            full_amount: None,
            fee: None,
            pub_nonce: None,
            new_pub_key_hash: None,
            eth_address: None,
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
        pub_data_commitment: None,
        validator_balances: vec![None; params::total_tokens()],
        validator_audit_path: vec![None; params::account_tree_depth()],
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

    log::info!("Transpiling circuit");
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
    log::info!(
        "Reading setup file, gates_count: {}, pow2: {}",
        gates_count,
        size_log2
    );

    let key_monomial_form =
        get_universal_setup_monomial_form(size_log2).expect("Failed to read setup file.");

    log::info!("Generating setup");
    let setup = setup(circuit, &transpilation_hints).expect("failed to make setup");
    log::info!("Generating verification key");
    let verification_key = make_verification_key(&setup, &key_monomial_form)
        .expect("failed to create verification key");
    verification_key
        .write(File::create(path).unwrap())
        .expect("Failed to write verification file."); // unwrap - checked at the function entry
    log::info!("Verification key successfully generated");
    size_log2
}
