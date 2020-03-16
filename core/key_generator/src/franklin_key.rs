// Built-in deps
use std::path::PathBuf;
// External deps
use circuit::account::AccountWitness;
use circuit::circuit::FranklinCircuit;
use circuit::operation::*;
use crypto_exports::franklin_crypto::bellman::groth16::generate_random_parameters;
use crypto_exports::franklin_crypto::bellman::groth16::Parameters;
use crypto_exports::franklin_crypto::bellman::pairing::bn256::*;
use crypto_exports::rand::OsRng;
// Workspace deps
use crate::vk_contract_generator::generate_vk_function;
use circuit::exit_circuit::ZksyncExitCircuit;
use models::params;
use models::prover_utils::{get_block_proof_key_and_vk_path, get_exodus_proof_key_and_vk_path};
use std::time::Instant;

const CONTRACT_FUNCTION_NAME: &str = "getVk";

fn generate_and_write_parameters<F: Fn() -> Parameters<Bn256>>(
    key_file_path: PathBuf,
    contract_file_path: PathBuf,
    gen_parameters: F,
    contract_function_name: &str,
) {
    info!(
        "Generating key file into: {}",
        key_file_path.to_str().unwrap()
    );
    info!(
        "Generating contract key file into: {}",
        contract_file_path.to_str().unwrap()
    );
    let f_cont = File::create(contract_file_path).expect("Unable to create file");

    let tmp_cirtuit_params = gen_parameters();

    use std::fs::File;
    use std::io::{BufWriter, Write};
    {
        let f = File::create(&key_file_path).expect("Unable to create file");
        let mut f = BufWriter::new(f);
        tmp_cirtuit_params
            .write(&mut f)
            .expect("Unable to write proving key");
    }

    use std::io::BufReader;

    let f_r = File::open(&key_file_path).expect("Unable to open file");
    let mut r = BufReader::new(f_r);
    let circuit_params = crypto_exports::bellman::groth16::Parameters::<Bn256>::read(&mut r, true)
        .expect("Unable to read proving key");

    let contract_content = generate_vk_function(&circuit_params.vk, contract_function_name);

    let mut f_cont = BufWriter::new(f_cont);
    f_cont
        .write_all(contract_content.as_bytes())
        .expect("Unable to write contract");
}

pub fn make_block_proof_key() {
    for &block_size in params::block_chunk_sizes() {
        let (key_file_path, get_vk_file_path) = get_block_proof_key_and_vk_path(block_size);
        generate_and_write_parameters(
            key_file_path,
            get_vk_file_path,
            || make_circuit_parameters(block_size),
            &format!("{}Block{}", CONTRACT_FUNCTION_NAME, block_size),
        );
    }
}

pub fn make_exodus_key() {
    let (key_file_path, get_vk_file_path) = get_exodus_proof_key_and_vk_path();
    generate_and_write_parameters(
        key_file_path,
        get_vk_file_path,
        make_exit_circuit_parameters,
        &format!("{}{}", CONTRACT_FUNCTION_NAME, "Exit"),
    );
}

pub fn make_circuit_parameters(block_size: usize) -> Parameters<Bn256> {
    // let p_g = FixedGenerators::SpendingKeyGenerator;
    let params = &params::JUBJUB_PARAMS;
    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let rng = &mut OsRng::new().unwrap();

    let empty_operation = Operation {
        new_root: None,
        tx_type: None,
        chunk: None,
        pubdata_chunk: None,
        signer_pub_key_packed: vec![None; params::FR_BIT_WIDTH_PADDED],
        first_sig_msg: None,
        second_sig_msg: None,
        third_sig_msg: None,
        signature_data: SignatureData {
            r_packed: vec![None; params::FR_BIT_WIDTH_PADDED],
            s: vec![None; params::FR_BIT_WIDTH_PADDED],
        },
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
                balance_subtree_path: vec![None; params::BALANCE_TREE_DEPTH],
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
                balance_subtree_path: vec![None; params::BALANCE_TREE_DEPTH],
            },
        },
    };

    let instance_for_generation: FranklinCircuit<'_, Bn256> = FranklinCircuit {
        params,
        operation_batch_size: block_size,
        old_root: None,
        new_root: None,
        validator_address: None,
        block_number: None,
        pub_data_commitment: None,
        validator_balances: vec![None; (1 << params::BALANCE_TREE_DEPTH) as usize],
        validator_audit_path: vec![None; params::account_tree_depth()],
        operations: vec![empty_operation; block_size],
        validator_account: AccountWitness {
            nonce: None,
            pub_key_hash: None,
            address: None,
        },
    };

    info!("generating setup...");
    let start = Instant::now();
    let tmp_cirtuit_params = generate_random_parameters(instance_for_generation, rng).unwrap();
    info!("setup generated in {} s", start.elapsed().as_secs());

    tmp_cirtuit_params
}

pub fn make_exit_circuit_parameters() -> Parameters<Bn256> {
    // let p_g = FixedGenerators::SpendingKeyGenerator;
    let params = &params::JUBJUB_PARAMS;
    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let rng = &mut OsRng::new().unwrap();
    let exit_circuit_instance = ZksyncExitCircuit::<'_, Bn256> {
        params,
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
                balance_subtree_path: vec![None; params::BALANCE_TREE_DEPTH],
            },
        },
    };

    info!("generating setup for exit circuit...");
    let start = Instant::now();
    let tmp_cirtuit_params = generate_random_parameters(exit_circuit_instance, rng).unwrap();
    info!("setup generated in {} s", start.elapsed().as_secs());

    tmp_cirtuit_params
}
