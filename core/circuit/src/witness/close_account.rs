// Workspace deps
use models::{
    circuit::{
        account::CircuitAccountTree,
        utils::{append_be_fixed_width, le_bit_vector_into_field_element},
    },
    node::operations::CloseOp,
    params as franklin_constants,
};
// Local deps
use crate::franklin_crypto::{
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::{Field, PrimeField},
    },
    rescue::RescueEngine,
};
use crate::{
    operation::{
        Operation, OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
    },
    witness::utils::{apply_leaf_operation, get_audits},
};

pub struct CloseAccountData {
    pub account_address: u32,
}
pub struct CloseAccountWitness<E: RescueEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}

impl<E: RescueEngine> CloseAccountWitness<E> {
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

        pubdata_bits.resize(franklin_constants::CHUNK_BIT_WIDTH, false);
        pubdata_bits
    }
    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("4").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );

        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}

pub fn apply_close_account_tx(
    tree: &mut CircuitAccountTree,
    close_account: &CloseOp,
) -> CloseAccountWitness<Bn256> {
    let close_acoount_data = CloseAccountData {
        account_address: close_account.account_id as u32,
    };
    apply_close_account(tree, &close_acoount_data)
}

pub fn apply_close_account(
    tree: &mut CircuitAccountTree,
    close_account: &CloseAccountData,
) -> CloseAccountWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    debug!("Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, close_account.account_address, 0);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_address_fe = Fr::from_str(&close_account.account_address.to_string()).unwrap();

    //calculate a and b
    let a = Fr::zero();
    let b = Fr::zero();

    //applying close_account
    let (account_witness_before, account_witness_after, balance_before, balance_after) =
        apply_leaf_operation(
            tree,
            close_account.account_address,
            0,
            |acc| {
                acc.pub_key_hash = Fr::zero();
                acc.nonce = Fr::zero();
            },
            |_| {},
        );

    let after_root = tree.root_hash();
    debug!("After root = {}", after_root);
    let (audit_path_after, audit_balance_path_after) =
        get_audits(tree, close_account.account_address, 0);

    CloseAccountWitness {
        before: OperationBranch {
            address: Some(account_address_fe),
            token: Some(Fr::zero()),
            witness: OperationBranchWitness {
                account_witness: account_witness_before,
                account_path: audit_path_before,
                balance_value: Some(balance_before),
                balance_subtree_path: audit_balance_path_before,
            },
        },
        after: OperationBranch {
            address: Some(account_address_fe),
            token: Some(Fr::zero()),
            witness: OperationBranchWitness {
                account_witness: account_witness_after,
                account_path: audit_path_after,
                balance_value: Some(balance_after),
                balance_subtree_path: audit_balance_path_after,
            },
        },
        args: OperationArguments {
            eth_address: Some(Fr::zero()),
            amount_packed: Some(Fr::zero()),
            full_amount: Some(Fr::zero()),
            pub_nonce: Some(Fr::zero()),
            fee: Some(Fr::zero()),
            a: Some(a),
            b: Some(b),
            new_pub_key_hash: Some(Fr::zero()),
        },
        before_root: Some(before_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("4").unwrap()),
    }
}

pub fn calculate_close_account_operations_from_witness(
    close_account_witness: &CloseAccountWitness<Bn256>,
    first_sig_msg: &Fr,
    second_sig_msg: &Fr,
    third_sig_msg: &Fr,
    signature_data: &SignatureData,
    signer_pub_key_packed: &[Option<bool>],
) -> Vec<Operation<Bn256>> {
    let pubdata_chunks: Vec<_> = close_account_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();
    let operation_zero = Operation {
        new_root: close_account_witness.after_root,
        tx_type: close_account_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signature_data: signature_data.clone(),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: close_account_witness.args.clone(),
        lhs: close_account_witness.before.clone(),
        rhs: close_account_witness.before.clone(),
    };

    let operations: Vec<Operation<_>> = vec![operation_zero];
    operations
}

// Close disabled
//
//#[cfg(test)]
//mod test {
//    use super::*;
//    use crate::witness::utils::public_data_commitment;
//    use models::merkle_tree::PedersenHasher;
//    use models::primitives::bytes_into_be_bits;
//
//    use crate::circuit::FranklinCircuit;
//    use bellman::Circuit;
//    use crate::franklin_crypto::bellman::pairing::ff::{Field, PrimeField};
//    use crate::franklin_crypto::alt_babyjubjub::AltJubjubBn256;
//    use crate::franklin_crypto::circuit::test::*;
//    use crate::franklin_crypto::eddsa::{PrivateKey, PublicKey};
//    use crate::franklin_crypto::jubjub::FixedGenerators;
//    use models::circuit::account::{CircuitAccount, CircuitAccountTree, CircuitBalanceTree};
//    use models::circuit::utils::*;
//    use models::node::tx::PackedPublicKey;
//    use models::params as franklin_constants;
//    use rand::{Rng, SeedableRng, XorShiftRng};

