// Pedersen hash implementation of the Hasher trait

use ff::{Field, PrimeField, BitIterator};
use rand::{Rand, thread_rng};
use sapling_crypto::pedersen_hash::{baby_pedersen_hash, Personalization};

use pairing::bn256::Bn256;
use sapling_crypto::alt_babyjubjub::{JubjubEngine, AltJubjubBn256, edwards::Point, PrimeOrder};

use super::super::primitives::GetBitsFixed;
use super::hasher::Hasher;

pub struct PedersenHasher<E: JubjubEngine> {
    params: E::Params,
}

impl<E: JubjubEngine> Hasher<E::Fr> for PedersenHasher<E> {

    fn hash_bits<I: IntoIterator<Item=bool>>(&self, input: I) -> E::Fr {
        let hash = baby_pedersen_hash::<E, _>(Personalization::NoteCommitment, input, &self.params).into_xy().0;
        // print!("Leaf hash = {}\n", hash.clone());

        hash
    }

    fn compress(&self, lhs: &E::Fr, rhs: &E::Fr, i: usize) -> E::Fr {
        let mut input: Vec<bool> = Vec::with_capacity(2 * E::Fr::NUM_BITS as usize);
        input.extend(lhs.get_bits_le_fixed(E::Fr::NUM_BITS as usize));
        input.extend(rhs.get_bits_le_fixed(E::Fr::NUM_BITS as usize));
        baby_pedersen_hash::<E, _>(Personalization::MerkleTree(i), input, &self.params).into_xy().0
    }

}

pub type BabyPedersenHasher = PedersenHasher<Bn256>;

impl Default for PedersenHasher<Bn256> {
    fn default() -> Self {
        Self{
            params: AltJubjubBn256::new(),
        }
    }
}

#[test]
fn test_pedersen_hash() {
    let hasher = BabyPedersenHasher::default();

    let hash = hasher.hash_bits(vec![false, false, false, true, true, true, true, true]);
    //println!("hash:  {:?}", &hash);

    let hash2 = hasher.compress(&hash, &hash, 0);
    //println!("compr: {:?}", &hash2);

    let hash3 = hasher.compress(&hash, &hash, 1);
    //println!("compr: {:?}", &hash3);

    //assert_eq!(hasher.empty_hash(),
}
