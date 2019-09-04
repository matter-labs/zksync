use super::utils::*;
use crate::utils::*;

use crate::operation::*;
use ff::{Field, PrimeField};
use num_traits::cast::ToPrimitive;

use franklin_crypto::circuit::float_point::{convert_to_float, parse_float_to_u128};
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::CircuitAccountTree;

use models::node::DepositOp;
use models::params as franklin_constants;
use pairing::bn256::*;

pub struct DepositData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub account_address: u32,
    pub new_pub_key_hash: Fr,
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
            &self.args.amount.unwrap(),
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.new_pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        //        assert_eq!(pubdata_bits.len(), 37 * 8);
        pubdata_bits.resize(32 * 8, false);
        pubdata_bits
    }
}

pub fn apply_deposit_tx(
    tree: &mut CircuitAccountTree,
    deposit: &DepositOp,
) -> DepositWitness<Bn256> {
    let alt_new_pubkey_hash = Fr::from_hex(&deposit.tx.to.to_hex()).unwrap();
    let deposit_data = DepositData {
        amount: deposit.tx.amount.to_u128().unwrap(),
        fee: deposit.tx.fee.to_u128().unwrap(),
        token: u32::from(deposit.tx.token),
        account_address: deposit.account_id,
        new_pub_key_hash: alt_new_pubkey_hash,
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
    assert_eq!(capacity, 1 << franklin_constants::ACCOUNT_TREE_DEPTH);
    let account_address_fe = Fr::from_str(&deposit.account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&deposit.token.to_string()).unwrap();
    let amount_as_field_element = Fr::from_str(&deposit.amount.to_string()).unwrap();

    let amount_bits = convert_to_float(
        deposit.amount,
        franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();
    let reparsed_amount = parse_float_to_u128(
        amount_bits.clone(),
        franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();
    assert_eq!(reparsed_amount, deposit.amount);

    let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

    let fee_as_field_element = Fr::from_str(&deposit.fee.to_string()).unwrap();

    let fee_bits = convert_to_float(
        deposit.fee,
        franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();
    let reparsed_fee = parse_float_to_u128(
        fee_bits.clone(),
        franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();
    assert_eq!(reparsed_fee, deposit.fee);
    let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

    //calculate a and b
    let a = amount_as_field_element;
    let b = fee_as_field_element;

    //applying deposit
    let (account_witness_before, account_witness_after, balance_before, balance_after) =
        apply_leaf_operation(
            tree,
            deposit.account_address,
            deposit.token,
            |acc| {
                assert!(
                    (acc.pub_key_hash == deposit.new_pub_key_hash)
                        || (acc.pub_key_hash == Fr::zero())
                );
                acc.pub_key_hash = deposit.new_pub_key_hash;
                acc.nonce.add_assign(&Fr::from_str("1").unwrap());
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
            ethereum_key: Some(Fr::zero()),
            amount: Some(amount_encoded),
            fee: Some(fee_encoded),
            a: Some(a),
            b: Some(b),
            new_pub_key_hash: Some(deposit.new_pub_key_hash),
        },
        before_root: Some(before_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("1").unwrap()),
    }
}

pub fn calculate_deposit_operations_from_witness(
    deposit_witness: &DepositWitness<Bn256>,
    sig_msg: &Fr,
    signature: Option<TransactionSignature<Bn256>>,
    signer_pub_key_x: &Fr,
    signer_pub_key_y: &Fr,
) -> Vec<Operation<Bn256>> {
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
        sig_msg: Some(*sig_msg),
        signature: signature.clone(),
        signer_pub_key_x: Some(*signer_pub_key_x),
        signer_pub_key_y: Some(*signer_pub_key_y),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.before.clone(),
        rhs: deposit_witness.before.clone(),
    };

    let operation_one = Operation {
        new_root: deposit_witness.after_root,
        tx_type: deposit_witness.tx_type,
        chunk: Some(Fr::from_str("1").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[1]),
        sig_msg: Some(*sig_msg),
        signature: signature.clone(),
        signer_pub_key_x: Some(*signer_pub_key_x),
        signer_pub_key_y: Some(*signer_pub_key_y),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.after.clone(),
        rhs: deposit_witness.after.clone(),
    };

    let operation_two = Operation {
        new_root: deposit_witness.after_root,
        tx_type: deposit_witness.tx_type,
        chunk: Some(Fr::from_str("2").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[2]),
        sig_msg: Some(*sig_msg),
        signature: signature.clone(),
        signer_pub_key_x: Some(*signer_pub_key_x),
        signer_pub_key_y: Some(*signer_pub_key_y),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.after.clone(),
        rhs: deposit_witness.after.clone(),
    };

    let operation_three = Operation {
        new_root: deposit_witness.after_root,
        tx_type: deposit_witness.tx_type,
        chunk: Some(Fr::from_str("3").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[3]),
        sig_msg: Some(*sig_msg),
        signature: signature.clone(),
        signer_pub_key_x: Some(*signer_pub_key_x),
        signer_pub_key_y: Some(*signer_pub_key_y),
        args: deposit_witness.args.clone(),
        lhs: deposit_witness.after.clone(),
        rhs: deposit_witness.after.clone(),
    };

    let operations: Vec<Operation<_>> = vec![
        operation_zero,
        operation_one,
        operation_two,
        operation_three,
    ];
    operations
}
#[cfg(test)]
mod test {
    use super::*;
    use models::merkle_tree::PedersenHasher;

    use crate::witness::utils::public_data_commitment;
    use bellman::groth16::generate_random_parameters;
    use bellman::groth16::{create_random_proof, prepare_verifying_key, verify_proof};

    use crate::circuit::FranklinCircuit;
    use bellman::Circuit;
    use ff::{BitIterator, Field, PrimeField};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
    use models::primitives::GetBits;

    use franklin_crypto::circuit::test::*;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklin_crypto::jubjub::FixedGenerators;
    use models::circuit::account::{CircuitAccount, CircuitAccountTree, CircuitBalanceTree};
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
            CircuitAccountTree::new(franklin_constants::ACCOUNT_TREE_DEPTH as u32);
        println!("empty tree root_hash is: {}", tree.root_hash());
        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let sender_pub_key_hash = pub_key_hash(&sender_pk, &phasher);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
        let sender_leaf = CircuitAccount::<Bn256> {
            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
            nonce: Fr::zero(),
            pub_key_hash: sender_pub_key_hash, // pub_x: validator_x.clone(),
                                               // pub_y: validator_y.clone(),
        };
        println!("zero root_hash equals: {}", sender_leaf.subtree.root_hash());

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let validator_pub_key_hash = pub_key_hash(&validator_pk, &phasher);

        let validator_leaf = CircuitAccount::<Bn256> {
            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
            nonce: Fr::zero(),
            pub_key_hash: validator_pub_key_hash,
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
        let fee: u128 = 80;
        let token: u32 = 2;

        //-------------- Start applying changes to state
        let deposit_witness = apply_deposit(
            &mut tree,
            &DepositData {
                amount,
                fee,
                token,
                account_address,
                new_pub_key_hash: sender_pub_key_hash,
            },
        );

        let sig_msg = Fr::from_str("2").unwrap(); //dummy sig msg cause skipped on deposit proof
        let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
        sig_bits.reverse();
        sig_bits.truncate(80);

        // println!(" capacity {}",<Bn256 as JubjubEngine>::Fs::Capacity);
        let signature = sign(&sig_bits, &sender_sk, p_g, params, rng);
        //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));

        let operations = calculate_deposit_operations_from_witness(
            &deposit_witness,
            &sig_msg,
            signature,
            &sender_x,
            &sender_y,
        );

        println!("tree before_applying fees: {}", tree.root_hash());

        let (root_after_fee, validator_account_witness) =
            apply_fee(&mut tree, validator_address_number, token, fee);
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
            let err = cs.which_is_unsatisfied();
            if err.is_some() {
                panic!("ERROR satisfying in {}", err.unwrap());
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
            CircuitAccountTree::new(franklin_constants::ACCOUNT_TREE_DEPTH as u32);
        println!("empty tree root_hash is: {}", tree.root_hash());
        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let sender_pub_key_hash = pub_key_hash(&sender_pk, &phasher);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
        let sender_leaf = CircuitAccount::<Bn256> {
            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
            nonce: Fr::zero(),
            pub_key_hash: sender_pub_key_hash,
        };
        println!("zero root_hash equals: {}", sender_leaf.subtree.root_hash());

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let validator_pub_key_hash = pub_key_hash(&validator_pk, &phasher);

        let validator_leaf = CircuitAccount::<Bn256> {
            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
            nonce: Fr::zero(),
            pub_key_hash: validator_pub_key_hash,
        };

        let mut validator_balances = vec![];
        for _ in 0..1 << franklin_constants::BALANCE_TREE_DEPTH {
            validator_balances.push(Some(Fr::zero()));
        }
        tree.insert(validator_address_number, validator_leaf);

        let mut account_address: u32 = rng.gen();
        account_address %= tree.capacity();
        let amount: u128 = 500;
        let fee: u128 = 80;
        let token: u32 = 2;

        //-------------- Start applying changes to state
        let deposit_witness = apply_deposit(
            &mut tree,
            &DepositData {
                amount,
                fee,
                token,
                account_address,
                new_pub_key_hash: sender_pub_key_hash,
            },
        );

        let sig_msg = Fr::from_str("2").unwrap(); //dummy sig msg cause skipped on deposit proof
        let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
        sig_bits.reverse();
        sig_bits.truncate(80);

        // println!(" capacity {}",<Bn256 as JubjubEngine>::Fs::Capacity);
        let signature = sign(&sig_bits, &sender_sk, p_g, params, rng);
        //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));

        let operations = calculate_deposit_operations_from_witness(
            &deposit_witness,
            &sig_msg,
            signature,
            &sender_x,
            &sender_y,
        );

        println!("tree before_applying fees: {}", tree.root_hash());

        let (root_after_fee, validator_account_witness) =
            apply_fee(&mut tree, validator_address_number, token, fee);
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
