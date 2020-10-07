use crate::playground::get_path_in_file_dump_dir;
use crate::witness::tests::test_utils::{WitnessTestAccount, ZkSyncStateGenerator};
use crate::witness::utils::WitnessBuilder;
use crate::witness::{deposit::DepositWitness, Witness};
use num::BigUint;
use rayon::prelude::*;
use std::time::Instant;
use zksync_crypto::Fr;
use zksync_types::prover_utils::fs_utils::{
    get_universal_setup_lagrange_form, get_universal_setup_monomial_form,
};
use zksync_types::{Deposit, DepositOp};

#[test]
fn test_transpile_deposit_franklin_existing_account() {
    let account = WitnessTestAccount::new_empty(1);

    let deposit_to_account_id = account.id;
    let deposit_to_account_address = account.account.address;
    let (mut plasma_state, mut circuit_account_tree) =
        ZkSyncStateGenerator::generate(&vec![account]);

    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: deposit_to_account_address,
            token: 0,
            amount: BigUint::from(1u32),
            to: deposit_to_account_address,
        },
        account_id: deposit_to_account_id,
    };

    plasma_state.apply_deposit_op(&deposit_op);

    let deposit_witness = DepositWitness::apply_tx(&mut witness_accum.account_tree, &deposit_op);
    let deposit_operations = deposit_witness.calculate_operations(());
    let pub_data_from_witness = deposit_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
    witness_accum.collect_fees(&Vec::new());
    witness_accum.calculate_pubdata_commitment();

    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
    use zksync_crypto::franklin_crypto::bellman::plonk::adaptor::alternative::*;
    use zksync_crypto::franklin_crypto::bellman::plonk::plonk::generator::*;
    use zksync_crypto::franklin_crypto::bellman::plonk::plonk::prover::*;
    use zksync_crypto::franklin_crypto::bellman::Circuit;

    let mut transpiler = Transpiler::new();

    let c = witness_accum.into_circuit_instance();

    c.clone().synthesize(&mut transpiler).unwrap();

    println!("Done transpiling");

    let hints = transpiler.into_hints();

    use zksync_crypto::franklin_crypto::bellman::plonk::cs::Circuit as PlonkCircuit;

    let adapted_curcuit = Adaptorcircuit::new(c.clone(), &hints);

    let mut assembly = GeneratorAssembly::<Bn256>::new();
    adapted_curcuit.synthesize(&mut assembly).unwrap();
    assembly.finalize();

    println!("Transpiled into {} gates", assembly.num_gates());

    println!("Trying to prove");

    let adapted_curcuit = Adaptorcircuit::new(c.clone(), &hints);

    let mut prover = ProvingAssembly::<Bn256>::new();
    adapted_curcuit.synthesize(&mut prover).unwrap();
    prover.finalize();

    println!("Checking if is satisfied");
    assert!(prover.is_satisfied());
}

