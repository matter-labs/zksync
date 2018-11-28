// Pedersen hash implementation of the Hasher trait

use ff::{Field, PrimeField, BitIterator};
use rand::{Rand, thread_rng};
use sapling_crypto::baby_pedersen_hash::{pedersen_hash, Personalization};

use pairing::bn256::Bn256;
use sapling_crypto::babyjubjub::{JubjubEngine, JubjubBn256, edwards::Point, PrimeOrder};

pub struct PedersenHasher<E: JubjubEngine> {
    params: E::Params,
    personalization_hash: Personalization,
    personalization_compress: Personalization,
}

impl<E: JubjubEngine> PedersenHasher<E> {

    pub fn empty_hash(&self) -> E::Fr {
       self.hash(vec![])
    }

    pub fn hash<I>(&self, input: I) -> E::Fr where I: IntoIterator<Item=bool> {
        let v: Vec<bool> = input.into_iter().collect();
        pedersen_hash::<E, _>(self.personalization_hash, v, &self.params).into_xy().0
    }

    pub fn compress(&self, lhs: &E::Fr, rhs: &E::Fr) -> E::Fr {
        let mut input = Vec::new();
        input.extend(BitIterator::new(lhs.into_repr()));
        input.extend(BitIterator::new(rhs.into_repr()));
        pedersen_hash::<E, _>(self.personalization_compress, input, &self.params).into_xy().0
    }
}

pub type BabyPedersenHasher = PedersenHasher<Bn256>;

impl Default for PedersenHasher<Bn256> {
    fn default() -> Self {
        Self{
            params: JubjubBn256::new(),

            // NB: this is hardcoded based on the baby plasma circuit spec
            personalization_hash: Personalization::NoteCommitment,
            personalization_compress: Personalization::MerkleTree(24),
        }
    }
}

#[test]
fn test_pedersen_hash() {
    let hasher = BabyPedersenHasher::default();

    let hash = hasher.empty_hash();
    println!("empty: {:?}", &hash);

    let hash = hasher.hash(vec![false, false, false, true, true, true, true, true]);
    println!("hash:  {:?}", &hash);

    let hash = hasher.compress(&hash, &hash);
    println!("compr: {:?}", &hash);

    //assert_eq!(hasher.empty_hash(),
}
