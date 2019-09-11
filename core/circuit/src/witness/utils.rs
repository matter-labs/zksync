use crate::account::*;

use crate::operation::TransactionSignature;
use crate::utils::*;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use ff::Field;
use ff::{BitIterator, PrimeField, PrimeFieldRepr};
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use franklin_crypto::eddsa::PrivateKey;
use franklin_crypto::eddsa::PublicKey;
use franklin_crypto::jubjub::FixedGenerators;
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::{Balance, CircuitAccount, CircuitAccountTree};
use models::merkle_tree::hasher::Hasher;
use models::merkle_tree::PedersenHasher;
use models::params as franklin_constants;
use pairing::bn256::*;
use rand::{Rng, SeedableRng, XorShiftRng};

pub fn _generate_dummy_sig_data() -> (Option<TransactionSignature<Bn256>>, Fr, Fr, Fr) {
    let params = &AltJubjubBn256::new();
    let p_g = FixedGenerators::SpendingKeyGenerator;
    let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
    let sender_sk = PrivateKey::<Bn256>(rng.gen());
    let sender_pk = PublicKey::from_private(&sender_sk, p_g, &params);
    let (sender_x, sender_y) = sender_pk.0.into_xy();
    let sig_msg = Fr::from_str("2").unwrap(); //dummy sig msg cause skipped on deposit proof
    let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
    sig_bits.reverse();
    sig_bits.truncate(80);

    let signature = sign_pedersen(&sig_bits, &sender_sk, p_g, &params, rng);
    (signature, sig_msg, sender_x, sender_y)
}
pub fn generate_dummy_sig_data(
    bits: &[bool],
    phasher: &PedersenHasher<Bn256>,
    params: &AltJubjubBn256,
) -> (Option<TransactionSignature<Bn256>>, Fr, Fr, Fr, Fr, Fr) {
    let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
    let p_g = FixedGenerators::SpendingKeyGenerator;
    let private_key = PrivateKey::<Bn256>(rng.gen());
    let sender_pk = PublicKey::from_private(&private_key, p_g, &params);
    let (sender_x, sender_y) = sender_pk.0.into_xy();
    let mut sig_bits_to_hash = bits.to_vec();
    assert!(sig_bits_to_hash.len() < franklin_constants::MAX_CIRCUIT_PEDERSEN_HASH_BITS);

    sig_bits_to_hash.resize(franklin_constants::MAX_CIRCUIT_PEDERSEN_HASH_BITS, false);
    let (first_sig_part_bits, remaining) = sig_bits_to_hash.split_at(Fr::CAPACITY as usize);
    let remaining = remaining.to_vec();
    let (second_sig_part_bits, third_sig_part_bits) = remaining.split_at(Fr::CAPACITY as usize);
    let first_sig_part: Fr = le_bit_vector_into_field_element(&first_sig_part_bits);
    let second_sig_part: Fr = le_bit_vector_into_field_element(&second_sig_part_bits);
    let third_sig_part: Fr = le_bit_vector_into_field_element(&third_sig_part_bits);
    let sig_msg = phasher.hash_bits(sig_bits_to_hash.clone());
    let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
    sig_bits.reverse();
    sig_bits.resize(256, false);

    let signature = sign_pedersen(&sig_bits, &private_key, p_g, params, rng);
    // let signature = sign_sha(&sig_bits, &private_key, p_g, params, rng);
    (
        signature,
        first_sig_part,
        second_sig_part,
        third_sig_part,
        sender_x,
        sender_y,
    )
}
pub fn generate_sig_data(
    bits: &[bool],
    phasher: &PedersenHasher<Bn256>,
    private_key: &PrivateKey<Bn256>,
    params: &AltJubjubBn256,
) -> (Option<TransactionSignature<Bn256>>, Fr, Fr, Fr) {
    let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
    let p_g = FixedGenerators::SpendingKeyGenerator;
    let mut sig_bits_to_hash = bits.to_vec();
    assert!(sig_bits_to_hash.len() <= franklin_constants::MAX_CIRCUIT_PEDERSEN_HASH_BITS);

    sig_bits_to_hash.resize(franklin_constants::MAX_CIRCUIT_PEDERSEN_HASH_BITS, false);
    println!(
        "inside generation after resize: {}",
        hex::encode(be_bit_vector_into_bytes(&sig_bits_to_hash))
    );

    let (first_sig_part_bits, remaining) = sig_bits_to_hash.split_at(Fr::CAPACITY as usize);
    let remaining = remaining.to_vec();
    let (second_sig_part_bits, third_sig_part_bits) = remaining.split_at(Fr::CAPACITY as usize);
    let first_sig_part: Fr = le_bit_vector_into_field_element(&first_sig_part_bits);
    let second_sig_part: Fr = le_bit_vector_into_field_element(&second_sig_part_bits);
    let third_sig_part: Fr = le_bit_vector_into_field_element(&third_sig_part_bits);
    let sig_msg = phasher.hash_bits(sig_bits_to_hash.clone());

    let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
    sig_bits.reverse();
    sig_bits.resize(256, false);

    println!(
        "inside generation: {}",
        hex::encode(be_bit_vector_into_bytes(&sig_bits))
    );
    let signature = sign_pedersen(&sig_bits, &private_key, p_g, params, rng);
    // let signature = sign_sha(&sig_bits, &private_key, p_g, params, rng);
    (signature, first_sig_part, second_sig_part, third_sig_part)
}
pub fn pub_key_hash<E: JubjubEngine, H: Hasher<E::Fr>>(
    pub_key: &PublicKey<E>,
    hasher: &H,
) -> E::Fr {
    let (pub_x, pub_y) = pub_key.0.into_xy();
    println!("x = {}, y = {}", pub_x, pub_y);
    let mut pub_key_bits = vec![];
    append_le_fixed_width(
        &mut pub_key_bits,
        &pub_x,
        franklin_constants::FR_BIT_WIDTH_PADDED,
    );
    append_le_fixed_width(
        &mut pub_key_bits,
        &pub_y,
        franklin_constants::FR_BIT_WIDTH_PADDED,
    );
    let pub_key_hash = hasher.hash_bits(pub_key_bits);
    let mut pub_key_hash_bits = vec![];
    append_le_fixed_width(
        &mut pub_key_hash_bits,
        &pub_key_hash,
        franklin_constants::NEW_PUBKEY_HASH_WIDTH,
    );
    le_bit_vector_into_field_element(&pub_key_hash_bits)
}

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

    E::Fr::from_repr(repr).unwrap()
}