#[test]
fn test_new_transpile_deposit_franklin_existing_account_validate_only() {
    const NUM_DEPOSITS: usize = 50;
    println!(
        "Testing for {} deposits {} chunks",
        NUM_DEPOSITS,
        DepositOp::CHUNKS * NUM_DEPOSITS
    );
    let account = WitnessTestAccount::new_empty(1);

    let deposit_to_account_id = account.id;
    let deposit_to_account_address = account.account.address;
    let (mut plasma_state, mut circuit_tree) = ZkSyncStateGenerator::generate(&vec![account]);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_tree, 0, 1);

    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: deposit_to_account_address,
            token: 0,
            amount: BigUint::from(1u32),
            to: deposit_to_account_address,
        },
        account_id: deposit_to_account_id,
    };

    for _ in 0..NUM_DEPOSITS {
        plasma_state.apply_deposit_op(&deposit_op);
        let deposit_witness = DepositWitness::apply_tx(witness_accum.account_tree, &deposit_op);
        let deposit_operations = deposit_witness.calculate_operations(());
        let pub_data_from_witness = deposit_witness.get_pubdata();

        witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
    }
    witness_accum.collect_fees(&Vec::new());
    witness_accum.calculate_pubdata_commitment();

    println!("pubdata commitment: {:?}", witness_accum.pubdata_commitment);
    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
    use zksync_crypto::franklin_crypto::bellman::plonk::better_cs::adaptor::*;
    use zksync_crypto::franklin_crypto::bellman::plonk::*;

    // let mut transpiler = Transpiler::new();

    let c = witness_accum.into_circuit_instance();

    // c.clone().synthesize(&mut transpiler).unwrap();

    println!("Start transpiling");
    let (n, mut hints) =
        transpile_with_gates_count::<Bn256, _>(c.clone()).expect("transpilation is successful");
    println!("Transpiled into {} gates", n);
    let mut tmp_buff = Vec::new();
    write_transpilation_hints(&hints, &mut tmp_buff).expect("hint write");
    hints = read_transpilation_hints(tmp_buff.as_slice()).expect("hint read");

    let mut hints_hist = std::collections::HashMap::new();
    hints_hist.insert("into addition gate".to_owned(), 0);
    hints_hist.insert("merge LC".to_owned(), 0);
    hints_hist.insert("into quadratic gate".to_owned(), 0);
    hints_hist.insert("into multiplication gate".to_owned(), 0);

    use zksync_crypto::franklin_crypto::bellman::plonk::better_cs::adaptor::TranspilationVariant;

    for (_, h) in hints.iter() {
        match h {
            TranspilationVariant::IntoQuadraticGate => {
                *hints_hist
                    .get_mut(&"into quadratic gate".to_owned())
                    .unwrap() += 1;
            }
            TranspilationVariant::MergeLinearCombinations(..) => {
                *hints_hist.get_mut(&"merge LC".to_owned()).unwrap() += 1;
            }
            TranspilationVariant::IntoAdditionGate(..) => {
                *hints_hist
                    .get_mut(&"into addition gate".to_owned())
                    .unwrap() += 1;
            }
            TranspilationVariant::IntoMultiplicationGate(..) => {
                *hints_hist
                    .get_mut(&"into multiplication gate".to_owned())
                    .unwrap() += 1;
            }
        }
    }

    println!("Transpilation hist = {:?}", hints_hist);

    println!("Done transpiling");

    is_satisfied_using_one_shot_check(c.clone(), &hints).expect("must validate");

    println!("Done checking if satisfied using one-shot");

    is_satisfied(c.clone(), &hints).expect("must validate");

    println!("Done checking if satisfied");
}

