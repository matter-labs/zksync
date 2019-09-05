use bellman;

use time::PreciseTime;

use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use pairing::bn256::*;
use rand::OsRng;

use bellman::groth16::generate_random_parameters;

use crate::vk_contract_generator::generate_vk_contract;
use circuit::account::AccountWitness;
use circuit::circuit::FranklinCircuit;
use circuit::operation::*;
use models::params as franklin_constants;
use std::path::PathBuf;

const CONTRACT_FILENAME: &str = "VerificationKey.sol";
const CONTRACT_NAME: &str = "VerificationKey";
const CONTRACT_FUNCTION_NAME: &str = "getVk";

pub fn make_franklin_key() {
    let out_dir = {
        let mut out_dir = PathBuf::new();
        out_dir.push(&std::env::var("KEY_DIR").expect("KEY_DIR not set"));
        out_dir.push(&format!("{}", franklin_constants::BLOCK_SIZE_CHUNKS));
        out_dir
    };
    let key_file_path = {
        let mut key_file_path = out_dir.clone();
        key_file_path.push(franklin_constants::KEY_FILENAME);
        key_file_path
    };
    let contract_file_path = {
        let mut contract_file_path = out_dir.clone();
        contract_file_path.push(CONTRACT_FILENAME);
        contract_file_path
    };

    info!(
        "Generating key file into: {}",
        key_file_path.to_str().unwrap()
    );
    info!(
        "Generating contract key file into: {}",
        contract_file_path.to_str().unwrap()
    );

    // let p_g = FixedGenerators::SpendingKeyGenerator;
    let params = &AltJubjubBn256::new();
    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let rng = &mut OsRng::new().unwrap();

    let empty_operation = Operation {
        new_root: None,
        tx_type: None,
        chunk: None,
        pubdata_chunk: None,
        signer_pub_key_x: None,
        signer_pub_key_y: None,
        first_sig_msg: None,
        second_sig_msg: None,
        signature: None,
        args: OperationArguments {
            a: None,
            b: None,
            amount: None,
            fee: None,
            new_pub_key_hash: None,
            ethereum_key: None,
        },
        lhs: OperationBranch {
            address: None,
            token: None,
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: None,
                    pub_key_hash: None,
                },
                account_path: vec![None; franklin_constants::ACCOUNT_TREE_DEPTH],
                balance_value: None,
                balance_subtree_path: vec![None; franklin_constants::BALANCE_TREE_DEPTH],
            },
        },
        rhs: OperationBranch {
            address: None,
            token: None,
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: None,
                    pub_key_hash: None,
                },
                account_path: vec![None; franklin_constants::ACCOUNT_TREE_DEPTH],
                balance_value: None,
                balance_subtree_path: vec![None; franklin_constants::BALANCE_TREE_DEPTH],
            },
        },
    };

    let instance_for_generation: FranklinCircuit<'_, Bn256> = FranklinCircuit {
        params,
        operation_batch_size: franklin_constants::BLOCK_SIZE_CHUNKS,
        old_root: None,
        new_root: None,
        validator_address: None,
        block_number: None,
        pub_data_commitment: None,
        validator_balances: vec![None; (1 << franklin_constants::BALANCE_TREE_DEPTH) as usize],
        validator_audit_path: vec![None; franklin_constants::ACCOUNT_TREE_DEPTH],
        operations: vec![empty_operation; franklin_constants::BLOCK_SIZE_CHUNKS],
        validator_account: AccountWitness {
            nonce: None,
            pub_key_hash: None,
        },
    };

    info!("generating setup...");
    let start = PreciseTime::now();
    let tmp_cirtuit_params = generate_random_parameters(instance_for_generation, rng).unwrap();
    info!(
        "setup generated in {} s",
        start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0
    );

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
    let circuit_params = bellman::groth16::Parameters::<Bn256>::read(&mut r, true)
        .expect("Unable to read proving key");

    let contract_content = generate_vk_contract(
        &circuit_params.vk,
        CONTRACT_NAME.to_string(),
        CONTRACT_FUNCTION_NAME.to_string(),
    );

    let f_cont = File::create(contract_file_path).expect("Unable to create file");
    let mut f_cont = BufWriter::new(f_cont);
    f_cont
        .write_all(contract_content.as_bytes())
        .expect("Unable to write contract");

    info!("Done");
}
