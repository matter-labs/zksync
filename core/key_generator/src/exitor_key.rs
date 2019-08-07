//use bellman;
//
//use time::PreciseTime;
//
//use pairing::bn256::*;
//use rand::OsRng;
//use sapling_crypto::alt_babyjubjub::AltJubjubBn256;
//
//use bellman::groth16::generate_random_parameters;
//
//use crate::vk_contract_generator::generate_vk_contract;
//
//use circuit::exit::circuit::{Exit, ExitWitness};
//use circuit::exit::exit_request::ExitRequest;
//use circuit::leaf::LeafWitness;
//use models::plasma::params as plasma_constants;
//
//const EXIT_BATCH_SIZE: usize = 1;
//const FILENAME: &str = "exit_pk.key";
//const CONTRACT_FILENAME: &str = "ExitVerificationKey.sol";
//const CONTRACT_NAME: &str = "ExitVerificationKey";
//const CONTRACT_FUNCTION_NAME: &str = "getVkExitCircuit";
//
//pub fn make_exitor_key() {
//    // let p_g = FixedGenerators::SpendingKeyGenerator;
//    let params = &AltJubjubBn256::new();
//    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
//    let rng = &mut OsRng::new().unwrap();
//
//    let empty_request = ExitRequest {
//        from: None,
//        amount: None,
//    };
//
//    let empty_leaf_witness = LeafWitness {
//        balance: None,
//        nonce: None,
//        pub_x: None,
//        pub_y: None,
//    };
//
//    let empty_witness = ExitWitness {
//        leaf: empty_leaf_witness.clone(),
//        auth_path: vec![None; plasma_constants::BALANCE_TREE_DEPTH],
//    };
//
//    let instance_for_generation: Exit<'_, Bn256> = Exit {
//        params,
//        number_of_exits: EXIT_BATCH_SIZE,
//        old_root: None,
//        new_root: None,
//        public_data_commitment: None,
//        empty_leaf_witness: empty_leaf_witness.clone(),
//        block_number: None,
//        requests: vec![(empty_request, empty_witness); EXIT_BATCH_SIZE],
//    };
//
//    info!("generating setup...");
//    let start = PreciseTime::now();
//    let tmp_cirtuit_params = generate_random_parameters(instance_for_generation, rng).unwrap();
//    info!(
//        "setup generated in {} s",
//        start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0
//    );
//
//    use std::fs::File;
//    use std::io::{BufWriter, Write};
//    {
//        let f = File::create(FILENAME).expect("Unable to create file");
//        let mut f = BufWriter::new(f);
//        tmp_cirtuit_params
//            .write(&mut f)
//            .expect("Unable to write proving key");
//    }
//
//    use std::io::BufReader;
//
//    let f_r = File::open(FILENAME).expect("Unable to open file");
//    let mut r = BufReader::new(f_r);
//    let circuit_params = bellman::groth16::Parameters::<Bn256>::read(&mut r, true)
//        .expect("Unable to read proving key");
//
//    let contract_content = generate_vk_contract(
//        &circuit_params.vk,
//        CONTRACT_NAME.to_string(),
//        CONTRACT_FUNCTION_NAME.to_string(),
//    );
//
//    let f_cont = File::create(CONTRACT_FILENAME).expect("Unable to create file");
//    let mut f_cont = BufWriter::new(f_cont);
//    f_cont
//        .write_all(contract_content.as_bytes())
//        .expect("Unable to write contract");
//
//    info!("Done");
//}
