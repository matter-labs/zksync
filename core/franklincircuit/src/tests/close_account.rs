use super::utils::*;

use crate::operation::*;
use crate::utils::*;

use ff::{BitIterator, Field, PrimeField, PrimeFieldRepr};

use crate::account::AccountWitness;
use franklin_crypto::circuit::float_point::{convert_to_float, parse_float_to_u128};
use franklin_crypto::jubjub::JubjubEngine;
use franklinmodels::circuit::account::{
    Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree,
};
use franklinmodels::merkle_tree::hasher::Hasher;
use franklinmodels::merkle_tree::PedersenHasher;
use franklinmodels::params as franklin_constants;
use pairing::bn256::*;

pub struct CloseAccountData {
    pub account_address: u32,
}
pub struct CloseAccountWitness<E: JubjubEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}
impl<E: JubjubEngine> CloseAccountWitness<E> {
    pub fn get_pubdata(&self) -> Vec<bool> {
        let mut pubdata_bits = vec![];
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.tx_type.unwrap(),
            *franklin_constants::TX_TYPE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.address.unwrap(),
            franklin_constants::ACCOUNT_TREE_DEPTH,
        );

        assert_eq!(pubdata_bits.len(), 4 * 8);
        pubdata_bits.resize(8 * 8, false);
        pubdata_bits
    }
}
pub fn apply_close_account(
    tree: &mut CircuitAccountTree,
    close_account: &CloseAccountData,
) -> CloseAccountWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    println!("Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, close_account.account_address, 0);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::ACCOUNT_TREE_DEPTH);
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
    println!("After root = {}", after_root);
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
            ethereum_key: Some(Fr::zero()),
            amount: Some(Fr::zero()),
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
    sig_msg: &Fr,
    signature: Option<TransactionSignature<Bn256>>,
    signer_pub_key_x: &Fr,
    signer_pub_key_y: &Fr,
) -> Vec<Operation<Bn256>> {
    let pubdata_chunks: Vec<_> = close_account_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();
    let operation_zero = Operation {
        new_root: close_account_witness.after_root.clone(),
        tx_type: close_account_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(signer_pub_key_x.clone()),
        signer_pub_key_y: Some(signer_pub_key_y.clone()),
        args: close_account_witness.args.clone(),
        lhs: close_account_witness.before.clone(),
        rhs: close_account_witness.before.clone(),
    };

    let operations: Vec<Operation<_>> = vec![operation_zero];
    operations
}
#[test]
fn test_close_account_franklin_empty_leaf() {
    use super::utils::public_data_commitment;

    use crate::circuit::FranklinCircuit;
    use crate::operation::*;
    use crate::utils::*;
    use bellman::Circuit;

    use ff::{BitIterator, Field, PrimeField};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;

    use franklin_crypto::circuit::test::*;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklin_crypto::jubjub::FixedGenerators;
    use franklinmodels::circuit::account::{
        Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree,
    };
    use franklinmodels::params as franklin_constants;

    use pairing::bn256::*;
    use rand::{Rng, SeedableRng, XorShiftRng};

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
    let sender_leaf = CircuitAccount::<Bn256> {
        subtree: CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32),
        nonce: Fr::zero(),
        pub_key_hash: sender_pub_key_hash
        // pub_x: validator_x.clone(),
        // pub_y: validator_y.clone(),
    };
    println!("zero root_hash equals: {}", sender_leaf.subtree.root_hash());

    // give some funds to sender and make zero balance for recipient
    let validator_sk = PrivateKey::<Bn256>(rng.gen());
    let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
    let validator_pub_key_hash = pub_key_hash(&validator_pk, &phasher);
    let (validator_x, validator_y) = validator_pk.0.into_xy();

    let validator_leaf = CircuitAccount::<Bn256> {
        subtree: CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32),
        nonce: Fr::zero(),
        pub_key_hash: validator_pub_key_hash,
    };

    let mut validator_balances = vec![];
    for _ in 0..1 << *franklin_constants::BALANCE_TREE_DEPTH {
        validator_balances.push(Some(Fr::zero()));
    }
    tree.insert(validator_address_number, validator_leaf);

    let mut account_address: u32 = rng.gen();
    account_address %= tree.capacity();

    //-------------- Start applying changes to state
    let close_account_witness = apply_close_account(
        &mut tree,
        &CloseAccountData {
            account_address: account_address,
        },
    );

    let sig_msg = Fr::from_str("2").unwrap(); //dummy sig msg cause skipped on close_account proof
    let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
    sig_bits.reverse();
    sig_bits.truncate(80);

    // println!(" capacity {}",<Bn256 as JubjubEngine>::Fs::Capacity);
    let signature = sign(&sig_bits, &sender_sk, p_g, params, rng);
    //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));

    let operations = calculate_close_account_operations_from_witness(
        &close_account_witness,
        &sig_msg,
        signature,
        &sender_x,
        &sender_y,
    );

    println!("tree before_applying fees: {}", tree.root_hash());

    let (root_after_fee, validator_account_witness) =
        apply_fee(&mut tree, validator_address_number, 0, 0);
    println!("test root after fees {}", root_after_fee);
    let (validator_audit_path, _) = get_audits(&mut tree, validator_address_number, 0);

    let public_data_commitment = public_data_commitment::<Bn256>(
        &close_account_witness.get_pubdata(),
        close_account_witness.before_root,
        Some(root_after_fee),
        Some(validator_address),
        Some(block_number),
    );

    {
        let mut cs = TestConstraintSystem::<Bn256>::new();

        let instance = FranklinCircuit {
            operation_batch_size: 10,
            params,
            old_root: close_account_witness.before_root,
            new_root: Some(root_after_fee),
            operations: operations,
            pub_data_commitment: Some(public_data_commitment),
            block_number: Some(block_number),
            validator_account: validator_account_witness,
            validator_address: Some(validator_address),
            validator_balances: validator_balances,
            validator_audit_path: validator_audit_path,
        };

        instance.synthesize(&mut cs).unwrap();

        println!("{}", cs.find_unconstrained());

        println!("number of constraints {}", cs.num_constraints());
        let err = cs.which_is_unsatisfied();
        if err.is_some() {
            panic!("ERROR satisfying in {}", err.unwrap());
        }
        // assert_eq!(cs.num_constraints(), 1)
    }
}
