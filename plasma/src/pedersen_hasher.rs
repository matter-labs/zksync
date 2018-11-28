// Pedersen hash implementation of the Hasher trait

use ff::{Field, PrimeField};
use rand::{Rand, thread_rng};
use sapling_crypto::pedersen_hash::{pedersen_hash, Personalization::NoteCommitment};

use pairing::bn256::Bn256;
use sapling_crypto::alt_babyjubjub::{JubjubEngine, AltJubjubBn256, edwards::Point, PrimeOrder};

pub struct PedersenHasher<E: JubjubEngine> {
    params: E::Params
}

impl<E: JubjubEngine> PedersenHasher<E> {

    pub fn empty_hash(&self) -> Point<E, PrimeOrder> {
        pedersen_hash::<E, _>(NoteCommitment, vec![].into_iter(), &self.params)
    }

    pub fn hash(&self /*, value: &Account<Bn256>*/) -> Point<E, PrimeOrder> {
        //let input = vec![]; // TODO: decompose `value` into bits
        pedersen_hash::<E, _>(NoteCommitment, vec![].into_iter(), &self.params)
    }

    pub fn compress(&self/*, lhs: &Self::Hash, rhs: &Self::Hash*/) -> Point<E, PrimeOrder> {
        //let input = vec![]; // TODO: to_bits(lhs) || to_bits(rhs)
        pedersen_hash::<E, _>(NoteCommitment, vec![].into_iter(), &self.params)
    }
}

pub type BabyPedersenHasher = PedersenHasher<Bn256>;

impl Default for PedersenHasher<Bn256> {
    fn default() -> Self {
        Self{params: AltJubjubBn256::new()}
    }
}

#[test]
fn test_pedersen_hasher() {

}
