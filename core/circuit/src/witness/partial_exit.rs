use super::utils::*;

use crate::operation::*;
use crate::utils::*;

use ff::{Field, PrimeField};

use franklin_crypto::circuit::float_point::convert_to_float;
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::CircuitAccountTree;
use num_traits::cast::ToPrimitive;

use models::node::PartialExitOp;
use models::params as franklin_constants;
use pairing::bn256::*;

pub struct PartialExitData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub account_address: u32,
    pub ethereum_key: Fr,
}
pub struct PartialExitWitness<E: JubjubEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}
impl<E: JubjubEngine> PartialExitWitness<E> {
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
            franklin_constants::ACCOUNT_TREE_DEPTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
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
            &self.args.ethereum_key.unwrap(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        );
        pubdata_bits.resize(32 * 8, false);
        pubdata_bits
    }
    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("3").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.ethereum_key.unwrap(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_EXT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.amount.unwrap(),
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.fee.unwrap(),
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}
pub fn apply_partial_exit_tx(
    tree: &mut CircuitAccountTree,
    partial_exit: &PartialExitOp,
) -> PartialExitWitness<Bn256> {
    let transfer_data = PartialExitData {
        amount: partial_exit.tx.amount.to_u128().unwrap(),
        fee: partial_exit.tx.fee.to_u128().unwrap(),
        token: u32::from(partial_exit.tx.token),
        account_address: partial_exit.account_id,
        ethereum_key: Fr::from_hex(&format!("{:x}", &partial_exit.tx.eth_address)).unwrap(),
    };
    // le_bit_vector_into_field_element()
    apply_partial_exit(tree, &transfer_data)
}
pub fn apply_partial_exit(
    tree: &mut CircuitAccountTree,
    partial_exit: &PartialExitData,
) -> PartialExitWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    println!("Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, partial_exit.account_address, partial_exit.token);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::ACCOUNT_TREE_DEPTH);
    let account_address_fe = Fr::from_str(&partial_exit.account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&partial_exit.token.to_string()).unwrap();
    let amount_as_field_element = Fr::from_str(&partial_exit.amount.to_string()).unwrap();

    let amount_bits = convert_to_float(
        partial_exit.amount,
        franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

    let fee_as_field_element = Fr::from_str(&partial_exit.fee.to_string()).unwrap();

    let fee_bits = convert_to_float(
        partial_exit.fee,
        franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

    //calculate a and b

    //applying partial_exit

    let (account_witness_before, account_witness_after, balance_before, balance_after) =
        apply_leaf_operation(
            tree,
            partial_exit.account_address,
            partial_exit.token,
            |acc| {
                acc.nonce.add_assign(&Fr::from_str("1").unwrap());
            },
            |bal| {
                if partial_exit.amount == 0 {
                    bal.value = Fr::zero();
                } else {
                    bal.value.sub_assign(&amount_as_field_element);
                    bal.value.sub_assign(&fee_as_field_element);
                }
            },
        );

    let after_root = tree.root_hash();
    println!("After root = {}", after_root);
    let (audit_path_after, audit_balance_path_after) =
        get_audits(tree, partial_exit.account_address, partial_exit.token);

    let a = balance_before;
    let mut b = amount_as_field_element;
    b.add_assign(&fee_as_field_element);

    PartialExitWitness {
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
            ethereum_key: Some(partial_exit.ethereum_key),
            amount: Some(amount_encoded),
            fee: Some(fee_encoded),
            a: Some(a),
            b: Some(b),
            new_pub_key_hash: Some(Fr::zero()),
        },
        before_root: Some(before_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("3").unwrap()),
    }
}
pub fn calculate_partial_exit_operations_from_witness(
    partial_exit_witness: &PartialExitWitness<Bn256>,
    first_sig_msg: &Fr,
    second_sig_msg: &Fr,
    signature: Option<TransactionSignature<Bn256>>,
    signer_pub_key_x: &Fr,
    signer_pub_key_y: &Fr,
) -> Vec<Operation<Bn256>> {
    let pubdata_chunks: Vec<_> = partial_exit_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();

    let operation_zero = Operation {
        new_root: partial_exit_witness.after_root,
        tx_type: partial_exit_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        signature: signature.clone(),
        signer_pub_key_x: Some(*signer_pub_key_x),
        signer_pub_key_y: Some(*signer_pub_key_y),
        args: partial_exit_witness.args.clone(),
        lhs: partial_exit_witness.before.clone(),
        rhs: partial_exit_witness.before.clone(),
    };

    let operation_one = Operation {
        new_root: partial_exit_witness.after_root,
        tx_type: partial_exit_witness.tx_type,
        chunk: Some(Fr::from_str("1").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[1]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        signature: signature.clone(),
        signer_pub_key_x: Some(*signer_pub_key_x),
        signer_pub_key_y: Some(*signer_pub_key_y),
        args: partial_exit_witness.args.clone(),
        lhs: partial_exit_witness.after.clone(),
        rhs: partial_exit_witness.after.clone(),
    };

    let operation_two = Operation {
        new_root: partial_exit_witness.after_root,
        tx_type: partial_exit_witness.tx_type,
        chunk: Some(Fr::from_str("2").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[2]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        signature: signature.clone(),
        signer_pub_key_x: Some(*signer_pub_key_x),
        signer_pub_key_y: Some(*signer_pub_key_y),
        args: partial_exit_witness.args.clone(),
        lhs: partial_exit_witness.after.clone(),
        rhs: partial_exit_witness.after.clone(),
    };

    let operation_three = Operation {
        new_root: partial_exit_witness.after_root,
        tx_type: partial_exit_witness.tx_type,
        chunk: Some(Fr::from_str("3").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[3]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        signature: signature.clone(),
        signer_pub_key_x: Some(*signer_pub_key_x),
        signer_pub_key_y: Some(*signer_pub_key_y),
        args: partial_exit_witness.args.clone(),
        lhs: partial_exit_witness.after.clone(),
        rhs: partial_exit_witness.after.clone(),
    };
    vec![
        operation_zero,
        operation_one,
        operation_two,
        operation_three,
    ]
}
#[cfg(test)]
mod test {
    use super::*;

    use crate::witness::utils::public_data_commitment;

    use crate::circuit::FranklinCircuit;
    use bellman::Circuit;

    use ff::{Field, PrimeField};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
    use franklin_crypto::circuit::test::*;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklin_crypto::jubjub::FixedGenerators;
    use models::circuit::account::{
        Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree,
    };
    use models::merkle_tree::PedersenHasher;
    use models::params as franklin_constants;
    use rand::{Rng, SeedableRng, XorShiftRng};
    #[test]
    #[ignore]
    fn test_partial_exit_franklin() {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let validator_address_number = 7;
        let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
        let block_number = Fr::from_str("1").unwrap();
        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let phasher = PedersenHasher::<Bn256>::default();

        let mut tree: CircuitAccountTree =
            CircuitAccountTree::new(franklin_constants::ACCOUNT_TREE_DEPTH as u32);

        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let sender_pub_key_hash = pub_key_hash(&sender_pk, &phasher);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
        println!("x = {}, y = {}", sender_x, sender_y);

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let validator_pub_key_hash = pub_key_hash(&validator_pk, &phasher);
        let (validator_x, validator_y) = validator_pk.0.into_xy();
        println!("x = {}, y = {}", validator_x, validator_y);
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
        let fee: u128 = 100;
        let token: u32 = 2;
        let ethereum_key = Fr::from_str("124").unwrap();

        let sender_balance_before: u128 = 2000;

        let sender_balance_before_as_field_element =
            Fr::from_str(&sender_balance_before.to_string()).unwrap();

        let mut sender_balance_tree =
            CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32);
        sender_balance_tree.insert(
            token,
            Balance {
                value: sender_balance_before_as_field_element,
            },
        );

        let sender_leaf_initial = CircuitAccount::<Bn256> {
            subtree: sender_balance_tree,
            nonce: Fr::zero(),
            pub_key_hash: sender_pub_key_hash,
        };

        tree.insert(account_address, sender_leaf_initial);

        let partial_exit_witness = apply_partial_exit(
            &mut tree,
            &PartialExitData {
                amount,
                fee,
                token,
                account_address,
                ethereum_key,
            },
        );

        let (signature, first_sig_part, second_sig_part) = generate_sig_data(
            &partial_exit_witness.get_sig_bits(),
            &phasher,
            &sender_sk,
            params,
        );

        let operations = calculate_partial_exit_operations_from_witness(
            &partial_exit_witness,
            &first_sig_part,
            &second_sig_part,
            signature,
            &sender_x,
            &sender_y,
        );

        let (root_after_fee, validator_account_witness) =
            apply_fee(&mut tree, validator_address_number, token, fee);

        let (validator_audit_path, _) = get_audits(&tree, validator_address_number, 0);

        let public_data_commitment = public_data_commitment::<Bn256>(
            &partial_exit_witness.get_pubdata(),
            partial_exit_witness.before_root,
            Some(root_after_fee),
            Some(validator_address),
            Some(block_number),
        );
        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let instance = FranklinCircuit {
                operation_batch_size: 10,
                params,
                old_root: partial_exit_witness.before_root,
                new_root: Some(root_after_fee),
                operations,
                pub_data_commitment: Some(public_data_commitment),
                block_number: Some(block_number),
                validator_account: validator_account_witness,
                validator_address: Some(validator_address),
                validator_balances,
                validator_audit_path,
            };

            instance.synthesize(&mut cs).unwrap();

            println!("{}", cs.find_unconstrained());

            println!("{}", cs.num_constraints());

            let err = cs.which_is_unsatisfied();
            if err.is_some() {
                panic!("ERROR satisfying in {}", err.unwrap());
            }
        }
    }

    #[test]
    #[ignore]
    fn test_full_exit_franklin() {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let validator_address_number = 7;
        let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
        let block_number = Fr::from_str("1").unwrap();
        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let phasher = PedersenHasher::<Bn256>::default();

        let mut tree: CircuitAccountTree =
            CircuitAccountTree::new(franklin_constants::ACCOUNT_TREE_DEPTH as u32);

        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let sender_pub_key_hash = pub_key_hash(&sender_pk, &phasher);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
        println!("x = {}, y = {}", sender_x, sender_y);

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let validator_pub_key_hash = pub_key_hash(&validator_pk, &phasher);
        let (validator_x, validator_y) = validator_pk.0.into_xy();
        println!("x = {}, y = {}", validator_x, validator_y);
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
        let amount: u128 = 0;
        let fee: u128 = 100;
        let token: u32 = 2;
        let ethereum_key = Fr::from_str("124").unwrap();

        let sender_balance_before: u128 = 2000;

        let sender_balance_before_as_field_element =
            Fr::from_str(&sender_balance_before.to_string()).unwrap();

        let mut sender_balance_tree =
            CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32);
        sender_balance_tree.insert(
            token,
            Balance {
                value: sender_balance_before_as_field_element,
            },
        );

        let sender_leaf_initial = CircuitAccount::<Bn256> {
            subtree: sender_balance_tree,
            nonce: Fr::zero(),
            pub_key_hash: sender_pub_key_hash,
        };

        tree.insert(account_address, sender_leaf_initial);

        let partial_exit_witness = apply_partial_exit(
            &mut tree,
            &PartialExitData {
                amount,
                fee,
                token,
                account_address,
                ethereum_key,
            },
        );

        let (signature, first_sig_part, second_sig_part) = generate_sig_data(
            &partial_exit_witness.get_sig_bits(),
            &phasher,
            &sender_sk,
            params,
        );

        let operations = calculate_partial_exit_operations_from_witness(
            &partial_exit_witness,
            &first_sig_part,
            &second_sig_part,
            signature,
            &sender_x,
            &sender_y,
        );

        let (root_after_fee, validator_account_witness) =
            apply_fee(&mut tree, validator_address_number, token, fee);

        let (validator_audit_path, _) = get_audits(&tree, validator_address_number, 0);

        let public_data_commitment = public_data_commitment::<Bn256>(
            &partial_exit_witness.get_pubdata(),
            partial_exit_witness.before_root,
            Some(root_after_fee),
            Some(validator_address),
            Some(block_number),
        );
        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let instance = FranklinCircuit {
                operation_batch_size: 10,
                params,
                old_root: partial_exit_witness.before_root,
                new_root: Some(root_after_fee),
                operations,
                pub_data_commitment: Some(public_data_commitment),
                block_number: Some(block_number),
                validator_account: validator_account_witness,
                validator_address: Some(validator_address),
                validator_balances,
                validator_audit_path,
            };

            instance.synthesize(&mut cs).unwrap();

            println!("{}", cs.find_unconstrained());

            println!("{}", cs.num_constraints());

            let err = cs.which_is_unsatisfied();
            if err.is_some() {
                panic!("ERROR satisfying in {}", err.unwrap());
            }
        }
    }

}
