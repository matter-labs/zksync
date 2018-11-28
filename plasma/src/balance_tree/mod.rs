// Plasma account (Merkle tree leaf)

use std::fmt::{self, Debug};
use ff::{Field, PrimeField};
use rand::{Rand, thread_rng};
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::babyjubjub::{JubjubEngine, JubjubBn256, edwards::Point, PrimeOrder};

use super::smt::SparseMerkleTree;
use super::smt::hasher::IntoBits;
use super::smt::pedersen_hasher::BabyPedersenHasher;

use sapling_crypto::circuit::matter::plasma_constants;

#[derive(Debug, Clone)]
pub struct Leaf<E: JubjubEngine> {
    balance:    E::Fr,
    nonce:      E::Fr,
    pub_x:      E::Fr,
    pub_y:      E::Fr,
}

impl<E: JubjubEngine> IntoBits for Leaf<E> {
    fn into_bits(&self) -> Vec<bool> {
        // TODO: expand
        vec![]
    }
}

impl<E: JubjubEngine> Default for Leaf<E> {
    fn default() -> Self{
        Self {
            balance:    E::Fr::zero(),
            nonce:      E::Fr::zero(),
            pub_x:      E::Fr::zero(),
            pub_y:      E::Fr::zero(),
        }
    }
}

pub type BabyLeaf = Leaf<Bn256>;

pub type BabyBalanceTree = SparseMerkleTree<BabyLeaf, Fr, BabyPedersenHasher>;

#[test]
fn test_account_merkle_tree() {
    let mut tree = BabyBalanceTree::new(3);
    let leaf = BabyLeaf {
        balance:    Fr::zero(),
        nonce:      Fr::one(),
        pub_x:      Fr::one(),
        pub_y:      Fr::one(),
    };
    tree.insert(0, leaf);
    let root = tree.root_hash();
    println!("root: {:?}", root);
}


////        let mut leaf_content = vec![];
////
////        let mut value_content_from = boolean::field_into_boolean_vec_le(
////            cs.namespace(|| "from leaf amount bits"),
////            tx_witness.balance_from
////        ).unwrap();
////
////        value_content_from.truncate(*plasma_constants::BALANCE_BIT_WIDTH);
////        leaf_content.extend(value_content_from.clone());
////
////        let mut nonce_content_from = boolean::field_into_boolean_vec_le(
////            cs.namespace(|| "from leaf nonce bits"),
////            tx_witness.nonce_from
////        ).unwrap();
////
////        nonce_content_from.truncate(*plasma_constants::NONCE_BIT_WIDTH);
////        leaf_content.extend(nonce_content_from.clone());
////
////        let mut pub_x_content_from = boolean::field_into_boolean_vec_le(
////            cs.namespace(|| "from leaf pub_x bits"),
////            tx_witness.pub_x_from
////        ).unwrap();
////
////        for _ in 0..(*plasma_constants::FR_BIT_WIDTH - pub_x_content_from.len())
////            {
////                pub_x_content_from.push(boolean::Boolean::Constant(false));
////            }
////        leaf_content.extend(pub_x_content_from.clone());
////
////        let mut pub_y_content_from = boolean::field_into_boolean_vec_le(
////            cs.namespace(|| "from leaf pub_y bits"),
////            tx_witness.pub_y_from
////        ).unwrap();
////
////        for _ in 0..(*plasma_constants::FR_BIT_WIDTH - pub_y_content_from.len())
////            {
////                pub_y_content_from.push(boolean::Boolean::Constant(false));
////            }
////        leaf_content.extend(pub_y_content_from.clone());
////
////        assert_eq!(leaf_content.len(), *plasma_constants::BALANCE_BIT_WIDTH
////            + *plasma_constants::NONCE_BIT_WIDTH
////            + 2 * (*plasma_constants::FR_BIT_WIDTH)
////        );
////
////        // Compute the hash of the from leaf
////        let mut from_leaf_hash = pedersen_hash::pedersen_hash(
////            cs.namespace(|| "from leaf content hash"),
////            pedersen_hash::Personalization::NoteCommitment,
////            &leaf_content,
////            params
////        )?;
