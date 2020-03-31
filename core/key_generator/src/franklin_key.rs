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

fn estimate_power_of_two(block_size: usize) -> u32 {
    // let p_g = FixedGenerators::SpendingKeyGenerator;
    let jubjub_params = &params::JUBJUB_PARAMS;
    let rescue_params = &params::RESCUE_PARAMS;
    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

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
        rescue_params,
        jubjub_params,
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

    use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
    use crypto_exports::franklin_crypto::bellman::kate_commitment::*;
    use crypto_exports::franklin_crypto::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
    use crypto_exports::franklin_crypto::bellman::plonk::*;
    use crypto_exports::franklin_crypto::bellman::worker::Worker;
    use crypto_exports::franklin_crypto::bellman::plonk::better_cs::{adaptor::{TranspilationVariant, write_transpilation_hints}, cs::PlonkCsWidth4WithNextStepParams};
    use crypto_exports::franklin_crypto::bellman::plonk::fft::cooley_tukey_ntt::*;

    let hints = transpile::<Bn256, _>(instance_for_generation.clone())
        .expect("transpilation is successful");
    let setup = setup(instance_for_generation.clone(), &hints).expect("must make setup");

    // let timer = Instant::now();
    // let setup = SetupPolynomials::<_, PlonkCsWidth4WithNextStepParams>::read(
    //     std::io::BufReader::with_capacity(1 << 29, std::fs::File::open("setup").unwrap()),
    // )
    // .expect("setup read");
    // println!("setup read: {}", timer.elapsed().as_secs());

    let size = setup.n.next_power_of_two();
    let power_of_two = size.trailing_zeros();
    return power_of_two;
}

pub fn make_circuit_parameters(block_size: usize) -> Parameters<Bn256> {
    // let p_g = FixedGenerators::SpendingKeyGenerator;
    let jubjub_params = &params::JUBJUB_PARAMS;
    let rescue_params = &params::RESCUE_PARAMS;
    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

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
        rescue_params,
        jubjub_params,
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

    use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
    use crypto_exports::franklin_crypto::bellman::kate_commitment::*;
    use crypto_exports::franklin_crypto::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
    use crypto_exports::franklin_crypto::bellman::plonk::*;
    use crypto_exports::franklin_crypto::bellman::worker::Worker;
    use crypto_exports::franklin_crypto::bellman::plonk::better_cs::{adaptor::{TranspilationVariant, write_transpilation_hints}, cs::PlonkCsWidth4WithNextStepParams};
    use crypto_exports::franklin_crypto::bellman::plonk::fft::cooley_tukey_ntt::*;

    let universal_setup_path: String = format!(
        "{}/keys/setup/",
        std::env::var("ZKSYNC_HOME").expect("ZKSYNC_HOME_ENV")
    );

    let timer = Instant::now();
    let hints = transpile::<Bn256, _>(instance_for_generation.clone())
        .expect("transpilation is successful");
    println!("Done transpiling {}", timer.elapsed().as_secs());
    write_transpilation_hints(
        &hints,
        std::io::BufWriter::with_capacity(1 << 24, std::fs::File::create("hints").unwrap()),
    );

    let timer = Instant::now();
    let setup = setup(instance_for_generation.clone(), &hints).expect("must make setup");
    println!("setup generated: {}", timer.elapsed().as_secs());
    let mut setup_frs = 0;
    setup_frs += dbg!(setup
        .selector_polynomials
        .iter()
        .map(|p| p.clone().into_coeffs().len())
        .sum::<usize>());
    setup_frs += dbg!(setup
        .next_step_selector_polynomials
        .iter()
        .map(|p| p.clone().into_coeffs().len())
        .sum::<usize>());
    setup_frs += dbg!(setup
        .permutation_polynomials
        .iter()
        .map(|p| p.clone().into_coeffs().len())
        .sum::<usize>());
    dbg!(setup_frs);

    //
    // setup
    //     .write(std::io::BufWriter::with_capacity(
    //         1 << 29,
    //         std::fs::File::create("setup").unwrap(),
    //     ))
    //     .expect("setup write");

    // let timer = Instant::now();
    // let setup = SetupPolynomials::<_, PlonkCsWidth4WithNextStepParams>::read(
    //     std::io::BufReader::with_capacity(1 << 29, std::fs::File::open("setup").unwrap()),
    // )
    // .expect("setup read");
    // println!("setup read: {}", timer.elapsed().as_secs());

    println!("Made into {} gates", setup.n);
    let size = setup.n.next_power_of_two();
    let power_of_two = size.trailing_zeros();
    println!("power of two {}", power_of_two);

    // let timer = Instant::now();
    // let key_monomial_form = {
    //     let mut monomial_form_reader = std::io::BufReader::with_capacity(
    //         1 << 29,
    //         std::fs::File::open(format!(
    //             "{}/setup_2^{}.key",
    //             universal_setup_path, power_of_two
    //         ))
    //         .unwrap(),
    //     );
    //     Crs::<Bn256, CrsForMonomialForm>::read(&mut monomial_form_reader).unwrap()
    // };
    //
    // let key_lagrange_form = {
    //     let mut lagrange_form_reader = std::io::BufReader::with_capacity(
    //         1 << 29,
    //         std::fs::File::open(format!(
    //             "{}/setup_2^{}_lagrange.key",
    //             universal_setup_path, power_of_two
    //         ))
    //         .unwrap(),
    //     );
    //     Crs::<Bn256, CrsForLagrangeForm>::read(&mut lagrange_form_reader).unwrap()
    // };
    // println!("setup files read {}", timer.elapsed().as_secs());

    // let timer = Instant::now();
    // let verification_key =
    //     make_verification_key(&setup, &key_monomial_form).expect("must make a verification key");
    // println!("verification key done {}", timer.elapsed().as_secs());
    // verification_key
    //     .write(std::io::BufWriter::with_capacity(
    //         1 << 24,
    //         std::fs::File::create("ver_key").unwrap(),
    //     ))
    //     .expect("vk write");
    //
    let timer = Instant::now();
    let precomputations =
        make_precomputations(&setup).expect("must make precomputations for proving");
    println!("percomputation time: {}", timer.elapsed().as_secs());
    let mut total_frs: usize = dbg!(precomputations
        .selector_polynomials_on_coset_of_size_4n_bitreversed
        .into_iter()
        .map(|p| p.into_coeffs().len())
        .sum());
    total_frs += dbg!(precomputations
        .next_step_selector_polynomials_on_coset_of_size_4n_bitreversed
        .into_iter()
        .map(|p| p.into_coeffs().len())
        .sum::<usize>());
    total_frs += dbg!(precomputations
        .permutation_polynomials_on_coset_of_size_4n_bitreversed
        .into_iter()
        .map(|p| p.into_coeffs().len())
        .sum::<usize>());
    total_frs += dbg!(precomputations
        .permutation_polynomials_values_of_size_n_minus_one
        .into_iter()
        .map(|p| p.into_coeffs().len())
        .sum::<usize>());
    total_frs += dbg!(precomputations
        .inverse_divisor_on_coset_of_size_4n_bitreversed
        .into_coeffs()
        .len());
    total_frs += dbg!(precomputations
        .x_on_coset_of_size_4n_bitreversed
        .into_coeffs()
        .len());
    dbg!(total_frs);
    // precomputations
    //     .write(std::io::BufWriter::with_capacity(
    //         1 << 29,
    //         std::fs::File::create("precomp").unwrap(),
    //     ))
    //     .expect("precomp write");

    // let timer = Instant::now();
    // let omegas_bitreversed = BitReversedOmegas::<Fr>::new_for_domain_size(size);
    // let omegas_inv_bitreversed =
    //     <OmegasInvBitreversed<Fr> as CTPrecomputations<Fr>>::new_for_domain_size(size);
    // println!("omegas time: {}", timer.elapsed().as_secs());

    unimplemented!()
}