pub fn get_audits(
    tree: &CircuitAccountTree,
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
        pub_key_hash: Some(account.pub_key_hash),
    };
    let mut balance = account
        .subtree
        .remove(token)
        .unwrap_or(Balance { value: Fr::zero() });
    let balance_before = balance.value;
    fb(&mut balance);
    let balance_after = balance.value;
    account.subtree.insert(token, balance.clone());

    fa(&mut account);

    let account_witness_after = AccountWitness {
        nonce: Some(account.nonce),
        pub_key_hash: Some(account.pub_key_hash),
    };
    tree.insert(account_address, account);
    (
        account_witness_before,
        account_witness_after,
        balance_before,
        balance_after,
    )
}

pub fn apply_fee(
    tree: &mut CircuitAccountTree,
    validator_address: u32,
    token: u32,
    fee: u128,
) -> (Fr, AccountWitness<Bn256>) {
    let fee_fe = Fr::from_str(&fee.to_string()).unwrap();
    let mut validator_leaf = tree
        .remove(validator_address)
        .expect("validator_leaf not empty");
    let validator_account_witness = AccountWitness {
        nonce: Some(validator_leaf.nonce),
        pub_key_hash: Some(validator_leaf.pub_key_hash),
    };

    let mut balance = validator_leaf.subtree.remove(token).unwrap_or_default();
    balance.value.add_assign(&fee_fe);
    validator_leaf.subtree.insert(token, balance);

    tree.insert(validator_address, validator_leaf);

    let root_after_fee = tree.root_hash();
    (root_after_fee, validator_account_witness)
}

pub fn fr_from_bytes(bytes: Vec<u8>) -> Fr {
    let mut fr_repr = <Fr as PrimeField>::Repr::default();
    fr_repr.read_be(&*bytes).unwrap();
    Fr::from_repr(fr_repr).unwrap()
}
