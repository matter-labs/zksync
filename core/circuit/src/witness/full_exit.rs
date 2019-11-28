use super::utils::*;

use crate::operation::SignatureData;
use crate::operation::*;
use ff::{Field, PrimeField};
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{append_be_fixed_width, le_bit_vector_into_field_element};
use models::node::FullExitOp;
use models::params as franklin_constants;
use pairing::bn256::*;
pub struct FullExitData {
    pub token: u32,
    pub account_address: u32,
    pub ethereum_key: Fr,
    pub pub_nonce: Fr,
}
pub struct FullExitWitness<E: JubjubEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}
impl<E: JubjubEngine> FullExitWitness<E> {
    pub fn get_pubdata(&self, sig_data: &SignatureData, signer_pubkey: &[bool]) -> Vec<bool> {
        assert_eq!(signer_pubkey.len(), 256);
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
        pubdata_bits.extend(signer_pubkey.to_vec());

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.ethereum_key.unwrap(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );

        pubdata_bits.extend(sig_data.r_packed.iter().map(|x| x.unwrap()));
        pubdata_bits.extend(sig_data.s.iter().map(|x| x.unwrap()));

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.full_amount.unwrap(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        pubdata_bits.resize(18 * franklin_constants::CHUNK_BIT_WIDTH, false);
        // println!("pub_data outside: ");
        // for (i, bit) in pubdata_bits.iter().enumerate() {
        //     if i % 64 == 0 {
        //         println!("")
        //     } else if i % 8 == 0 {
        //         print!(" ")
        //     };
        //     let numb = {
        //         if *bit {
        //             1
        //         } else {
        //             0
        //         }
        //     };
        //     print!("{}", numb);
        // }
        // println!("");
        pubdata_bits
    }

    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("6").unwrap(), //Corresponding tx_type
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
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}
pub fn apply_full_exit_tx(
    tree: &mut CircuitAccountTree,
    full_exit: &FullExitOp,
    is_sig_valid: bool,
) -> FullExitWitness<Bn256> {
    let full_exit = FullExitData {
        token: u32::from(full_exit.priority_op.token),
        account_address: full_exit
            .account_with_id
            .as_ref()
            .map(|(id, _)| *id)
            .unwrap_or(0),
        ethereum_key: Fr::from_hex(&format!("{:x}", &full_exit.priority_op.eth_address)).unwrap(),
        pub_nonce: Fr::from_str(&full_exit.priority_op.nonce.to_string()).unwrap(),
    };
    // le_bit_vector_into_field_element()
    apply_full_exit(tree, &full_exit, is_sig_valid)
}
pub fn apply_full_exit(
    tree: &mut CircuitAccountTree,
    full_exit: &FullExitData,
    is_sig_valid: bool,
) -> FullExitWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    println!("Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, full_exit.account_address, full_exit.token);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_address_fe = Fr::from_str(&full_exit.account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&full_exit.token.to_string()).unwrap();

    let nonce_valid = tree
        .get(full_exit.account_address)
        .map(|acc| acc.nonce == full_exit.pub_nonce)
        .unwrap_or(false);

    //applying full_exit
    let amount_to_exit = {
        let (_, _, balance, _) = apply_leaf_operation(
            tree,
            full_exit.account_address,
            full_exit.token,
            |_| {},
            |_| {},
        );
        if is_sig_valid && nonce_valid {
            balance
        } else {
            Fr::zero()
        }
    };

    let (account_witness_before, account_witness_after, balance_before, balance_after) = {
        if is_sig_valid && nonce_valid {
            apply_leaf_operation(
                tree,
                full_exit.account_address,
                full_exit.token,
                |acc| {
                    acc.nonce.add_assign(&Fr::from_str("1").unwrap());
                },
                |bal| {
                    bal.value = Fr::zero();
                },
            )
        } else {
            apply_leaf_operation(
                tree,
                full_exit.account_address,
                full_exit.token,
                |_| {},
                |_| {},
            )
        }
    };

    let after_root = tree.root_hash();
    println!("After root = {}", after_root);
    let (audit_path_after, audit_balance_path_after) =
        get_audits(tree, full_exit.account_address, full_exit.token);

    let a = balance_before;
    let b = Fr::zero();

    FullExitWitness {
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
            ethereum_key: Some(full_exit.ethereum_key),
            amount_packed: Some(Fr::zero()),
            full_amount: Some(amount_to_exit),
            fee: Some(Fr::zero()),
            pub_nonce: Some(full_exit.pub_nonce),
            a: Some(a),
            b: Some(b),
            new_pub_key_hash: Some(Fr::zero()),
        },
        before_root: Some(before_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("6").unwrap()),
    }
}
pub fn calculate_full_exit_operations_from_witness(
    full_exit_witness: &FullExitWitness<Bn256>,
    first_sig_msg: &Fr,
    second_sig_msg: &Fr,
    third_sig_msg: &Fr,
    signature_data: &SignatureData,
    signer_pub_key_packed: &[Option<bool>],
) -> Vec<Operation<Bn256>> {
    let signer_pub_key_bits: Vec<bool> = signer_pub_key_packed
        .to_vec()
        .iter()
        .map(|x| x.unwrap())
        .collect();
    let pubdata_chunks: Vec<_> = full_exit_witness
        .get_pubdata(signature_data, &signer_pub_key_bits)
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();

    let mut operations = vec![];
    operations.push(Operation {
        new_root: full_exit_witness.after_root,
        tx_type: full_exit_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: full_exit_witness.args.clone(),
        lhs: full_exit_witness.before.clone(),
        rhs: full_exit_witness.before.clone(),
        signature_data: signature_data.clone(),
    });

    for (i, pubdata_chunk) in pubdata_chunks.iter().cloned().enumerate().take(18).skip(1) {
        operations.push(Operation {
            new_root: full_exit_witness.after_root,
            tx_type: full_exit_witness.tx_type,
            chunk: Some(Fr::from_str(&i.to_string()).unwrap()),
            pubdata_chunk: Some(pubdata_chunk),
            first_sig_msg: Some(*first_sig_msg),
            second_sig_msg: Some(*second_sig_msg),
            third_sig_msg: Some(*third_sig_msg),
            signer_pub_key_packed: signer_pub_key_packed.to_vec(),
            args: full_exit_witness.args.clone(),
            lhs: full_exit_witness.after.clone(),
            rhs: full_exit_witness.after.clone(),
            signature_data: signature_data.clone(),
        });
    }

    operations
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::witness::utils::public_data_commitment;
    use models::circuit::utils::*;
    use models::primitives::bytes_into_be_bits;

    use crate::circuit::FranklinCircuit;
    use bellman::Circuit;
    use ff::{BitIterator, Field, PrimeField};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
    use franklin_crypto::circuit::test::*;
    use franklin_crypto::eddsa::Signature;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklin_crypto::jubjub::edwards;
    use franklin_crypto::jubjub::FixedGenerators;
    use models::circuit::account::{
        Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree,
    };
    use models::merkle_tree::hasher::Hasher;
    use models::merkle_tree::PedersenHasher;
    use models::node::tx::{PackedPublicKey, TxSignature};
    use models::node::Fs;
    use models::params as franklin_constants;
    use rand::{Rng, SeedableRng, XorShiftRng};

    #[test]
    #[ignore]
    fn test_full_exit_franklin_success() {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let validator_address_number = 7;
        let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
        let block_number = Fr::from_str("1").unwrap();
        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let phasher = PedersenHasher::<Bn256>::default();

        let mut tree: CircuitAccountTree =
            CircuitAccountTree::new(franklin_constants::account_tree_depth() as u32);

        let sender_sk = PrivateKey::<Bn256>(Fs::from_str("5").unwrap());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let sender_pub_key_hash = pub_key_hash_fe(&sender_pk, &phasher);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
        println!("x = {}, y = {}", sender_x, sender_y);

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let validator_pub_key_hash = pub_key_hash_fe(&validator_pk, &phasher);
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
        let token: u32 = 2;
        let token_fe = Fr::from_str(&token.to_string()).unwrap();
        let ethereum_key = Fr::from_str("0").unwrap();

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

        let packed_public_key = PackedPublicKey(sender_pk.clone());
        let packed_public_key_bytes = packed_public_key.serialize_packed().unwrap();
        let _signer_packed_key_bits: Vec<_> = bytes_into_be_bits(&packed_public_key_bytes)
            .iter()
            .map(|x| Some(*x))
            .collect();

        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("6").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        sig_bits.extend(&bytes_into_be_bits(&packed_public_key_bytes));
        append_be_fixed_width(
            &mut sig_bits,
            &ethereum_key,
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &token_fe,
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &sender_leaf_initial.nonce,
            franklin_constants::NONCE_BIT_WIDTH,
        );
        let message_bytes = be_bit_vector_into_bytes(&sig_bits);

        tree.insert(account_address, sender_leaf_initial);

        println!("message bytes: {}", hex::encode(&message_bytes));

        let signature = TxSignature::sign_musig_pedersen(&sender_sk, &message_bytes);
        let (r_bytes, s_bytes) = {
            let sign_bytes = signature.signature.serialize_packed().unwrap();
            let (r_slice, s_slice) = sign_bytes.split_at(32);
            (r_slice.to_vec(), s_slice.to_vec())
        };
        let is_sig_correct = signature.verify_musig_pedersen(&message_bytes).is_some();

        let r_bits: Vec<_> = bytes_into_be_bits(&r_bytes)
            .iter()
            .map(|x| Some(*x))
            .collect();
        let s_bits: Vec<_> = bytes_into_be_bits(&s_bytes)
            .iter()
            .map(|x| Some(*x))
            .collect();
        let signature_data = SignatureData {
            r_packed: r_bits,
            s: s_bits,
        };

        let (first_sig_msg, second_sig_msg, third_sig_msg) =
            generate_sig_witness(&sig_bits, &phasher, &params);
        let signer_packed_key_bytes = signature.pub_key.serialize_packed().unwrap();
        let signer_packed_key_bits: Vec<_> = bytes_into_be_bits(&signer_packed_key_bytes)
            .iter()
            .map(|x| Some(*x))
            .collect();

        assert!(is_sig_correct, "Signature is incorrect");

        let full_exit_witness = apply_full_exit(
            &mut tree,
            &FullExitData {
                token,
                account_address,
                ethereum_key,
                pub_nonce: Fr::zero(),
            },
            is_sig_correct,
        );

        let operations = calculate_full_exit_operations_from_witness(
            &full_exit_witness,
            &first_sig_msg,
            &second_sig_msg,
            &third_sig_msg,
            &signature_data,
            &signer_packed_key_bits,
        );

        let (root_after_fee, validator_account_witness) =
            apply_fee(&mut tree, validator_address_number, 0, 0);

        let (validator_audit_path, _) = get_audits(&tree, validator_address_number, 0);
        let public_data_commitment = public_data_commitment::<Bn256>(
            &full_exit_witness.get_pubdata(
                &signature_data,
                &bytes_into_be_bits(&packed_public_key_bytes),
            ),
            full_exit_witness.before_root,
            Some(root_after_fee),
            Some(validator_address),
            Some(block_number),
        );
        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let instance = FranklinCircuit {
                operation_batch_size: 10,
                params,
                old_root: full_exit_witness.before_root,
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

    #[test]
    #[ignore]
    fn test_full_exit_franklin_failure() {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let validator_address_number = 7;
        let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
        let block_number = Fr::from_str("1").unwrap();
        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let phasher = PedersenHasher::<Bn256>::default();

        let mut tree: CircuitAccountTree =
            CircuitAccountTree::new(franklin_constants::account_tree_depth() as u32);

        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let sender_pub_key_hash = pub_key_hash_fe(&sender_pk, &phasher);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
        println!("x = {}, y = {}", sender_x, sender_y);

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let validator_pub_key_hash = pub_key_hash_fe(&validator_pk, &phasher);
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
        let token: u32 = 2;
        let token_fe = Fr::from_str(&token.to_string()).unwrap();
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

        let packed_public_key = PackedPublicKey(sender_pk.clone());
        let packed_public_key_bytes = packed_public_key.serialize_packed().unwrap();
        let signer_packed_key_bits: Vec<_> = bytes_into_be_bits(&packed_public_key_bytes)
            .iter()
            .map(|x| Some(*x))
            .collect();

        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("6").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        sig_bits.extend(&bytes_into_be_bits(&packed_public_key_bytes));
        append_be_fixed_width(
            &mut sig_bits,
            &ethereum_key,
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &token_fe,
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &sender_leaf_initial.nonce,
            franklin_constants::NONCE_BIT_WIDTH,
        );
        tree.insert(account_address, sender_leaf_initial);

        sig_bits.resize(franklin_constants::MAX_CIRCUIT_PEDERSEN_HASH_BITS, false);
        println!(
            "outside generation after resize: {}",
            hex::encode(be_bit_vector_into_bytes(&sig_bits))
        );

        let sig_msg = phasher.hash_bits(sig_bits.clone());

        let mut sig_bits_to_check: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
        sig_bits_to_check.reverse();
        sig_bits_to_check.resize(256, false);
        println!(
            "outside generation: {}",
            hex::encode(be_bit_vector_into_bytes(&sig_bits_to_check))
        );
        let mut message_bytes = vec![];
        let byte_chunks = sig_bits_to_check.chunks(8);
        for byte_chunk in byte_chunks {
            let mut byte = 0u8;
            for (i, bit) in byte_chunk.iter().enumerate() {
                if *bit {
                    byte |= 1 << i;
                }
            }
            message_bytes.push(byte);
        }

        let (signature, first_sig_part, second_sig_part, third_sig_part) =
            generate_sig_data(&sig_bits, &phasher, &sender_sk, params);

        // move to func
        let r_x_bit = signature.r_packed[0].unwrap();

        let mut r_y_bits: Vec<bool> = signature.r_packed[1..].iter().map(|x| x.unwrap()).collect();
        r_y_bits.reverse();
        let r_y = le_bit_vector_into_field_element(&r_y_bits);

        let mut s_bits: Vec<bool> =
            BitIterator::new(Fr::from_str("5").unwrap().into_repr()).collect();
        s_bits.reverse();
        s_bits.resize(franklin_constants::FR_BIT_WIDTH_PADDED, false);

        let s: <Bn256 as JubjubEngine>::Fs = le_bit_vector_into_field_element(&s_bits);
        let r = edwards::Point::get_for_y(r_y, r_x_bit, params);

        let (is_sig_correct, _) = match r {
            None => (false, None),
            Some(r) => {
                let sig = Signature { r, s };
                let is_valid_signature =
                    sender_pk.verify_musig_pedersen(&message_bytes, &sig, p_g, params);
                (is_valid_signature, Some(sig))
            }
        };

        assert_eq!(is_sig_correct, false);

        let mut signature_data = signature.clone();

        signature_data.s = s_bits.clone().iter().map(|x| Some(*x)).collect();

        let full_exit_witness = apply_full_exit(
            &mut tree,
            &FullExitData {
                token,
                account_address,
                ethereum_key,
                pub_nonce: Fr::zero(),
            },
            is_sig_correct,
        );

        let operations = calculate_full_exit_operations_from_witness(
            &full_exit_witness,
            &first_sig_part,
            &second_sig_part,
            &third_sig_part,
            &signature_data,
            &signer_packed_key_bits,
        );

        let (root_after_fee, validator_account_witness) =
            apply_fee(&mut tree, validator_address_number, token, 0);

        let (validator_audit_path, _) = get_audits(&tree, validator_address_number, 0);
        let public_data_commitment = public_data_commitment::<Bn256>(
            &full_exit_witness.get_pubdata(
                &signature_data,
                &bytes_into_be_bits(&packed_public_key_bytes),
            ),
            full_exit_witness.before_root,
            Some(root_after_fee),
            Some(validator_address),
            Some(block_number),
        );
        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let instance = FranklinCircuit {
                operation_batch_size: 10,
                params,
                old_root: full_exit_witness.before_root,
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
