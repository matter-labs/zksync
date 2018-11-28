// Pedersen hash implementation of the Hasher trait

use ff::{Field, PrimeField, BitIterator};
use rand::{Rand, thread_rng};
use sapling_crypto::baby_pedersen_hash::{pedersen_hash, Personalization};

use pairing::bn256::Bn256;
use sapling_crypto::babyjubjub::{JubjubEngine, JubjubBn256, edwards::Point, PrimeOrder};

pub struct PedersenHasher<E: JubjubEngine> {
    params: E::Params,
}

impl<E: JubjubEngine> PedersenHasher<E> {

    pub fn empty_hash(&self) -> E::Fr {
       self.hash(vec![])
    }

    pub fn hash<I>(&self, input: I) -> E::Fr where I: IntoIterator<Item=bool> {
        let v: Vec<bool> = input.into_iter().collect();
        pedersen_hash::<E, _>(Personalization::NoteCommitment, v, &self.params).into_xy().0
    }

    pub fn compress(&self, lhs: &E::Fr, rhs: &E::Fr, i: usize) -> E::Fr {
        let mut input = Vec::new();
        input.extend(BitIterator::new(lhs.into_repr()));
        input.extend(BitIterator::new(rhs.into_repr()));
        pedersen_hash::<E, _>(Personalization::MerkleTree(i), input, &self.params).into_xy().0
    }
}

pub type BabyPedersenHasher = PedersenHasher<Bn256>;

impl Default for PedersenHasher<Bn256> {
    fn default() -> Self {
        Self{
            params: JubjubBn256::new(),
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

    let hash2 = hasher.compress(&hash, &hash, 0);
    println!("compr: {:?}", &hash2);

    let hash3 = hasher.compress(&hash, &hash, 1);
    println!("compr: {:?}", &hash3);

    //assert_eq!(hasher.empty_hash(),
}