pub fn make_circuit_parameters_plonk(block_size: usize) {
    // let p_g = FixedGenerators::SpendingKeyGenerator;
    let jubjub_params = &params::JUBJUB_PARAMS;
    let rescue_params = &params::RESCUE_PARAMS;
    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

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
        rescue_params,
        jubjub_params,
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

    use crypto_exports::franklin_crypto::bellman::pairing::bn256::Bn256;
    use crypto_exports::franklin_crypto::bellman::kate_commitment::*;
    use crypto_exports::franklin_crypto::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
    use crypto_exports::franklin_crypto::bellman::plonk::*;
    use crypto_exports::franklin_crypto::bellman::worker::Worker;
    use crypto_exports::franklin_crypto::bellman::plonk::better_cs::{adaptor::{TranspilationVariant, write_transpilation_hints}, cs::PlonkCsWidth4WithNextStepParams};
    use crypto_exports::franklin_crypto::bellman::plonk::fft::cooley_tukey_ntt::*;

    let universal_setup_path: String = format!(
        "{}/keys/setup/",
        std::env::var("ZKSYNC_HOME").expect("ZKSYNC_HOME_ENV")
    );

    let timer = Instant::now();
    let hints = transpile::<Bn256, _>(instance_for_generation.clone())
        .expect("transpilation is successful");
    println!("Done transpiling {}", timer.elapsed().as_secs());

    let timer = Instant::now();
    let setup = setup(instance_for_generation.clone(), &hints).expect("must make setup");
    println!("setup generated: {}", timer.elapsed().as_secs());
    let mut setup_frs = 0;
    setup_frs += setup
        .selector_polynomials
        .iter()
        .map(|p| p.clone().into_coeffs().len())
        .sum::<usize>();
    setup_frs += setup
        .next_step_selector_polynomials
        .iter()
        .map(|p| p.clone().into_coeffs().len())
        .sum::<usize>();
    setup_frs += setup
        .permutation_polynomials
        .iter()
        .map(|p| p.clone().into_coeffs().len())
        .sum::<usize>();
    println!("setup frs {}", setup_frs);

    //
    // setup
    //     .write(std::io::BufWriter::with_capacity(
    //         1 << 29,
    //         std::fs::File::create("setup").unwrap(),
    //     ))
    //     .expect("setup write");

    // let timer = Instant::now();
    // let setup = SetupPolynomials::<_, PlonkCsWidth4WithNextStepParams>::read(
    //     std::io::BufReader::with_capacity(1 << 29, std::fs::File::open("setup").unwrap()),
    // )
    // .expect("setup read");
    // println!("setup read: {}", timer.elapsed().as_secs());

    let size = setup.n.next_power_of_two();
    let power_of_two = size.trailing_zeros();
    assert!(power_of_two <= 26, "too big");
    println!("power of two {}", power_of_two);

    // let timer = Instant::now();
    // let key_monomial_form = {
    //     let mut monomial_form_reader = std::io::BufReader::with_capacity(
    //         1 << 29,
    //         std::fs::File::open(format!(
    //             "{}/setup_2^{}.key",
    //             universal_setup_path, power_of_two
    //         ))
    //         .unwrap(),
    //     );
    //     Crs::<Bn256, CrsForMonomialForm>::read(&mut monomial_form_reader).unwrap()
    // };
    //
    // let key_lagrange_form = {
    //     let mut lagrange_form_reader = std::io::BufReader::with_capacity(
    //         1 << 29,
    //         std::fs::File::open(format!(
    //             "{}/setup_2^{}_lagrange.key",
    //             universal_setup_path, power_of_two
    //         ))
    //         .unwrap(),
    //     );
    //     Crs::<Bn256, CrsForLagrangeForm>::read(&mut lagrange_form_reader).unwrap()
    // };
    // println!("setup files read {}", timer.elapsed().as_secs());

    // let timer = Instant::now();
    // let verification_key =
    //     make_verification_key(&setup, &key_monomial_form).expect("must make a verification key");
    // println!("verification key done {}", timer.elapsed().as_secs());
    // verification_key
    //     .write(std::io::BufWriter::with_capacity(
    //         1 << 24,
    //         std::fs::File::create("ver_key").unwrap(),
    //     ))
    //     .expect("vk write");
    //
    let timer = Instant::now();
    let precomputations =
        make_precomputations(&setup).expect("must make precomputations for proving");
    println!("percomputation time: {}", timer.elapsed().as_secs());
    let mut precomp_fr: usize = precomputations
        .selector_polynomials_on_coset_of_size_4n_bitreversed
        .into_iter()
        .map(|p| p.into_coeffs().len())
        .sum();
    precomp_fr += precomputations
        .next_step_selector_polynomials_on_coset_of_size_4n_bitreversed
        .into_iter()
        .map(|p| p.into_coeffs().len())
        .sum::<usize>();
    precomp_fr += precomputations
        .permutation_polynomials_on_coset_of_size_4n_bitreversed
        .into_iter()
        .map(|p| p.into_coeffs().len())
        .sum::<usize>();
    precomp_fr += precomputations
        .permutation_polynomials_values_of_size_n_minus_one
        .into_iter()
        .map(|p| p.into_coeffs().len())
        .sum::<usize>();
    precomp_fr += precomputations
        .inverse_divisor_on_coset_of_size_4n_bitreversed
        .into_coeffs()
        .len();
    precomp_fr += precomputations
        .x_on_coset_of_size_4n_bitreversed
        .into_coeffs()
        .len();
    println!("precomp fr: {}", precomp_fr);
    // precomputations
    //     .write(std::io::BufWriter::with_capacity(
    //         1 << 29,
    //         std::fs::File::create("precomp").unwrap(),
    //     ))
    //     .expect("precomp write");

    // let timer = Instant::now();
    // let omegas_bitreversed = BitReversedOmegas::<Fr>::new_for_domain_size(size);
    // let omegas_inv_bitreversed =
    //     <OmegasInvBitreversed<Fr> as CTPrecomputations<Fr>>::new_for_domain_size(size);
    // println!("omegas time: {}", timer.elapsed().as_secs());
}

#[test]
fn run_make_circuit_parameters() {
    for n_chunks in [1].iter() {
        println!("n chunks {}", n_chunks);
        make_circuit_parameters_plonk(*n_chunks);
    }
    // Vec::<[u8; 128]>::with_capacity(4341320977169282661);
}

pub fn make_exit_circuit_parameters() -> Parameters<Bn256> {
    // let p_g = FixedGenerators::SpendingKeyGenerator;
    let jubjub_params = &params::JUBJUB_PARAMS;
    let rescue_params = &params::RESCUE_PARAMS;
    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let rng = &mut OsRng::new().unwrap();
    let exit_circuit_instance = ZksyncExitCircuit::<'_, Bn256> {
        params: rescue_params,
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
