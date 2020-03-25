use super::utils::*;

use crate::franklin_crypto::bellman::pairing::bn256::*;
use crate::franklin_crypto::bellman::pairing::ff::{Field, PrimeField};
use crate::franklin_crypto::jubjub::JubjubEngine;
use crate::operation::SignatureData;
use crate::operation::*;
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{
    append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element,
};
use models::node::DepositOp;
use models::params as franklin_constants;

pub struct DepositData {
    pub amount: u128,
    pub token: u32,
    pub account_address: u32,
    pub address: Fr,
}
pub struct DepositWitness<E: JubjubEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}
impl<E: JubjubEngine> DepositWitness<E> {
    pub fn get_pubdata(&self) -> Vec<bool> {
        let mut pubdata_bits = vec![];
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.tx_type.unwrap(),
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.address.unwrap(),
            franklin_constants::ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.full_amount.unwrap(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            franklin_constants::ETH_ADDRESS_BIT_WIDTH,
        );
        //        assert_eq!(pubdata_bits.len(), 37 * 8);
        pubdata_bits.resize(6 * franklin_constants::CHUNK_BIT_WIDTH, false);
        pubdata_bits
    }

    // CLARIFY: What? Why?
    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("1").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.new_pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.full_amount.unwrap(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}

pub fn apply_deposit_tx(
    tree: &mut CircuitAccountTree,
    deposit: &DepositOp,
) -> DepositWitness<Bn256> {
    let deposit_data = DepositData {
        amount: deposit.priority_op.amount.to_string().parse().unwrap(),
        token: u32::from(deposit.priority_op.token),
        account_address: deposit.account_id,
        address: eth_address_to_fr(&deposit.priority_op.to),
    };
    apply_deposit(tree, &deposit_data)
}
pub fn apply_deposit(
    tree: &mut CircuitAccountTree,
    deposit: &DepositData,
) -> DepositWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    debug!("deposit Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, deposit.account_address, deposit.token);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_address_fe = Fr::from_str(&deposit.account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&deposit.token.to_string()).unwrap();
    let amount_as_field_element = Fr::from_str(&deposit.amount.to_string()).unwrap();
    debug!("amount_as_field_element is: {}", amount_as_field_element);
    //calculate a and b
    let a = amount_as_field_element;
    let b = Fr::zero();

    //applying deposit
    let (account_witness_before, account_witness_after, balance_before, balance_after) =
        apply_leaf_operation(
            tree,
            deposit.account_address,
            deposit.token,
            |acc| {
                assert!((acc.address == deposit.address) || (acc.address == Fr::zero()));
                acc.address = deposit.address;
            },
            |bal| bal.value.add_assign(&amount_as_field_element),
        );

    let after_root = tree.root_hash();
    debug!("deposit After root = {}", after_root);
    let (audit_path_after, audit_balance_path_after) =
        get_audits(tree, deposit.account_address, deposit.token);

    DepositWitness {
        before: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_before,
                account_path: audit_path_before,
                balance_value: Some(balance_before),
                balance_subtree_path: audit_balance_path_before,
            },
        },
        after: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_after,
                account_path: audit_path_after,
                balance_value: Some(balance_after),
                balance_subtree_path: audit_balance_path_after,
            },
        },
        args: OperationArguments {
            eth_address: Some(deposit.address),
            amount_packed: Some(Fr::zero()),
            full_amount: Some(amount_as_field_element),
            fee: Some(Fr::zero()),
            a: Some(a),
            b: Some(b),
            pub_nonce: Some(Fr::zero()),
            new_pub_key_hash: Some(Fr::zero()),
        },
        before_root: Some(before_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("1").unwrap()),
    }
}