//    #[test]
//    #[ignore]
//    fn test_close_account_franklin_empty_leaf() {
//        let params = &AltJubjubBn256::new();
//        let p_g = FixedGenerators::SpendingKeyGenerator;
//        let validator_address_number = 7;
//        let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
//        let block_number = Fr::from_str("1").unwrap();
//        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
//        let phasher = PedersenHasher::<Bn256>::default();
//
//        let mut tree: CircuitAccountTree =
//            CircuitAccountTree::new(franklin_constants::account_tree_depth() as u32);
//        let capacity = tree.capacity();
//
//        let sender_sk = PrivateKey::<Bn256>(rng.gen());
//        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
//        let sender_pub_key_hash = pub_key_hash_fe(&sender_pk, &phasher);
//        let sender_leaf = CircuitAccount::<Bn256> {
//            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
//            nonce: Fr::zero(),
//            pub_key_hash: sender_pub_key_hash,
//        };
//        let mut sender_leaf_number: u32 = rng.gen();
//        sender_leaf_number %= capacity;
//        println!("zero root_hash equals: {}", sender_leaf.subtree.root_hash());
//
//        tree.insert(sender_leaf_number, sender_leaf);
//
//        // give some funds to sender and make zero balance for recipient
//        let validator_sk = PrivateKey::<Bn256>(rng.gen());
//        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
//        let validator_pub_key_hash = pub_key_hash_fe(&validator_pk, &phasher);
//
//        let validator_leaf = CircuitAccount::<Bn256> {
//            subtree: CircuitBalanceTree::new(franklin_constants::BALANCE_TREE_DEPTH as u32),
//            nonce: Fr::zero(),
//            pub_key_hash: validator_pub_key_hash,
//        };
//
//        let mut validator_balances = vec![];
//        for _ in 0..1 << franklin_constants::BALANCE_TREE_DEPTH {
//            validator_balances.push(Some(Fr::zero()));
//        }
//        tree.insert(validator_address_number, validator_leaf);
//
//        let account_address = sender_leaf_number;
//
//        //-------------- Start applying changes to state
//        let close_account_witness =
//            apply_close_account(&mut tree, &CloseAccountData { account_address });
//        let (signature_data, first_sig_part, second_sig_part, third_sig_part) = generate_sig_data(
//            &close_account_witness.get_sig_bits(),
//            &phasher,
//            &sender_sk,
//            params,
//        );
//        let packed_public_key = PackedPublicKey(sender_pk);
//        let packed_public_key_bytes = packed_public_key.serialize_packed().unwrap();
//        let signer_packed_key_bits: Vec<_> = bytes_into_be_bits(&packed_public_key_bytes)
//            .iter()
//            .map(|x| Some(*x))
//            .collect();
//
//        let operations = calculate_close_account_operations_from_witness(
//            &close_account_witness,
//            &first_sig_part,
//            &second_sig_part,
//            &third_sig_part,
//            &signature_data,
//            &signer_packed_key_bits,
//        );
//
//        println!("tree before_applying fees: {}", tree.root_hash());
//
//        let (root_after_fee, validator_account_witness) =
//            apply_fee(&mut tree, validator_address_number, 0, 0);
//        println!("test root after fees {}", root_after_fee);
//        let (validator_audit_path, _) = get_audits(&tree, validator_address_number, 0);
//
//        let public_data_commitment = public_data_commitment::<Bn256>(
//            &close_account_witness.get_pubdata(),
//            close_account_witness.before_root,
//            Some(root_after_fee),
//            Some(validator_address),
//            Some(block_number),
//        );
//
//        {
//            let mut cs = TestConstraintSystem::<Bn256>::new();
//
//            let instance = FranklinCircuit {
//                operation_batch_size: 10,
//                params,
//                old_root: close_account_witness.before_root,
//                new_root: Some(root_after_fee),
//                operations,
//                pub_data_commitment: Some(public_data_commitment),
//                block_number: Some(block_number),
//                validator_account: validator_account_witness,
//                validator_address: Some(validator_address),
//                validator_balances,
//                validator_audit_path,
//            };
//
//            instance.synthesize(&mut cs).unwrap();
//
//            println!("{}", cs.find_unconstrained());
//
//            println!("number of constraints {}", cs.num_constraints());
//            if let Some(err) = cs.which_is_unsatisfied() {
//                panic!("ERROR satisfying in {}", err);
//            }
//        }
//    }
//}
