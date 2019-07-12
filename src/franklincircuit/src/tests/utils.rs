use crate::account::*;

use crate::utils::*;

use crypto::digest::Digest;
use crypto::sha2::Sha256;
use ff::Field;
use ff::{BitIterator, PrimeField, PrimeFieldRepr};

use franklin_crypto::jubjub::JubjubEngine;
use franklinmodels::circuit::account::{Balance, CircuitAccount, CircuitAccountTree};

use pairing::bn256::*;

pub fn public_data_commitment<E: JubjubEngine>(
    pubdata_bits: &[bool],
    initial_root: Option<E::Fr>,
    new_root: Option<E::Fr>,
    validator_address: Option<E::Fr>,
    block_number: Option<E::Fr>,
) -> E::Fr {
    let mut public_data_initial_bits = vec![];

    // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

    let block_number_bits: Vec<bool> =
        BitIterator::new(block_number.unwrap().into_repr()).collect();
    for _ in 0..256 - block_number_bits.len() {
        public_data_initial_bits.push(false);
    }
    public_data_initial_bits.extend(block_number_bits.into_iter());

    let validator_id_bits: Vec<bool> =
        BitIterator::new(validator_address.unwrap().into_repr()).collect();
    for _ in 0..256 - validator_id_bits.len() {
        public_data_initial_bits.push(false);
    }
    public_data_initial_bits.extend(validator_id_bits.into_iter());

    assert_eq!(public_data_initial_bits.len(), 512);

    let mut h = Sha256::new();

    let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

    h.input(&bytes_to_hash);

    let mut hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    println!("Initial hash hex {}", hex::encode(hash_result));

    let mut packed_old_root_bits = vec![];
    let old_root_bits: Vec<bool> = BitIterator::new(initial_root.unwrap().into_repr()).collect();
    for _ in 0..256 - old_root_bits.len() {
        packed_old_root_bits.push(false);
    }
    packed_old_root_bits.extend(old_root_bits);

    let packed_old_root_bytes = be_bit_vector_into_bytes(&packed_old_root_bits);

    let mut packed_with_old_root = vec![];
    packed_with_old_root.extend(hash_result.iter());
    packed_with_old_root.extend(packed_old_root_bytes);

    h = Sha256::new();
    h.input(&packed_with_old_root);
    hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    let mut packed_new_root_bits = vec![];
    let new_root_bits: Vec<bool> = BitIterator::new(new_root.unwrap().into_repr()).collect();
    for _ in 0..256 - new_root_bits.len() {
        packed_new_root_bits.push(false);
    }
    packed_new_root_bits.extend(new_root_bits);

    let packed_new_root_bytes = be_bit_vector_into_bytes(&packed_new_root_bits);

    let mut packed_with_new_root = vec![];
    packed_with_new_root.extend(hash_result.iter());
    packed_with_new_root.extend(packed_new_root_bytes);

    h = Sha256::new();
    h.input(&packed_with_new_root);
    hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    println!("hash with new root as hex {}", hex::encode(hash_result));

    let mut final_bytes = vec![];
    let pubdata_bytes = be_bit_vector_into_bytes(&pubdata_bits.to_vec());
    final_bytes.extend(hash_result.iter());
    final_bytes.extend(pubdata_bytes);

    h = Sha256::new();
    h.input(&final_bytes);
    hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    println!("final hash as hex {}", hex::encode(hash_result));

    hash_result[0] &= 0x1f; // temporary solution, this nullifies top bits to be encoded into field element correctly

    let mut repr = E::Fr::zero().into_repr();
    repr.read_be(&hash_result[..])
        .expect("pack hash as field element");

    let public_data_commitment = E::Fr::from_repr(repr).unwrap();
    public_data_commitment
}

pub fn get_audits(
    tree: &mut CircuitAccountTree,
    account_address: u32,
    token: u32,
) -> (Vec<Option<Fr>>, Vec<Option<Fr>>) {
    let default_account = CircuitAccount::default();
    let audit_account: Vec<Option<Fr>> = tree
        .merkle_path(account_address)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let audit_balance: Vec<Option<Fr>> = tree
        .get(account_address)
        .unwrap_or(&default_account)
        .subtree
        .merkle_path(token)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();
    (audit_account, audit_balance)
}

pub fn apply_leaf_operation<
    Fa: Fn(&mut CircuitAccount<Bn256>) -> (),
    Fb: Fn(&mut Balance<Bn256>) -> (),
>(
    tree: &mut CircuitAccountTree,
    account_address: u32,
    token: u32,
    fa: Fa,
    fb: Fb,
) -> (AccountWitness<Bn256>, AccountWitness<Bn256>, Fr, Fr) {
    let default_account = CircuitAccount::default();

    //applying deposit
    let mut account = tree.remove(account_address).unwrap_or(default_account);
    let account_witness_before = AccountWitness {
        nonce: Some(account.nonce),
        pub_x: Some(account.pub_x),
        pub_y: Some(account.pub_y),
    };
    let mut balance = account
        .subtree
        .remove(token)
        .unwrap_or(Balance { value: Fr::zero() });
    let balance_before = balance.value.clone();
    fb(&mut balance);
    let balance_after = balance.value.clone();
    account.subtree.insert(token, balance.clone());

    fa(&mut account);

    let account_witness_after = AccountWitness {
        nonce: Some(account.nonce),
        pub_x: Some(account.pub_x),
        pub_y: Some(account.pub_y),
    };
    tree.insert(account_address, account);
    (
        account_witness_before,
        account_witness_after,
        balance_before,
        balance_after,
    )
}