#[test]
fn test_new_transpile_deposit_franklin_existing_account() {
    const NUM_DEPOSITS: usize = 1;
    println!(
        "Testing for {} deposits {} chunks",
        NUM_DEPOSITS,
        DepositOp::CHUNKS * NUM_DEPOSITS
    );
    let account = WitnessTestAccount::new_empty(1);

    let deposit_to_account_id = account.id;
    let deposit_to_account_address = account.account.address;
    let (mut plasma_state, mut circuit_account_tree) =
        ZkSyncStateGenerator::generate(&vec![account]);
    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: deposit_to_account_address,
            token: 0,
            amount: BigUint::from(1u32),
            to: deposit_to_account_address,
        },
        account_id: deposit_to_account_id,
    };

    for _ in 0..NUM_DEPOSITS {
        plasma_state.apply_deposit_op(&deposit_op);
        let deposit_witness =
            DepositWitness::apply_tx(&mut witness_accum.account_tree, &deposit_op);
        let deposit_operations = deposit_witness.calculate_operations(());
        let pub_data_from_witness = deposit_witness.get_pubdata();

        witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
    }
    witness_accum.collect_fees(&Vec::new());
    witness_accum.calculate_pubdata_commitment();

    println!("pubdata commitment: {:?}", witness_accum.pubdata_commitment);
    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
    use zksync_crypto::franklin_crypto::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
    use zksync_crypto::franklin_crypto::bellman::plonk::*;

    // let mut transpiler = Transpiler::new();

    let c = witness_accum.into_circuit_instance();

    // c.clone().synthesize(&mut transpiler).unwrap();

    let timer = Instant::now();
    let (n, hints) =
        transpile_with_gates_count::<Bn256, _>(c.clone()).expect("transpilation is successful");
    println!("Transpilation time: {}s", timer.elapsed().as_secs());
    println!("Transpiled into {} gates", n);

    let mut hints_hist = std::collections::HashMap::new();
    hints_hist.insert("into addition gate".to_owned(), 0);
    hints_hist.insert("merge LC".to_owned(), 0);
    hints_hist.insert("into quadratic gate".to_owned(), 0);
    hints_hist.insert("into multiplication gate".to_owned(), 0);

    use zksync_crypto::franklin_crypto::bellman::plonk::better_cs::adaptor::TranspilationVariant;

    for (_, h) in hints.iter() {
        match h {
            TranspilationVariant::IntoQuadraticGate => {
                *hints_hist
                    .get_mut(&"into quadratic gate".to_owned())
                    .unwrap() += 1;
            }
            TranspilationVariant::MergeLinearCombinations(..) => {
                *hints_hist.get_mut(&"merge LC".to_owned()).unwrap() += 1;
            }
            TranspilationVariant::IntoAdditionGate(..) => {
                *hints_hist
                    .get_mut(&"into addition gate".to_owned())
                    .unwrap() += 1;
            }
            TranspilationVariant::IntoMultiplicationGate(..) => {
                *hints_hist
                    .get_mut(&"into multiplication gate".to_owned())
                    .unwrap() += 1;
            }
        }
    }

    println!("Transpilation hist = {:?}", hints_hist);

    println!("Done transpiling");

    is_satisfied_using_one_shot_check(c.clone(), &hints).expect("must validate");

    println!("Done checking if satisfied using one-shot");

    is_satisfied(c.clone(), &hints).expect("must validate");

    println!("Done checking if satisfied");

    let timer = Instant::now();
    let setup = setup(c.clone(), &hints).expect("must make setup");
    println!("Setup generated time: {}s", timer.elapsed().as_secs());

    println!("Made into {} gates", setup.n);
    let size = setup.n.next_power_of_two();
    let size_log2 = size.trailing_zeros();
    println!("Log pow2: {}", size_log2);
    assert!(size_log2 <= 26, "power of two too big");

    let timer = Instant::now();
    let key_monomial_form = get_universal_setup_monomial_form(size_log2).unwrap();
    let key_lagrange_form = get_universal_setup_lagrange_form(size_log2).unwrap();
    println!("Setup files read: {}s", timer.elapsed().as_secs());

    // let worker = Worker::new();

    // let key_monomial_form = Crs::<Bn256, CrsForMonomialForm>::crs_42(size, &worker);
    // let key_lagrange_form = Crs::<Bn256, CrsForLagrangeForm>::from_powers(&key_monomial_form, size, &worker);

    // let key_monomial_form = Crs::<Bn256, CrsForMonomialForm>::dummy_crs(size);
    // let key_lagrange_form = Crs::<Bn256, CrsForLagrangeForm>::dummy_crs(size);

    let timer = Instant::now();
    let verification_key =
        make_verification_key(&setup, &key_monomial_form).expect("must make a verification key");
    println!("Verification key generated: {}s", timer.elapsed().as_secs());

    // tmp_buff = Vec::new();
    verification_key
        .write(
            std::fs::File::create(get_path_in_file_dump_dir("verification.key"))
                .expect("ver key file create"),
        )
        .expect("ver key serialize");

    let timer = Instant::now();
    let precomputations =
        make_precomputations(&setup).expect("must make precomputations for proving");
    println!("Precomputations generated: {}s", timer.elapsed().as_secs());

    use zksync_crypto::franklin_crypto::bellman::plonk::fft::cooley_tukey_ntt::*;

    let omegas_bitreversed = BitReversedOmegas::<Fr>::new_for_domain_size(size);
    let omegas_inv_bitreversed =
        <OmegasInvBitreversed<Fr> as CTPrecomputations<Fr>>::new_for_domain_size(size);

    let timer = Instant::now();
    let proof = prove_from_recomputations::<_, _, RollingKeccakTranscript<Fr>, _, _>(
        c.clone(),
        &hints,
        &setup,
        &precomputations,
        &omegas_bitreversed,
        &omegas_inv_bitreversed,
        &key_monomial_form,
        &key_lagrange_form,
    )
    .expect("must make a proof");
    println!("Proof generated: {}s", timer.elapsed().as_secs());

    proof
        .write(std::fs::File::create(get_path_in_file_dump_dir("deposit_proof.proof")).unwrap())
        .expect("proof write");

    let is_valid = verify::<_, RollingKeccakTranscript<Fr>>(&proof, &verification_key)
        .expect("must perform verification");
    assert!(is_valid);
}

