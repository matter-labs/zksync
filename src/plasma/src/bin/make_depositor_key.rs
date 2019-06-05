use bellman;

use time::PreciseTime;

use pairing::bn256::*;
use rand::OsRng;
use sapling_crypto::alt_babyjubjub::AltJubjubBn256;

use bellman::groth16::generate_random_parameters;

use plasma::vk_contract_generator::generate_vk_contract;

use plasma::circuit::deposit::circuit::{Deposit, DepositWitness};
use plasma::circuit::deposit::deposit_request::DepositRequest;
use plasma::circuit::leaf::LeafWitness;
use plasma::models::params as plasma_constants;

const DEPOSIT_BATCH_SIZE: usize = 1;
const FILENAME: &str = "deposit_pk.key";
const CONTRACT_FILENAME: &str = "DepositVerificationKey.sol";
const CONTRACT_NAME: &str = "DepositVerificationKey";
const CONTRACT_FUNCTION_NAME: &str = "getVkDepositCircuit";

fn main() {
    // let p_g = FixedGenerators::SpendingKeyGenerator;
    let params = &AltJubjubBn256::new();
    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let rng = &mut OsRng::new().unwrap();

    let empty_request = DepositRequest {
        into: None,
        amount: None,
        public_key: None,
    };

    let empty_leaf_witness = LeafWitness {
        balance: None,
        nonce: None,
        pub_x: None,
        pub_y: None,
    };

    let empty_witness = DepositWitness {
        leaf: empty_leaf_witness.clone(),
        auth_path: vec![None; plasma_constants::BALANCE_TREE_DEPTH],
        leaf_is_empty: None,
        new_pub_x: None,
        new_pub_y: None,
    };

    let instance_for_generation: Deposit<'_, Bn256> = Deposit {
        params: params,
        number_of_deposits: DEPOSIT_BATCH_SIZE,
        old_root: None,
        new_root: None,
        public_data_commitment: None,
        block_number: None,
        requests: vec![(empty_request, empty_witness); DEPOSIT_BATCH_SIZE],
    };

    println!("generating setup...");
    let start = PreciseTime::now();
    let tmp_cirtuit_params = generate_random_parameters(instance_for_generation, rng).unwrap();
    println!(
        "setup generated in {} s",
        start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0
    );

    use std::fs::File;
    use std::io::{BufWriter, Write};
    {
        let f = File::create(FILENAME).expect("Unable to create file");
        let mut f = BufWriter::new(f);
        tmp_cirtuit_params
            .write(&mut f)
            .expect("Unable to write proving key");
    }

    use std::io::BufReader;

    let f_r = File::open(FILENAME).expect("Unable to open file");
    let mut r = BufReader::new(f_r);
    let circuit_params = bellman::groth16::Parameters::<Bn256>::read(&mut r, true)
        .expect("Unable to read proving key");

    let contract_content = generate_vk_contract(
        &circuit_params.vk,
        CONTRACT_NAME.to_string(),
        CONTRACT_FUNCTION_NAME.to_string(),
    );

    let f_cont = File::create(CONTRACT_FILENAME).expect("Unable to create file");
    let mut f_cont = BufWriter::new(f_cont);
    f_cont
        .write_all(contract_content.as_bytes())
        .expect("Unable to write contract");

    println!("Done");
}
