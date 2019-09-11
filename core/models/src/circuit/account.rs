use crate::params;

use ff::{Field, PrimeField, PrimeFieldRepr};
use franklin_crypto::alt_babyjubjub::JubjubEngine;

use crate::merkle_tree::{PedersenHasher, SparseMerkleTree, hasher::Hasher};
use franklin_crypto::eddsa::PublicKey;
use crate::primitives::{GetBits, GetBitsFixed};
use pairing::bn256::{Bn256, Fr};
pub type CircuitAccountTree = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type CircuitBalanceTree = SparseMerkleTree<Balance<Bn256>, Fr, PedersenHasher<Bn256>>;
pub struct CircuitAccount<E: JubjubEngine> {
    pub subtree: SparseMerkleTree<Balance<E>, E::Fr, PedersenHasher<E>>,
    pub nonce: E::Fr,
    pub pub_key_hash: E::Fr,
}

impl<E: JubjubEngine> GetBits for CircuitAccount<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();

        leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH)); //32
        leaf_content.extend(
            self.pub_key_hash
                .get_bits_le_fixed(params::NEW_PUBKEY_HASH_WIDTH), //160
        );

        let mut root_hash_bits = self
            .subtree
            .root_hash()
            .get_bits_le_fixed(params::FR_BIT_WIDTH);
        root_hash_bits.resize(params::FR_BIT_WIDTH_PADDED, false); //256

        leaf_content.extend(root_hash_bits);

        leaf_content
    }
}
impl<E: JubjubEngine> CircuitAccount<E> {
    //we temporary pass it as repr. TODO: return Fr, when we could provide proper trait bound
    pub fn empty_balances_root_hash() -> Vec<u8> {
        let balances_smt = CircuitBalanceTree::new(params::BALANCE_TREE_DEPTH as u32);
        let mut tmp = [0u8; 32];
        balances_smt
            .root_hash()
            .into_repr()
            .write_be(&mut tmp[..])
            .unwrap();
        tmp.to_vec()
    }
}

impl std::default::Default for CircuitAccount<Bn256> {
    //default should be changed: since subtree_root_hash is not zero for all zero balances and subaccounts
    fn default() -> Self {
        Self {
            nonce: Fr::zero(),
            pub_key_hash: Fr::zero(),
            // pub_x: Fr::zero(),
            // pub_y: Fr::zero(),
            subtree: SparseMerkleTree::new(params::BALANCE_TREE_DEPTH as u32),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Balance<E: JubjubEngine> {
    pub value: E::Fr,
}

impl<E: JubjubEngine> GetBits for Balance<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(self.value.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));

        leaf_content
    }
}

impl<E: JubjubEngine> std::default::Default for Balance<E> {
    //default should be changed: since subtree_root_hash is not zero for all zero balances and subaccounts
    fn default() -> Self {
        Self {
            value: E::Fr::zero(),
        }
    }
}

fn pub_key_hash_bits<E: JubjubEngine, H: Hasher<E::Fr>>(
    pub_key: &PublicKey<E>,
    hasher: &H
) -> Vec<bool> {
    let (pub_x, pub_y) = pub_key.0.into_xy();
    let mut pub_key_bits = vec![];
    append_le_fixed_width(
        &mut pub_key_bits,
        &pub_x,
        params::FR_BIT_WIDTH_PADDED,
    );
    append_le_fixed_width(
        &mut pub_key_bits,
        &pub_y,
        params::FR_BIT_WIDTH_PADDED,
    );
    let pub_key_hash = hasher.hash_bits(pub_key_bits);
    let mut pub_key_hash_bits = vec![];
    append_le_fixed_width(
        &mut pub_key_hash_bits,
        &pub_key_hash,
        params::NEW_PUBKEY_HASH_WIDTH,
    );
    pub_key_hash_bits
}

pub fn pub_key_hash_fe<E: JubjubEngine, H: Hasher<E::Fr>>(
    pub_key: &PublicKey<E>,
    hasher: &H,
) -> E::Fr {
    let pk_hash_bits = pub_key_hash_bits(pub_key, hasher);
    le_bit_vector_into_field_element(&pub_key_hash_bits)
}

pub fn pub_key_hash_bytes<E: JubjubEngine, H: Hasher<E::Fr>>(
    pub_key: &PublicKey<E>,
    hasher: &H,
) -> Vec<u8> {
    let pk_hash_bits = pub_key_hash_bits(pub_key, hasher);
    le_bit_vector_into_bytes(&pk_hash_bits)
}

pub fn le_bit_vector_into_bytes(bits: &[bool]) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![];

    let byte_chunks = bits.chunks(8);

    for byte_chunk in byte_chunks {
        let mut byte = 0u8;
        // pack just in order
        for (i, bit) in byte_chunk.iter().enumerate() {
            if *bit {
                byte |= 1 << i;
            }
        }
        bytes.push(byte);
    }

    bytes
}

pub fn le_bit_vector_into_field_element<P: PrimeField>(bits: &[bool]) -> P {
    // double and add
    let mut fe = P::zero();
    let mut base = P::one();

    for bit in bits {
        if *bit {
            fe.add_assign(&base);
        }
        base.double();
    }

    fe
}

pub fn be_bit_vector_into_bytes(bits: &[bool]) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![];

    let byte_chunks = bits.chunks(8);

    for byte_chunk in byte_chunks {
        let mut byte = 0u8;
        // pack just in order
        for (i, bit) in byte_chunk.iter().enumerate() {
            if *bit {
                byte |= 1 << (7 - i);
            }
        }
        bytes.push(byte);
    }

    bytes
}

pub fn append_le_fixed_width<P: PrimeField>(content: &mut Vec<bool>, x: &P, width: usize) {
    let mut token_bits: Vec<bool> = BitIterator::new(x.into_repr()).collect();
    token_bits.reverse();
    // token_bits.truncate(width);
    token_bits.resize(width, false);
    content.extend(token_bits.clone());
}

pub fn append_be_fixed_width<P: PrimeField>(content: &mut Vec<bool>, x: &P, width: usize) {
    let mut token_bits: Vec<bool> = BitIterator::new(x.into_repr()).collect();
    token_bits.reverse();
    token_bits.resize(width, false);
    token_bits.reverse();
    content.extend(token_bits.clone());
}


pub fn encode_fs_into_fr<E: JubjubEngine>(input: E::Fs) -> E::Fr {
    let mut fs_le_bits: Vec<bool> = BitIterator::new(input.into_repr()).collect();
    fs_le_bits.reverse();

    le_bit_vector_into_field_element::<E::Fr>(&fs_le_bits)
}

pub fn encode_fr_into_fs<E: JubjubEngine>(input: E::Fr) -> E::Fs {
    let mut fr_le_bits: Vec<bool> = BitIterator::new(input.into_repr()).collect();
    fr_le_bits.reverse();

    le_bit_vector_into_field_element::<E::Fs>(&fr_le_bits)
}