#[test]
fn test_fma_transpile_deposit_franklin_existing_account() {
    const NUM_DEPOSITS: usize = 1;
    println!(
        "Testing for {} deposits {} chunks",
        NUM_DEPOSITS,
        DepositOp::CHUNKS * NUM_DEPOSITS
    );
    let account = WitnessTestAccount::new_empty(1);

    let deposit_to_account_id = account.id;
    let deposit_to_account_address = account.account.address;
    let (mut plasma_state, mut circuit_tree) = ZkSyncStateGenerator::generate(&vec![account]);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_tree, 0, 1);

    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: deposit_to_account_address,
            token: 0,
            amount: BigUint::from(1u32),
            to: deposit_to_account_address,
        },
        account_id: deposit_to_account_id,
    };

    for _ in 0..NUM_DEPOSITS {
        plasma_state.apply_deposit_op(&deposit_op);
        let deposit_witness =
            DepositWitness::apply_tx(&mut witness_accum.account_tree, &deposit_op);
        let deposit_operations = deposit_witness.calculate_operations(());
        let pub_data_from_witness = deposit_witness.get_pubdata();

        witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
    }
    witness_accum.collect_fees(&Vec::new());
    witness_accum.calculate_pubdata_commitment();

    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
    use zksync_crypto::franklin_crypto::bellman::plonk::better_cs::cs::PlonkCsWidth4WithNextStepParams;
    use zksync_crypto::franklin_crypto::bellman::plonk::better_cs::fma_adaptor::Transpiler;

    let mut transpiler = Transpiler::<Bn256, PlonkCsWidth4WithNextStepParams>::new();

    let c = witness_accum.into_circuit_instance();

    use zksync_crypto::franklin_crypto::bellman::Circuit;
    c.clone().synthesize(&mut transpiler).unwrap();

    let hints = transpiler.into_hints();

    let mut hints_hist = std::collections::HashMap::new();
    hints_hist.insert("into addition gate".to_owned(), 0);
    hints_hist.insert("merge LC".to_owned(), 0);
    hints_hist.insert("into quadratic gate".to_owned(), 0);
    hints_hist.insert("into multiplication gate".to_owned(), 0);
    hints_hist.insert("into fma gate".to_owned(), 0);

    use zksync_crypto::franklin_crypto::bellman::plonk::better_cs::fma_adaptor::TranspilationVariant;

    for (_, h) in hints.iter() {
        match h {
            TranspilationVariant::IntoQuadraticGate => {
                *hints_hist
                    .get_mut(&"into quadratic gate".to_owned())
                    .unwrap() += 1;
            }
            TranspilationVariant::MergeLinearCombinations(..) => {
                *hints_hist.get_mut(&"merge LC".to_owned()).unwrap() += 1;
            }
            TranspilationVariant::IntoAdditionGate(..) => {
                *hints_hist
                    .get_mut(&"into addition gate".to_owned())
                    .unwrap() += 1;
            }
            TranspilationVariant::IntoMultiplicationGate(..) => {
                *hints_hist
                    .get_mut(&"into multiplication gate".to_owned())
                    .unwrap() += 1;
            }
            TranspilationVariant::IntoFMAGate(..) => {
                *hints_hist.get_mut(&"into fma gate".to_owned()).unwrap() += 1;
            }
        }
    }

    println!("Transpilation hist = {:?}", hints_hist);

    println!("Done transpiling");
}

#[test]
fn print_available_setup_powers() {
    use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
    use zksync_crypto::franklin_crypto::bellman::plonk::*;

    let calculate_setup_power = |chunks: usize| -> (usize, u32) {
        let circuit = {
            let (_, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&[]);
            let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, 0, 1);
            witness_accum.extend_pubdata_with_noops(chunks);
            witness_accum.collect_fees(&[]);
            witness_accum.calculate_pubdata_commitment();
            witness_accum.into_circuit_instance()
        };
        let (gates, setup_power) = {
            let (gates, _) = transpile_with_gates_count::<Bn256, _>(circuit.clone())
                .expect("transpilation is successful");
            let size = gates.next_power_of_two();
            (gates, size.trailing_zeros())
        };
        (gates, setup_power)
    };

    println!("chunks,gates,setup_power");
    for chunk_range in (2..=750).step_by(2).collect::<Vec<_>>().chunks(32) {
        let mut chunk_data = chunk_range
            .into_par_iter()
            .map(|chunk| {
                let (gates, setup_power) = calculate_setup_power(*chunk);
                (*chunk, gates, setup_power)
            })
            .collect::<Vec<_>>();

        let is_finished = chunk_data
            .iter()
            .find(|(_, _, setup_power)| *setup_power > 26)
            .is_some();

        chunk_data.retain(|&(_, _, setup_power)| setup_power <= 26);
        for (chunks, gates, setup_power) in chunk_data {
            println!("{},{},{}", chunks, gates, setup_power);
        }
        if is_finished {
            break;
        }
    }
}

#[test]
fn test_playground() {
    std::fs::File::create(get_path_in_file_dump_dir("test_dump.txt")).unwrap();
}
