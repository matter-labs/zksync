// Plasma account (Merkle tree leaf)

use std::fmt::{self, Debug};
use ff::{Field, PrimeField};
use rand::{Rand, thread_rng};
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::babyjubjub::{JubjubEngine, JubjubBn256, edwards::Point, PrimeOrder};
use sapling_crypto::pedersen_hash::{pedersen_hash, Personalization::NoteCommitment};

use super::sparse_merkle_tree::SparseMerkleTree;
use super::hasher::{Hasher, Factory};
use super::pedersen_hasher::{PedersenHasher, BabyPedersenHasher};

#[derive(Debug, Clone)]
pub struct Leaf<E: JubjubEngine> {
    balance:    E::Fr,
    nonce:      E::Fr,
    pub_x:      E::Fr,
    pub_y:      E::Fr,
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

pub struct LeafHasher<E: JubjubEngine> {
    pedersen: PedersenHasher<E>
}

impl<E: JubjubEngine> Hasher<Leaf<E>, E::Fr> for LeafHasher<E> {

    fn hash(&self, value: &Leaf<E>) -> E::Fr {
        // TODO: implement
        self.pedersen.empty_hash()
    }

    fn compress(&self, lhs: &E::Fr, rhs: &E::Fr, i: usize) -> E::Fr {
        self.pedersen.compress(lhs, rhs, i)
    }

    fn empty_hash(&self) -> E::Fr {
        self.hash(&Leaf::<E>::default())
    }
}

pub type BabyLeaf = Leaf<Bn256>;
pub type BabyLeafHasher = LeafHasher<Bn256>;
impl Factory for BabyLeafHasher {
    fn new() -> Self {
        Self{ pedersen: BabyPedersenHasher::default()}
    }
}

pub type BabyBalanceTree = SparseMerkleTree<BabyLeaf, Fr, BabyLeafHasher>;

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

//impl Hasher<Account<Bn256>> for AccountPedersenHasher {
//
//    type Hash = Point<Bn256, PrimeOrder>;
//
//    fn empty_hash() -> Self::Hash {
//        let params = AltJubjubBn256::new();
//        pedersen_hash::<Bn256, _>(NoteCommitment, vec![].into_iter(), &params)
//    }
//
//    fn hash(value: &Account<Bn256>) -> Self::Hash {
//        let input = vec![]; // decompose `value` into bits
//        let params = AltJubjubBn256::new();
//        pedersen_hash::<Bn256, _>(NoteCommitment, input.into_iter(), &params)
//
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
//    }
//
//    fn compress(lhs: &Self::Hash, rhs: &Self::Hash) -> Self::Hash {
//        let params = AltJubjubBn256::new();
//        let input = vec![]; // to_bits(lhs) || to_bits(rhs)
//        pedersen_hash::<Bn256, _>(NoteCommitment, input.into_iter(), &params)
//    }
//}
