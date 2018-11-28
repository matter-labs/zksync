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

    pub fn empty_hash(&self) -> Point<E, PrimeOrder> {
        pedersen_hash::<E, _>(self.personalization_hash, vec![].into_iter(), &self.params)
    }

    pub fn hash<I>(&self, input: I) -> Point<E, PrimeOrder> where I: IntoIterator<Item=bool> {
        let v: Vec<bool> = input.into_iter().collect();
        println!("input {:?}", &v);
        pedersen_hash::<E, _>(self.personalization_hash, v, &self.params)
    }

    pub fn compress(&self, lhs: &Point<E, PrimeOrder>, rhs: &Point<E, PrimeOrder>) -> Point<E, PrimeOrder> {
        let mut input = Vec::new();
        let (x, y) = lhs.into_xy();
        input.extend(BitIterator::new(x.into_repr()));
        let (x, y) = rhs.into_xy();
        input.extend(BitIterator::new(x.into_repr()));
        pedersen_hash::<E, _>(self.personalization_compress, input, &self.params)
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
    let (x, y) = hash.into_xy();
    println!("\n\nempty: {:?}, {:?}\n\n", x, y);

    let hash2 = hasher.hash(vec![false, false, false, true, true, true, true, true]);
    let (x, y) = hash2.into_xy();
    println!("hash : {:?}, {:?}", x, y);

    let hash3 = hasher.compress(&hash, &hash);
    let (x, y) = hash3.into_xy();
    println!("compr: {:?}, {:?}", x, y);

    //assert_eq!(hasher.empty_hash(),
}
