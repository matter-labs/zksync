use super::utils::*;
use crate::operation::SignatureData;
use crate::operation::*;
use ff::{Field, PrimeField};
use franklin_crypto::circuit::float_point::convert_to_float;
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{append_be_fixed_width, le_bit_vector_into_field_element};
use models::params as franklin_constants;
use pairing::bn256::*;

use models::node::TransferOp;
use models::primitives::big_decimal_to_u128;

pub struct TransferData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub from_account_address: u32,
    pub to_account_address: u32,
}
pub struct TransferWitness<E: JubjubEngine> {
    pub from_before: OperationBranch<E>,
    pub from_intermediate: OperationBranch<E>,
    pub from_after: OperationBranch<E>,
    pub to_before: OperationBranch<E>,
    pub to_intermediate: OperationBranch<E>,
    pub to_after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub intermediate_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}
impl<E: JubjubEngine> TransferWitness<E> {
    pub fn get_pubdata(&self) -> Vec<bool> {
        // construct pubdata
        let mut pubdata_bits = vec![];
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.tx_type.unwrap(),
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.from_before.address.unwrap(),
            franklin_constants::ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.from_before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.to_before.address.unwrap(),
            franklin_constants::ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.amount_packed.unwrap(),
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH
                + franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            franklin_constants::FEE_MANTISSA_BIT_WIDTH + franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        );
        pubdata_bits.resize(2 * franklin_constants::CHUNK_BIT_WIDTH, false); //TODO verify if right padding is okay
        pubdata_bits
    }
    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("5").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self
                .from_before
                .witness
                .account_witness
                .pub_key_hash
                .unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.to_before.witness.account_witness.pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );

        append_be_fixed_width(
            &mut sig_bits,
            &self.from_before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.amount_packed.unwrap(),
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
            &self.from_before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}
pub fn apply_transfer_tx(
    tree: &mut CircuitAccountTree,
    transfer: &TransferOp,
) -> TransferWitness<Bn256> {
    let transfer_data = TransferData {
        amount: big_decimal_to_u128(&transfer.tx.amount),
        fee: big_decimal_to_u128(&transfer.tx.fee),
        token: u32::from(transfer.tx.token),
        from_account_address: transfer.from,
        to_account_address: transfer.to,
    };
    // le_bit_vector_into_field_element()
    apply_transfer(tree, &transfer_data)
}
pub fn apply_transfer(
    tree: &mut CircuitAccountTree,
    transfer: &TransferData,
) -> TransferWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    println!("Initial root = {}", before_root);
    let (audit_path_from_before, audit_balance_path_from_before) =
        get_audits(tree, transfer.from_account_address, transfer.token);

    let (audit_path_to_before, audit_balance_path_to_before) =
        get_audits(tree, transfer.to_account_address, transfer.token);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_address_from_fe = Fr::from_str(&transfer.from_account_address.to_string()).unwrap();
    let account_address_to_fe = Fr::from_str(&transfer.to_account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&transfer.token.to_string()).unwrap();
    let amount_as_field_element = Fr::from_str(&transfer.amount.to_string()).unwrap();

    let amount_bits = convert_to_float(
        transfer.amount,
        franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

    let fee_as_field_element = Fr::from_str(&transfer.fee.to_string()).unwrap();

    let fee_bits = convert_to_float(
        transfer.fee,
        franklin_constants::FEE_EXPONENT_BIT_WIDTH,
        franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        10,
    )
    .unwrap();

    let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

    //applying first transfer part
    let (
        account_witness_from_before,
        account_witness_from_intermediate,
        balance_from_before,
        balance_from_intermediate,
    ) = apply_leaf_operation(
        tree,
        transfer.from_account_address,
        transfer.token,
        |acc| {
            acc.nonce.add_assign(&Fr::from_str("1").unwrap());
        },
        |bal| {
            bal.value.sub_assign(&amount_as_field_element);
            bal.value.sub_assign(&fee_as_field_element)
        },
    );

    let intermediate_root = tree.root_hash();
    println!("Intermediate root = {}", intermediate_root);

    let (audit_path_from_intermediate, audit_balance_path_from_intermediate) =
        get_audits(tree, transfer.from_account_address, transfer.token);

    let (audit_path_to_intermediate, audit_balance_path_to_intermediate) =
        get_audits(tree, transfer.to_account_address, transfer.token);

    let (
        account_witness_to_intermediate,
        account_witness_to_after,
        balance_to_intermediate,
        balance_to_after,
    ) = apply_leaf_operation(
        tree,
        transfer.to_account_address,
        transfer.token,
        |_| {},
        |bal| bal.value.add_assign(&amount_as_field_element),
    );
    let after_root = tree.root_hash();
    let (audit_path_from_after, audit_balance_path_from_after) =
        get_audits(tree, transfer.from_account_address, transfer.token);

    let (audit_path_to_after, audit_balance_path_to_after) =
        get_audits(tree, transfer.to_account_address, transfer.token);

    //calculate a and b
    let a = balance_from_before;
    let mut b = amount_as_field_element;
    b.add_assign(&fee_as_field_element);

    TransferWitness {
        from_before: OperationBranch {
            address: Some(account_address_from_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_from_before,
                account_path: audit_path_from_before,
                balance_value: Some(balance_from_before),
                balance_subtree_path: audit_balance_path_from_before,
            },
        },
        from_intermediate: OperationBranch {
            address: Some(account_address_from_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_from_intermediate.clone(),
                account_path: audit_path_from_intermediate,
                balance_value: Some(balance_from_intermediate),
                balance_subtree_path: audit_balance_path_from_intermediate,
            },
        },
        from_after: OperationBranch {
            address: Some(account_address_from_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_from_intermediate,
                account_path: audit_path_from_after,
                balance_value: Some(balance_from_intermediate),
                balance_subtree_path: audit_balance_path_from_after,
            },
        },
        to_before: OperationBranch {
            address: Some(account_address_to_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_to_intermediate.clone(),
                account_path: audit_path_to_before,
                balance_value: Some(balance_to_intermediate),
                balance_subtree_path: audit_balance_path_to_before,
            },
        },
        to_intermediate: OperationBranch {
            address: Some(account_address_to_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_to_intermediate,
                account_path: audit_path_to_intermediate.clone(),
                balance_value: Some(balance_to_intermediate),
                balance_subtree_path: audit_balance_path_to_intermediate.clone(),
            },
        },
        to_after: OperationBranch {
            address: Some(account_address_to_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: account_witness_to_after,
                account_path: audit_path_to_after,
                balance_value: Some(balance_to_after),
                balance_subtree_path: audit_balance_path_to_after,
            },
        },
        args: OperationArguments {
            ethereum_key: Some(Fr::zero()),
            amount_packed: Some(amount_encoded),
            full_amount: Some(amount_as_field_element),
            fee: Some(fee_encoded),
            pub_nonce: Some(Fr::zero()),
            a: Some(a),
            b: Some(b),
            new_pub_key_hash: Some(Fr::zero()),
        },
        before_root: Some(before_root),
        intermediate_root: Some(intermediate_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("5").unwrap()),
    }
}

