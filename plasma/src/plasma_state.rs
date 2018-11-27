// Plasma sidechain state implementation

use ff::{Field, PrimeField};
//use rand::{Rand, thread_rng};
//use pairing::{Engine, bn256::Bn256};
//use sapling_crypto::{
//    //jubjub::JubjubEngine,
//    alt_babyjubjub::{
//        JubjubEngine,
//        AltJubjubBn256,
//        edwards::Point,
//        PrimeOrder,
//    },
//    pedersen_hash::{
//        baby_pedersen_hash,
//        Personalization::NoteCommitment
//    }
//};

use rand::{Rand, thread_rng};
use pairing::bn256::Bn256;
use sapling_crypto::alt_babyjubjub::{JubjubEngine, AltJubjubBn256, edwards::Point, PrimeOrder};
use sapling_crypto::pedersen_hash::{pedersen_hash, Personalization::NoteCommitment};

use super::sparse_merkle_tree::{Hasher, SparseMerkleTree};

#[derive(Debug, Clone)]
pub struct Account<E: JubjubEngine> {
    balance:    E::Fs,
    nonce:      E::Fs,
    //pubkey:     EdwardsPoint<E>,
}

struct AccountPedersenHasher {
}

//static ALT_JUBJUB_PARAMS: Option<&AltJubjubBn256> = Some(&AltJubjubBn256::new());
//static ALT_JUBJUB_PARAMS: &AltJubjubBn256 = &AltJubjubBn256::new();

impl AccountPedersenHasher {

}

impl Hasher<Account<Bn256>> for AccountPedersenHasher {

    type Hash = Point<Bn256, PrimeOrder>;

    fn empty_hash() -> Self::Hash {
        let params = AltJubjubBn256::new();
        pedersen_hash::<Bn256, _>(NoteCommitment, vec![].into_iter(), &params)
    }

    fn hash(value: &Account<Bn256>) -> Self::Hash {
        let input = vec![]; // decompose `value` into bits
        let params = AltJubjubBn256::new();
        pedersen_hash::<Bn256, _>(NoteCommitment, input.into_iter(), &params)
    }

    fn compress(lhs: &Self::Hash, rhs: &Self::Hash) -> Self::Hash {
        let params = AltJubjubBn256::new();
        let input = vec![]; // to_bits(lhs) || to_bits(rhs)
        pedersen_hash::<Bn256, _>(NoteCommitment, input.into_iter(), &params)
    }
}

//impl<E: JubjubEngine> Hasher<Account<E>> for AccountPedersenHasher {
//
//    type Hash = Point<E, PrimeOrder>;
//
//    fn empty_hash() -> Self::Hash {
//        Self::Hash::zero()
//    }
//
//    fn hash(value: &Account<E>) -> Self::Hash {
//        //let params = JubjubParams<E>::new();
//        //let params: &<E as sapling_crypto::jubjub::JubjubEngine>::Params = &AltJubjubBn256::new();
//        let rng = &mut thread_rng();
//        let input = (0..510).map(|_| bool::rand(rng)).collect::<Vec<_>>();
//        pedersen_hash::<E, _>(NoteCommitment, input.into_iter(), &Self::params())
//    }
//
//    fn compress(lhs: Self::Hash, rhs: Self::Hash) -> Self::Hash {
//        Self::Hash::zero()
//    }
//}

#[test]
fn test_account_merkle_tree() {

}

//pub type AccountId = u32;
//pub type Value = u32;
//
//pub struct Tx {
//    from:   AccountId,
//    to:     AccountId,
//    value:  Value,
//    fee:    Value,
//}
//
//pub type ValuePacked = u32;
//
//// 112 bit
//pub struct TxPacked {
//    from:   AccountId,
//    to:     AccountId,
//    value:  ValuePacked,
//    fee:    ValuePacked,
//}
//
//pub type TxPubInput = TxPacked;
//
//#[derive(Debug, Clone)]
//pub struct PlasmaState<E: JubjubEngine> {
//    tree_height: usize,
//    accounts: Vec<Account<E>>,
//    // hashes
//    merkle_root: E::Fs,
//}
//
//impl<E: JubjubEngine> PlasmaState<E> {
//
//    fn new(tree_height: usize) -> Self {
//
//        assert!(tree_height > 0 && tree_height < 32);
//
//        let accounts = vec![Account::<_>{
//            balance: E::Fs::zero(),
//            nonce:   E::Fs::zero(),
//            //pubkey:  EdwardsPoint<E>::
//        }; Self::capacity(tree_height)];
//
//        let merkle_root = E::Fs::zero();
//
//        Self{tree_height, accounts, merkle_root}.update_merkle_root()
//    }
//
//    fn capacity(tree_height: usize) -> usize {
//        2 << tree_height
//    }
//
//    // State transition function: S_new <= S_old.apply(tx)
//    fn apply(&mut self, tx: &Tx) -> &Self {
//        self
//    }
//
//    fn update_merkle_root(&mut self) -> &Self {
//        self
//    }
//}

//#[derive(Clone)]
//struct Leaf<E: JubjubEngine, TreeHeight: usize> {
//
//    // state: 4 field elements
//    balance:    E::Fs,
//    nonce:      E::Fs,
//    pubkey:     EdwardsPoint<E>,
//
//    // Merkle auth path
//    merkle_path: [E::Fs; TreeHeight],
//}
//
//struct TxWitness<E: JubjubEngine> {
//    pub_input:              TxPubInput,
//    from_leaf:              Leaf<E>,
//    to_leaf:                Leaf<E>,
//    sig:                    EdwardsPoint<E>,
//    merkle_root_updated:    E::Fs,
//}
//
////use sapling_crypto::circuit::sha256::{sha256};
////use sapling_crypto::circuit::num::{AllocatedNum};
////use sapling_crypto::circuit::multipack::{pack_into_inputs};
////use num_bigint::BigUint;
//
//struct PlasmaUpdateCircuit<E: Engine> {
//    final_hash: Option<E::Fr>,
//}
//
////    let params = JubJubEngine::new();
////    let rng = &mut thread_rng();
////    let bits = (0..510).map(|_| bool::rand(rng)).collect::<Vec<_>>();
////    let personalization = Personalization::MerkleTree(31);
////    pedersen_hash::<Bn256, _>(personalization, bits.clone(), &params);
//
//impl<E: Engine> Circuit<E> for PlasmaUpdateCircuit<E> {
//    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
//
//        Ok(())
//    }
//}
