use super::utils::*;

use crate::operation::SignatureData;
use crate::operation::*;
use ff::{Field, PrimeField};
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{append_be_fixed_width, le_bit_vector_into_field_element, eth_address_to_fr};
use models::node::DepositOp;
use models::params as franklin_constants;
use pairing::bn256::*;

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
            &self.args.ethereum_key.unwrap(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
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
    println!("deposit Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, deposit.account_address, deposit.token);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_address_fe = Fr::from_str(&deposit.account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&deposit.token.to_string()).unwrap();
    let amount_as_field_element = Fr::from_str(&deposit.amount.to_string()).unwrap();
    println!("amount_as_field_element is: {}", amount_as_field_element);
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
                assert!(
                    (acc.address == deposit.address)
                        || (acc.address == Fr::zero())
                );
                acc.address = deposit.address;
            },
            |bal| bal.value.add_assign(&amount_as_field_element),
        );

    let after_root = tree.root_hash();
    println!("deposit After root = {}", after_root);
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
            ethereum_key: Some(deposit.address),
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
    first_sig_msg: &Fr,
    second_sig_msg: &Fr,
    third_sig_msg: &Fr,
    signature_data: &SignatureData,
    signer_pub_key_packed: &[Option<bool>], // WHY? What signer?
) -> Vec<Operation<Bn256>> {
    
    let plasma_state = PlasmaState::new()

    let pubdata_chunks: Vec<_> = deposit_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();

    println!(
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
    use crate::witness::utils::public_data_commitment;
    use bellman::groth16::generate_random_parameters;
    use bellman::groth16::{create_random_proof, prepare_verifying_key, verify_proof};
    use models::merkle_tree::PedersenHasher;

    use crate::circuit::FranklinCircuit;
    use bellman::Circuit;
    use ff::{Field, PrimeField};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
    use models::primitives::GetBits;

    use franklin_crypto::circuit::test::*;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklin_crypto::jubjub::FixedGenerators;
    use models::circuit::account::{CircuitAccount, CircuitAccountTree, CircuitBalanceTree};
    use models::circuit::utils::*;
    use models::params as franklin_constants;

    use rand::{Rng, SeedableRng, XorShiftRng};

    #[test]
    #[ignore]
    fn test_deposit_franklin_in_empty_leaf() {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let validator_address_number = 7;
        let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
        let block_number = Fr::from_str("1").unwrap();
        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let phasher = PedersenHasher::<Bn256>::default();

        let mut tree: CircuitAccountTree =
            CircuitAccountTree::new(franklin_constants::account_tree_depth() as u32);
        println!("empty tree root_hash is: {}", tree.root_hash());
        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let sender_pub_key_hash = pub_key_hash_fe(&sender_pk, &phasher);
        let sender_leaf = CircuitAccount::<Bn256> {
            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
            nonce: Fr::zero(),
            pub_key_hash: sender_pub_key_hash,
            address: unimplemented!(),
        };
        println!("zero root_hash equals: {}", sender_leaf.subtree.root_hash());

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let validator_pub_key_hash = pub_key_hash_fe(&validator_pk, &phasher);

        let validator_leaf = CircuitAccount::<Bn256> {
            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
            nonce: Fr::zero(),
            pub_key_hash: validator_pub_key_hash,
            address: unimplemented!(),
        };
        println!(
            "validator_leaf_len {:?}",
            validator_leaf.get_bits_le().len()
        );
        println!(
            "validator_leaf_subree {:?}",
            validator_leaf.subtree.root_hash()
        );

        let mut validator_balances = vec![];
        for _ in 0..1 << franklin_constants::BALANCE_TREE_DEPTH {
            validator_balances.push(Some(Fr::zero()));
        }
        tree.insert(validator_address_number, validator_leaf);

        let mut account_address: u32 = rng.gen();
        account_address %= tree.capacity();
        let amount: u128 = 500;
        let token: u32 = 2;

        //-------------- Start applying changes to state
        let deposit_witness = apply_deposit(
            &mut tree,
            &DepositData {
                amount,
                token,
                account_address,
                new_pub_key_hash: sender_pub_key_hash,
            },
        );
        let (signature_data, first_sig_part, second_sig_part, third_sig_part) = generate_sig_data(
            &deposit_witness.get_sig_bits(),
            &phasher,
            &sender_sk,
            params,
        );

        let operations = calculate_deposit_operations_from_witness(
            &deposit_witness,
            &first_sig_part,
            &second_sig_part,
            &third_sig_part,
            &signature_data,
            &[Some(false); 256],
        );

        println!("tree before_applying fees: {}", tree.root_hash());

        let (root_after_fee, validator_account_witness) =
            apply_fee(&mut tree, validator_address_number, token, 0);
        println!("test root after fees {}", root_after_fee);
        let (validator_audit_path, _) = get_audits(&tree, validator_address_number, 0);

        let public_data_commitment = public_data_commitment::<Bn256>(
            &deposit_witness.get_pubdata(),
            deposit_witness.before_root,
            Some(root_after_fee),
            Some(validator_address),
            Some(block_number),
        );
        println!("validator balances: {}", validator_balances.len());

        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let instance = FranklinCircuit {
                operation_batch_size: 10,
                params,
                old_root: deposit_witness.before_root,
                new_root: Some(root_after_fee),
                operations: operations.clone(),
                pub_data_commitment: Some(public_data_commitment),
                block_number: Some(block_number),
                validator_account: validator_account_witness.clone(),
                validator_address: Some(validator_address),
                validator_balances: validator_balances.clone(),
                validator_audit_path: validator_audit_path.clone(),
            };
            instance.synthesize(&mut cs).unwrap();

            println!("unconstrained: {}", cs.find_unconstrained());
            println!("number of constraints {}", cs.num_constraints());
            if let Some(err) = cs.which_is_unsatisfied() {
                panic!("ERROR satisfying in {}", err);
            }
        }
    }

    #[test]
    #[ignore]
    fn test_deposit_franklin_proof() {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let validator_address_number = 7;
        let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
        let block_number = Fr::from_str("1").unwrap();
        let mut rng =
            &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let phasher = PedersenHasher::<Bn256>::default();

        let mut tree: CircuitAccountTree =
            CircuitAccountTree::new(franklin_constants::account_tree_depth() as u32);
        println!("empty tree root_hash is: {}", tree.root_hash());
        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let sender_pub_key_hash = pub_key_hash_fe(&sender_pk, &phasher);
        let sender_leaf = CircuitAccount::<Bn256> {
            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
            nonce: Fr::zero(),
            pub_key_hash: sender_pub_key_hash,
            address: unimplemented!(),
        };
        println!("zero root_hash equals: {}", sender_leaf.subtree.root_hash());

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let validator_pub_key_hash = pub_key_hash_fe(&validator_pk, &phasher);

        let validator_leaf = CircuitAccount::<Bn256> {
            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
            nonce: Fr::zero(),
            pub_key_hash: validator_pub_key_hash,
            address: unimplemented!(),
        };

        let mut validator_balances = vec![];
        for _ in 0..1 << franklin_constants::BALANCE_TREE_DEPTH {
            validator_balances.push(Some(Fr::zero()));
        }
        tree.insert(validator_address_number, validator_leaf);

        let mut account_address: u32 = rng.gen();
        account_address %= tree.capacity();
        let amount: u128 = 500;
        let token: u32 = 2;

        //-------------- Start applying changes to state
        let deposit_witness = apply_deposit(
            &mut tree,
            &DepositData {
                amount,
                token,
                account_address,
                new_pub_key_hash: sender_pub_key_hash,
            },
        );

        let (signature, first_sig_part, second_sig_part, third_sig_part) = generate_sig_data(
            &deposit_witness.get_sig_bits(),
            &phasher,
            &sender_sk,
            params,
        );

        let operations = calculate_deposit_operations_from_witness(
            &deposit_witness,
            &first_sig_part,
            &second_sig_part,
            &third_sig_part,
            &signature,
            &[Some(false); 256],
        );

        println!("tree before_applying fees: {}", tree.root_hash());

        let (root_after_fee, validator_account_witness) =
            apply_fee(&mut tree, validator_address_number, token, 0);
        println!("test root after fees {}", root_after_fee);
        let (validator_audit_path, _) = get_audits(&tree, validator_address_number, 0);

        let public_data_commitment = public_data_commitment::<Bn256>(
            &deposit_witness.get_pubdata(),
            deposit_witness.before_root,
            Some(root_after_fee),
            Some(validator_address),
            Some(block_number),
        );
        println!("validator balances: {}", validator_balances.len());

        {
            let instance = FranklinCircuit {
                operation_batch_size: 10,
                params,
                old_root: deposit_witness.before_root,
                new_root: Some(root_after_fee),
                operations: operations.clone(),
                pub_data_commitment: Some(public_data_commitment),
                block_number: Some(block_number),
                validator_account: validator_account_witness.clone(),
                validator_address: Some(validator_address),
                validator_balances: validator_balances.clone(),
                validator_audit_path: validator_audit_path.clone(),
            };

            let tmp_cirtuit_params = generate_random_parameters(instance, &mut rng).unwrap();
            println!("len a is {}", tmp_cirtuit_params.a.len());
            let instance = FranklinCircuit {
                operation_batch_size: 10,
                params,
                old_root: deposit_witness.before_root,
                new_root: Some(root_after_fee),
                operations,
                pub_data_commitment: Some(public_data_commitment),
                block_number: Some(block_number),
                validator_account: validator_account_witness,
                validator_address: Some(validator_address),
                validator_balances,
                validator_audit_path,
            };

            let proof = create_random_proof(instance, &tmp_cirtuit_params, &mut rng);
            if proof.is_err() {
                panic!("proof can not be created: {}", proof.err().unwrap());
                //             return Err(BabyProverErr::Other("proof.is_err()".to_owned()));
            }

            let p = proof.unwrap();

            let pvk = prepare_verifying_key(&tmp_cirtuit_params.vk);

            let success = verify_proof(&pvk, &p.clone(), &[public_data_commitment]);
            if success.is_err() {
                panic!(
                    "Proof is verification failed with error {}",
                    success.err().unwrap()
                );
            }
            if !success.unwrap() {
                panic!("Proof is invalid");
            }
        }
    }
}