pub fn calculate_deposit_operations_from_witness(
    deposit_witness: &DepositWitness<Bn256>,
) -> Vec<Operation<Bn256>> {
    let first_sig_msg = &Fr::zero();
    let second_sig_msg = &Fr::zero();
    let third_sig_msg = &Fr::zero();
    let signature_data = &SignatureData::init_empty();
    let signer_pub_key_packed = &[Some(false); 256]; //doesn't matter for deposit
    let pubdata_chunks: Vec<_> = deposit_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();

    debug!(
        "acc_path{} \n bal_path {} ",
        deposit_witness.before.witness.account_path.len(),
        deposit_witness.before.witness.balance_subtree_path.len()
    );
    let operation_zero = Operation {
        new_root: deposit_witness.after_root,
        tx_type: deposit_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.before.clone(),
        rhs: deposit_witness.before.clone(),
    };

    let operation_one = Operation {
        new_root: deposit_witness.after_root,
        tx_type: deposit_witness.tx_type,
        chunk: Some(Fr::from_str("1").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[1]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.after.clone(),
        rhs: deposit_witness.after.clone(),
    };

    let operation_two = Operation {
        new_root: deposit_witness.after_root,
        tx_type: deposit_witness.tx_type,
        chunk: Some(Fr::from_str("2").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[2]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.after.clone(),
        rhs: deposit_witness.after.clone(),
        signature_data: signature_data.clone(),
    };

    let operation_three = Operation {
        new_root: deposit_witness.after_root,
        tx_type: deposit_witness.tx_type,
        chunk: Some(Fr::from_str("3").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[3]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.after.clone(),
        rhs: deposit_witness.after.clone(),
        signature_data: signature_data.clone(),
    };

    let operation_four = Operation {
        new_root: deposit_witness.after_root,
        tx_type: deposit_witness.tx_type,
        chunk: Some(Fr::from_str("4").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[4]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.after.clone(),
        rhs: deposit_witness.after.clone(),
        signature_data: signature_data.clone(),
    };

    let operation_five = Operation {
        new_root: deposit_witness.after_root,
        tx_type: deposit_witness.tx_type,
        chunk: Some(Fr::from_str("5").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[5]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.after.clone(),
        rhs: deposit_witness.after.clone(),
        signature_data: signature_data.clone(),
    };
    let operations: Vec<Operation<_>> = vec![
        operation_zero,
        operation_one,
        operation_two,
        operation_three,
        operation_four,
        operation_five,
    ];
    operations
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::witness::test_utils::{check_circuit, test_genesis_plasma_state};
    use bigdecimal::BigDecimal;
    use models::node::{Account, Deposit};

    #[test]
    #[ignore]
    fn test_deposit_in_empty_leaf() {
        let (mut plasma_state, mut witness_accum) = test_genesis_plasma_state(Vec::new());

        let empty_account_id = 1;
        let empty_account_address = [7u8; 20].into();
        let deposit_op = DepositOp {
            priority_op: Deposit {
                from: empty_account_address,
                token: 0,
                amount: BigDecimal::from(1),
                to: empty_account_address,
            },
            account_id: empty_account_id,
        };

        println!(
            "node root hash before deposit: {:?}",
            plasma_state.root_hash()
        );
        plasma_state.apply_deposit_op(&deposit_op);
        println!(
            "node root hash after deposit: {:?}",
            plasma_state.root_hash()
        );
        println!(
            "node pub data: {}",
            hex::encode(&deposit_op.get_public_data())
        );

        let deposit_witness = apply_deposit_tx(&mut witness_accum.account_tree, &deposit_op);
        let deposit_operations = calculate_deposit_operations_from_witness(&deposit_witness);
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

        check_circuit(witness_accum.into_circuit_instance());
    }

    #[test]
    #[ignore]
    fn test_deposit_existing_account() {
        let deposit_to_account_id = 1;
        let deposit_to_account_address =
            "1111111111111111111111111111111111111111".parse().unwrap();
        let (mut plasma_state, mut witness_accum) = test_genesis_plasma_state(vec![(
            deposit_to_account_id,
            Account::default_with_address(&deposit_to_account_address),
        )]);

        let deposit_op = DepositOp {
            priority_op: Deposit {
                from: deposit_to_account_address,
                token: 0,
                amount: BigDecimal::from(1),
                to: deposit_to_account_address,
            },
            account_id: deposit_to_account_id,
        };

        println!("node root hash before op: {:?}", plasma_state.root_hash());
        plasma_state.apply_deposit_op(&deposit_op);
        println!("node root hash after op: {:?}", plasma_state.root_hash());

        let deposit_witness = apply_deposit_tx(&mut witness_accum.account_tree, &deposit_op);
        let deposit_operations = calculate_deposit_operations_from_witness(&deposit_witness);
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

        check_circuit(witness_accum.into_circuit_instance());
    }

    #[test]
    #[ignore]
    fn test_transpile_deposit_franklin_existing_account() {
        let deposit_to_account_id = 1;
        let deposit_to_account_address =
            "1111111111111111111111111111111111111111".parse().unwrap();
        let (mut plasma_state, mut witness_accum) = test_genesis_plasma_state(vec![(
            deposit_to_account_id,
            Account::default_with_address(&deposit_to_account_address),
        )]);

        let deposit_op = DepositOp {
            priority_op: Deposit {
                from: deposit_to_account_address,
                token: 0,
                amount: BigDecimal::from(1),
                to: deposit_to_account_address,
            },
            account_id: deposit_to_account_id,
        };

        plasma_state.apply_deposit_op(&deposit_op);

        let deposit_witness = apply_deposit_tx(&mut witness_accum.account_tree, &deposit_op);
        let deposit_operations = calculate_deposit_operations_from_witness(&deposit_witness);
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

        use crate::franklin_crypto::bellman::pairing::bn256::Bn256;
        use crate::franklin_crypto::bellman::plonk::adaptor::alternative::*;
        use crate::franklin_crypto::bellman::plonk::plonk::generator::*;
        use crate::franklin_crypto::bellman::plonk::plonk::prover::*;

        use crate::franklin_crypto::bellman::Circuit;

        let mut transpiler = Transpiler::new();

        let c = witness_accum.into_circuit_instance();

        c.clone().synthesize(&mut transpiler).unwrap();

        println!("Done transpiling");

        let hints = transpiler.into_hints();

        use crate::franklin_crypto::bellman::plonk::cs::Circuit as PlonkCircuit;

        let adapted_curcuit = AdaptorCircuit::new(c.clone(), &hints);

        let mut assembly = GeneratorAssembly::<Bn256>::new();
        adapted_curcuit.synthesize(&mut assembly).unwrap();
        assembly.finalize();

        println!("Transpiled into {} gates", assembly.num_gates());

        println!("Trying to prove");

        let adapted_curcuit = AdaptorCircuit::new(c.clone(), &hints);

        let mut prover = ProvingAssembly::<Bn256>::new();
        adapted_curcuit.synthesize(&mut prover).unwrap();
        prover.finalize();

        println!("Checking if is satisfied");
        assert!(prover.is_satisfied());
    }

    #[test]
    // #[ignore]
    fn test_new_transpile_deposit_franklin_existing_account() {
        let universal_setup_path: String = format!(
            "{}/keys/setup/",
            std::env::var("ZKSYNC_HOME").expect("ZKSYNC_HOME_ENV")
        );
        const NUM_DEPOSITS: usize = 1;
        println!(
            "Testing for {} deposits {} chunks",
            NUM_DEPOSITS,
            DepositOp::CHUNKS * NUM_DEPOSITS
        );
        let deposit_to_account_id = 1;
        let deposit_to_account_address =
            "1111111111111111111111111111111111111111".parse().unwrap();
        let (mut plasma_state, mut witness_accum) = test_genesis_plasma_state(vec![(
            deposit_to_account_id,
            Account::default_with_address(&deposit_to_account_address),
        )]);

        let deposit_op = DepositOp {
            priority_op: Deposit {
                from: deposit_to_account_address,
                token: 0,
                amount: BigDecimal::from(1),
                to: deposit_to_account_address,
            },
            account_id: deposit_to_account_id,
        };

        for _ in 0..NUM_DEPOSITS {
            plasma_state.apply_deposit_op(&deposit_op);
            let deposit_witness = apply_deposit_tx(&mut witness_accum.account_tree, &deposit_op);
            let deposit_operations = calculate_deposit_operations_from_witness(&deposit_witness);
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

        use crate::franklin_crypto::bellman::pairing::bn256::Bn256;
        // use crate::franklin_crypto::bellman::plonk::better_cs::adaptor::*;
        // use crate::franklin_crypto::bellman::plonk::better_cs::cs::Circuit as PlonkCircuit;
        use crate::franklin_crypto::bellman::kate_commitment::*;
        use crate::franklin_crypto::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
        use crate::franklin_crypto::bellman::plonk::*;
        use crate::franklin_crypto::bellman::worker::Worker;

        // let mut transpiler = Transpiler::new();

        let c = witness_accum.into_circuit_instance();

        // c.clone().synthesize(&mut transpiler).unwrap();

        let hints = transpile::<Bn256, _>(c.clone()).expect("transpilation is successful");

        let mut hints_hist = std::collections::HashMap::new();
        hints_hist.insert("into addition gate".to_owned(), 0);
        hints_hist.insert("merge LC".to_owned(), 0);
        hints_hist.insert("into quadratic gate".to_owned(), 0);
        hints_hist.insert("into multiplication gate".to_owned(), 0);

        use crate::franklin_crypto::bellman::plonk::better_cs::adaptor::TranspilationVariant;

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

        let setup = setup(c.clone(), &hints).expect("must make setup");

        println!("Made into {} gates", setup.n);
        let size = setup.n.next_power_of_two();

        let mut monomial_form_reader = std::io::BufReader::with_capacity(
            1 << 24,
            std::fs::File::open(format!("{}/setup_2^22.key", universal_setup_path)).unwrap(),
        );

        let mut lagrange_form_reader = std::io::BufReader::with_capacity(
            1 << 24,
            std::fs::File::open(format!("{}/setup_2^22_lagrange.key", universal_setup_path))
                .unwrap(),
        );

        let key_monomial_form =
            Crs::<Bn256, CrsForMonomialForm>::read(&mut monomial_form_reader).unwrap();
        let key_lagrange_form =
            Crs::<Bn256, CrsForLagrangeForm>::read(&mut lagrange_form_reader).unwrap();

        // let worker = Worker::new();

        // let key_monomial_form = Crs::<Bn256, CrsForMonomialForm>::crs_42(size, &worker);
        // let key_lagrange_form = Crs::<Bn256, CrsForLagrangeForm>::from_powers(&key_monomial_form, size, &worker);

        // let key_monomial_form = Crs::<Bn256, CrsForMonomialForm>::dummy_crs(size);
        // let key_lagrange_form = Crs::<Bn256, CrsForLagrangeForm>::dummy_crs(size);

        let verification_key = make_verification_key(&setup, &key_monomial_form)
            .expect("must make a verification key");

        let mut key_writer = std::io::BufWriter::with_capacity(
            1 << 24,
            std::fs::File::create("./deposit_vk.key").unwrap(),
        );

        verification_key
            .write(&mut key_writer)
            .expect("must write a verification key");
        drop(key_writer);

        let precomputations =
            make_precomputations(&setup).expect("must make precomputations for proving");

        use crate::franklin_crypto::bellman::plonk::fft::cooley_tukey_ntt::*;

        let omegas_bitreversed = BitReversedOmegas::<Fr>::new_for_domain_size(size);
        let omegas_inv_bitreversed =
            <OmegasInvBitreversed<Fr> as CTPrecomputations<Fr>>::new_for_domain_size(size);

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

        let is_valid = verify::<_, RollingKeccakTranscript<Fr>>(&proof, &verification_key)
            .expect("must perform verification");
        assert!(is_valid);

        let (inputs, proof) = serialize_proof::serialize_proof(&proof);
        println!("Inputs");
        let mut vec = vec![];
        for i in inputs.into_iter() {
            vec.push(format!("\"{}\"", i));
        }
        println!("[{}]", vec.join(","));
        println!("Proof");
        let mut vec = vec![];
        for i in proof.into_iter() {
            vec.push(format!("\"{}\"", i));
        }
        println!("[{}]", vec.join(","));
    }
}

mod serialize_proof {
    use crypto_exports::bellman;
    use crypto_exports::bellman::pairing::bn256::{Bn256, Fr};
    use crypto_exports::bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
    use crypto_exports::bellman::pairing::{CurveAffine, Engine};
    use crypto_exports::bellman::plonk::better_cs::cs::PlonkCsWidth4WithNextStepParams;
    use crypto_exports::bellman::plonk::better_cs::keys::{Proof, VerificationKey};

    use handlebars::*;

    use serde_json::value::Map;

    use web3::types::U256;

    pub fn render_verification_key(
        vk: &VerificationKey<Bn256, PlonkCsWidth4WithNextStepParams>,
        render_to_path: &str,
    ) {
        let mut map = Map::new();

        let domain_size = vk.n.next_power_of_two().to_string();
        map.insert("domain_size".to_owned(), to_json(domain_size));

        let num_inputs = vk.num_inputs.to_string();
        map.insert("num_inputs".to_owned(), to_json(num_inputs));

        let domain =
            bellman::plonk::domains::Domain::<Fr>::new_for_size(vk.n.next_power_of_two() as u64)
                .unwrap();
        let omega = domain.generator;
        map.insert("omega".to_owned(), to_json(render_scalar_to_hex(&omega)));

        for (i, c) in vk.selector_commitments.iter().enumerate() {
            let rendered = render_g1_affine_to_hex::<Bn256>(&c);

            for j in 0..2 {
                map.insert(
                    format!("selector_commitment_{}_{}", i, j),
                    to_json(&rendered[j]),
                );
            }
        }

        for (i, c) in vk.next_step_selector_commitments.iter().enumerate() {
            let rendered = render_g1_affine_to_hex::<Bn256>(&c);

            for j in 0..2 {
                map.insert(
                    format!("next_step_selector_commitment_{}_{}", i, j),
                    to_json(&rendered[j]),
                );
            }
        }

        for (i, c) in vk.permutation_commitments.iter().enumerate() {
            let rendered = render_g1_affine_to_hex::<Bn256>(&c);

            for j in 0..2 {
                map.insert(
                    format!("permutation_commitment_{}_{}", i, j),
                    to_json(&rendered[j]),
                );
            }
        }

        for (i, c) in vk.non_residues.iter().enumerate() {
            let rendered = render_scalar_to_hex::<Fr>(&c);

            map.insert(format!("permutation_non_residue_{}", i), to_json(&rendered));
        }

        let rendered = render_g2_affine_to_hex(&vk.g2_elements[1]);

        map.insert("g2_x_x_c0".to_owned(), to_json(&rendered[0]));
        map.insert("g2_x_x_c1".to_owned(), to_json(&rendered[1]));
        map.insert("g2_x_y_c0".to_owned(), to_json(&rendered[2]));
        map.insert("g2_x_y_c1".to_owned(), to_json(&rendered[3]));

        let mut handlebars = Handlebars::new();

        // register template from a file and assign a name to it
        handlebars
            .register_template_file("contract", "./template.sol")
            .expect("must read the template");

        // make data and render it
        // println!("{}", handlebars.render("contract", &map).unwrap());

        let mut writer = std::io::BufWriter::with_capacity(
            1 << 24,
            std::fs::File::create(render_to_path).unwrap(),
        );

        let rendered = handlebars.render("contract", &map).unwrap();

        use std::io::Write;
        writer
            .write(rendered.as_bytes())
            .expect("must write to file");
    }

    fn render_scalar_to_hex<F: PrimeField>(el: &F) -> String {
        let mut buff = vec![];
        let repr = el.into_repr();
        repr.write_be(&mut buff).unwrap();

        format!("0x{}", hex::encode(buff))
    }

    fn render_g1_affine_to_hex<E: Engine>(point: &E::G1Affine) -> [String; 2] {
        if point.is_zero() {
            return ["0x0".to_owned(), "0x0".to_owned()];
        }

        let (x, y) = point.into_xy_unchecked();
        [render_scalar_to_hex(&x), render_scalar_to_hex(&y)]
    }

    fn render_g2_affine_to_hex(point: &<Bn256 as Engine>::G2Affine) -> [String; 4] {
        if point.is_zero() {
            return [
                "0x0".to_owned(),
                "0x0".to_owned(),
                "0x0".to_owned(),
                "0x0".to_owned(),
            ];
        }

        let (x, y) = point.into_xy_unchecked();

        [
            render_scalar_to_hex(&x.c0),
            render_scalar_to_hex(&x.c1),
            render_scalar_to_hex(&y.c0),
            render_scalar_to_hex(&y.c1),
        ]
    }

    fn serialize_g1_for_ethereum(point: &<Bn256 as Engine>::G1Affine) -> (U256, U256) {
        if point.is_zero() {
            return (U256::zero(), U256::zero());
        }
        let uncompressed = point.into_uncompressed();

        let uncompressed_slice = uncompressed.as_ref();

        // bellman serializes points as big endian and in the form x, y
        // ethereum expects the same order in memory
        let x = U256::from_big_endian(&uncompressed_slice[0..32]);
        let y = U256::from_big_endian(&uncompressed_slice[32..64]);

        (x, y)
    }

    fn serialize_fe_for_ethereum(field_element: &Fr) -> U256 {
        let mut be_bytes = [0u8; 32];
        field_element
            .into_repr()
            .write_be(&mut be_bytes[..])
            .expect("get new root BE bytes");
        U256::from_big_endian(&be_bytes[..])
    }

    pub fn serialize_proof(
        proof: &Proof<Bn256, PlonkCsWidth4WithNextStepParams>,
    ) -> (Vec<U256>, Vec<U256>) {
        let mut inputs = vec![];
        for input in proof.input_values.iter() {
            inputs.push(serialize_fe_for_ethereum(&input));
        }
        let mut serialized_proof = vec![];

        for c in proof.wire_commitments.iter() {
            let (x, y) = serialize_g1_for_ethereum(&c);
            serialized_proof.push(x);
            serialized_proof.push(y);
        }

        let (x, y) = serialize_g1_for_ethereum(&proof.grand_product_commitment);
        serialized_proof.push(x);
        serialized_proof.push(y);

        for c in proof.quotient_poly_commitments.iter() {
            let (x, y) = serialize_g1_for_ethereum(&c);
            serialized_proof.push(x);
            serialized_proof.push(y);
        }

        for c in proof.wire_values_at_z.iter() {
            serialized_proof.push(serialize_fe_for_ethereum(&c));
        }

        for c in proof.wire_values_at_z_omega.iter() {
            serialized_proof.push(serialize_fe_for_ethereum(&c));
        }

        serialized_proof.push(serialize_fe_for_ethereum(&proof.grand_product_at_z_omega));
        serialized_proof.push(serialize_fe_for_ethereum(&proof.quotient_polynomial_at_z));
        serialized_proof.push(serialize_fe_for_ethereum(
            &proof.linearization_polynomial_at_z,
        ));

        for c in proof.permutation_polynomials_at_z.iter() {
            serialized_proof.push(serialize_fe_for_ethereum(&c));
        }

        let (x, y) = serialize_g1_for_ethereum(&proof.opening_at_z_proof);
        serialized_proof.push(x);
        serialized_proof.push(y);

        let (x, y) = serialize_g1_for_ethereum(&proof.opening_at_z_omega_proof);
        serialized_proof.push(x);
        serialized_proof.push(y);

        (inputs, serialized_proof)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn render_key() {
            let mut reader = std::io::BufReader::with_capacity(
                1 << 24,
                std::fs::File::open("./deposit_vk.key").unwrap(),
            );
            let vk = VerificationKey::<Bn256, PlonkCsWidth4WithNextStepParams>::read(&mut reader)
                .unwrap();
            render_verification_key(&vk, "../Verifier.sol");
        }

        #[test]
        fn render_simple_xor_key_and_proof() {
            let mut reader = std::io::BufReader::with_capacity(
                1 << 24,
                std::fs::File::open("./xor_vk.key").unwrap(),
            );
            let vk = VerificationKey::<Bn256, PlonkCsWidth4WithNextStepParams>::read(&mut reader)
                .unwrap();
            render_verification_key(&vk, "./xor.sol");

            let mut reader = std::io::BufReader::with_capacity(
                1 << 24,
                std::fs::File::open("./xor_proof.proof").unwrap(),
            );
            let proof = Proof::<Bn256, PlonkCsWidth4WithNextStepParams>::read(&mut reader).unwrap();
            let (inputs, proof) = serialize_proof(&proof);

            println!("Inputs");
            let mut vec = vec![];
            for i in inputs.into_iter() {
                vec.push(format!("\"{}\"", i));
            }
            println!("[{}]", vec.join(","));

            println!("Proof");
            let mut vec = vec![];
            for i in proof.into_iter() {
                vec.push(format!("\"{}\"", i));
            }

            println!("[{}]", vec.join(","));
        }
    }

    #[test]
    #[ignore]
    fn test_fma_transpile_deposit_franklin_existing_account() {
        const NUM_DEPOSITS: usize = 1;
        println!(
            "Testing for {} deposits {} chunks",
            NUM_DEPOSITS,
            DepositOp::CHUNKS * NUM_DEPOSITS
        );
        let deposit_to_account_id = 1;
        let deposit_to_account_address =
            "1111111111111111111111111111111111111111".parse().unwrap();
        let (mut plasma_state, mut witness_accum) = test_genesis_plasma_state(vec![(
            deposit_to_account_id,
            Account::default_with_address(&deposit_to_account_address),
        )]);

        let deposit_op = DepositOp {
            priority_op: Deposit {
                from: deposit_to_account_address,
                token: 0,
                amount: BigDecimal::from(1),
                to: deposit_to_account_address,
            },
            account_id: deposit_to_account_id,
        };

        for _ in 0..NUM_DEPOSITS {
            plasma_state.apply_deposit_op(&deposit_op);
            let deposit_witness = apply_deposit_tx(&mut witness_accum.account_tree, &deposit_op);
            let deposit_operations = calculate_deposit_operations_from_witness(&deposit_witness);
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

        use crate::franklin_crypto::bellman::pairing::bn256::Bn256;
        // use crate::franklin_crypto::bellman::plonk::better_cs::adaptor::*;
        // use crate::franklin_crypto::bellman::plonk::better_cs::cs::Circuit as PlonkCircuit;
        use crate::franklin_crypto::bellman::kate_commitment::*;
        use crate::franklin_crypto::bellman::plonk::commitments::transcript::keccak_transcript::RollingKeccakTranscript;
        use crate::franklin_crypto::bellman::plonk::*;
        use crate::franklin_crypto::bellman::plonk::better_cs::fma_adaptor::Transpiler;
        use crate::franklin_crypto::bellman::plonk::better_cs::cs::PlonkCsWidth4WithNextStepParams;

        let mut transpiler = Transpiler::<Bn256, PlonkCsWidth4WithNextStepParams>::new();

        let c = witness_accum.into_circuit_instance();

        use crate::franklin_crypto::bellman::Circuit;
        c.clone().synthesize(&mut transpiler).unwrap();

        let hints = transpiler.into_hints();

        let mut hints_hist = std::collections::HashMap::new();
        hints_hist.insert("into addition gate".to_owned(), 0);
        hints_hist.insert("merge LC".to_owned(), 0);
        hints_hist.insert("into quadratic gate".to_owned(), 0);
        hints_hist.insert("into multiplication gate".to_owned(), 0);
        hints_hist.insert("into fma gate".to_owned(), 0);

        use crate::franklin_crypto::bellman::plonk::better_cs::fma_adaptor::TranspilationVariant;

        for (_, h) in hints.iter() {
            match h {
                TranspilationVariant::IntoQuadraticGate => {
                    *hints_hist.get_mut(&"into quadratic gate".to_owned()).unwrap() += 1;
                },
                TranspilationVariant::MergeLinearCombinations(..) => {
                    *hints_hist.get_mut(&"merge LC".to_owned()).unwrap() += 1;
                },
                TranspilationVariant::IntoAdditionGate(..) => {
                    *hints_hist.get_mut(&"into addition gate".to_owned()).unwrap() += 1;
                },
                TranspilationVariant::IntoMultiplicationGate(..) => {
                    *hints_hist.get_mut(&"into multiplication gate".to_owned()).unwrap() += 1;
                },
                TranspilationVariant::IntoFMAGate(..) => {
                    *hints_hist.get_mut(&"into fma gate".to_owned()).unwrap() += 1;
                }
            }
        }

        println!("Transpilation hist = {:?}", hints_hist);

        println!("Done transpiling");
    }
}
