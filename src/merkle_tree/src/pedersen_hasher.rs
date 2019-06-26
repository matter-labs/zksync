// Pedersen hash implementation of the Hasher trait

use ff::PrimeField;
use franklin_crypto::pedersen_hash::{baby_pedersen_hash, Personalization};

use franklin_crypto::alt_babyjubjub::{AltJubjubBn256, JubjubEngine};
use pairing::bn256::Bn256;

use crate::hasher::Hasher;
use models::primitives::BitIteratorLe;

pub struct PedersenHasher<E: JubjubEngine> {
    params: E::Params,
}

impl<E: JubjubEngine> Hasher<E::Fr> for PedersenHasher<E> {
    fn hash_bits<I: IntoIterator<Item = bool>>(&self, input: I) -> E::Fr {
        baby_pedersen_hash::<E, _>(Personalization::NoteCommitment, input, &self.params)
            .into_xy()
            .0
        // print!("Leaf hash = {}\n", hash.clone());
    }

    fn compress(&self, lhs: &E::Fr, rhs: &E::Fr, i: usize) -> E::Fr {
        let lhs = BitIteratorLe::new(lhs.into_repr()).take(E::Fr::NUM_BITS as usize);
        let rhs = BitIteratorLe::new(rhs.into_repr()).take(E::Fr::NUM_BITS as usize);
        let input = lhs.chain(rhs);
        baby_pedersen_hash::<E, _>(Personalization::MerkleTree(i), input, &self.params)
            .into_xy()
            .0
    }
}

pub type BabyPedersenHasher = PedersenHasher<Bn256>;

impl Default for PedersenHasher<Bn256> {
    fn default() -> Self {
        Self {
            params: AltJubjubBn256::new(),
        }
    }
}

#[test]
fn test_pedersen_hash() {
    let hasher = BabyPedersenHasher::default();

    let hash = hasher.hash_bits(vec![false, false, false, true, true, true, true, true]);
    //println!("hash:  {:?}", &hash);

    hasher.compress(&hash, &hash, 0);
    //println!("compr: {:?}", &hash2);

    hasher.compress(&hash, &hash, 1);
    //println!("compr: {:?}", &hash3);

    //assert_eq!(hasher.empty_hash(),
}