pub fn calculate_transfer_operations_from_witness(
    transfer_witness: &TransferWitness<Bn256>,
    first_sig_msg: &Fr,
    second_sig_msg: &Fr,
    third_sig_msg: &Fr,
    signature_data: &SignatureData,
    signer_pub_key_packed: &[Option<bool>],
) -> Vec<Operation<Bn256>> {
    let pubdata_chunks: Vec<_> = transfer_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();

    let operation_zero = Operation {
        new_root: transfer_witness.intermediate_root,
        tx_type: transfer_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: transfer_witness.args.clone(),
        lhs: transfer_witness.from_before.clone(),
        rhs: transfer_witness.to_before.clone(),
    };

    let operation_one = Operation {
        new_root: transfer_witness.after_root,
        tx_type: transfer_witness.tx_type,
        chunk: Some(Fr::from_str("1").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[1]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: transfer_witness.args.clone(),
        lhs: transfer_witness.from_intermediate.clone(),
        rhs: transfer_witness.to_intermediate.clone(),
    };
    vec![operation_zero, operation_one]
}
#[cfg(test)]
mod test {
    use super::*;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use models::params as franklin_constants;
    use models::primitives::bytes_into_be_bits;

    use crate::circuit::FranklinCircuit;
    use bellman::Circuit;

    use ff::{Field, PrimeField};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
    use franklin_crypto::circuit::float_point::convert_to_float;
    use franklin_crypto::circuit::test::*;
    use franklin_crypto::jubjub::FixedGenerators;
    use models::circuit::account::{
        Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree,
    };
    use models::circuit::utils::*;
    use models::merkle_tree::PedersenHasher;
    use models::node::tx::PackedPublicKey;
    use rand::{Rng, SeedableRng, XorShiftRng};
    #[test]
    #[ignore]
    fn test_transfer() {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;

        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);

        let validator_address_number = 7;
        let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();

        let phasher = PedersenHasher::<Bn256>::default();

        let mut tree = CircuitAccountTree::new(franklin_constants::account_tree_depth() as u32);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());

        let from_sk = PrivateKey::<Bn256>(rng.gen());
        let from_pk = PublicKey::from_private(&from_sk, p_g, params);
        let (from_x, from_y) = from_pk.0.into_xy();
        println!("x = {}, y = {}", from_x, from_y);
        let from_pub_key_hash = pub_key_hash_fe(&from_pk, &phasher);

        let to_sk = PrivateKey::<Bn256>(rng.gen());
        let to_pk = PublicKey::from_private(&to_sk, p_g, params);
        let (to_x, to_y) = to_pk.0.into_xy();
        println!("x = {}, y = {}", to_x, to_y);
        let to_pub_key_hash = pub_key_hash_fe(&to_pk, &phasher);

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let (validator_x, validator_y) = validator_pk.0.into_xy();
        println!("x = {}, y = {}", validator_x, validator_y);
        let validator_pub_key_hash = pub_key_hash_fe(&validator_pk, &phasher);

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

        let mut from_leaf_number: u32 = rng.gen();
        from_leaf_number %= capacity;
        let _from_leaf_number_fe = Fr::from_str(&from_leaf_number.to_string()).unwrap();

        let mut to_leaf_number: u32 = rng.gen();
        to_leaf_number %= capacity;
        let _to_leaf_number_fe = Fr::from_str(&to_leaf_number.to_string()).unwrap();

        let from_balance_before: u128 = 2000;

        let from_balance_before_as_field_element =
            Fr::from_str(&from_balance_before.to_string()).unwrap();

        let to_balance_before: u128 = 2100;

        let to_balance_before_as_field_element =
            Fr::from_str(&to_balance_before.to_string()).unwrap();

        let transfer_amount: u128 = 500;

        let transfer_amount_bits = convert_to_float(
            transfer_amount,
            franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let _transfer_amount_encoded: Fr = le_bit_vector_into_field_element(&transfer_amount_bits);

        let fee: u128 = 7;

        let fee_bits = convert_to_float(
            fee,
            franklin_constants::FEE_EXPONENT_BIT_WIDTH,
            franklin_constants::FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let _fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

        let token: u32 = 2;
        let _token_fe = Fr::from_str(&token.to_string()).unwrap();
        let block_number = Fr::from_str("1").unwrap();
        // prepare state, so that we could make transfer
        let mut from_balance_tree =
            CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32);
        let mut to_balance_tree =
            CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32);
        from_balance_tree.insert(
            token,
            Balance {
                value: from_balance_before_as_field_element,
            },
        );

        let from_leaf_initial = CircuitAccount::<Bn256> {
            subtree: from_balance_tree,
            nonce: Fr::zero(),
            pub_key_hash: from_pub_key_hash,
        };

        to_balance_tree.insert(
            token,
            Balance {
                value: to_balance_before_as_field_element,
            },
        );
        let to_leaf_initial = CircuitAccount::<Bn256> {
            subtree: to_balance_tree,
            nonce: Fr::zero(),
            pub_key_hash: to_pub_key_hash,
        };
        tree.insert(from_leaf_number, from_leaf_initial);
        tree.insert(to_leaf_number, to_leaf_initial);

        let transfer_witness = apply_transfer(
            &mut tree,
            &TransferData {
                amount: transfer_amount,
                fee,
                token,
                from_account_address: from_leaf_number,
                to_account_address: to_leaf_number,
            },
        );
        println!(
            "transfer_witness calculated a is: {}",
            transfer_witness.args.a.unwrap()
        );
        let (signature_data, first_sig_part, second_sig_part, third_sig_part) =
            generate_sig_data(&transfer_witness.get_sig_bits(), &phasher, &from_sk, params);

        let packed_public_key = PackedPublicKey(from_pk);
        let packed_public_key_bytes = packed_public_key.serialize_packed().unwrap();
        let signer_packed_key_bits: Vec<_> = bytes_into_be_bits(&packed_public_key_bytes)
            .iter()
            .map(|x| Some(*x))
            .collect();
        let operations = calculate_transfer_operations_from_witness(
            &transfer_witness,
            &first_sig_part,
            &second_sig_part,
            &third_sig_part,
            &signature_data,
            &signer_packed_key_bits,
        );

        let (root_after_fee, validator_account_witness) =
            apply_fee(&mut tree, validator_address_number, token, fee);

        let (validator_audit_path, _) = get_audits(&tree, validator_address_number, 0);
        let public_data_commitment = public_data_commitment::<Bn256>(
            &transfer_witness.get_pubdata(),
            transfer_witness.before_root,
            Some(root_after_fee),
            Some(validator_address),
            Some(block_number),
        );

        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let instance = FranklinCircuit {
                operation_batch_size: 10,
                params,
                old_root: transfer_witness.before_root,
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

            if let Some(err) = cs.which_is_unsatisfied() {
                panic!("ERROR satisfying in {}", err);
            }
        }
    }
}
