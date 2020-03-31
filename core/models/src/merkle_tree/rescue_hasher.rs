// Pedersen hash implementation of the Hasher trait

use crate::franklin_crypto::rescue::{RescueEngine, rescue_hash};
use crate::franklin_crypto::bellman::pairing::bn256::Bn256;
use crate::franklin_crypto::circuit::multipack;

use super::hasher::Hasher;

#[derive(Clone)]
pub struct RescueHasher<E: RescueEngine> {
    params: &'static E::Params,
}

impl<E: RescueEngine> Hasher<E::Fr> for RescueHasher<E> {
    fn hash_bits<I: IntoIterator<Item = bool>>(&self, input: I) -> E::Fr {
        let bits: Vec<bool> = input.into_iter().collect();
        let packed = multipack::compute_multipacking::<E>(&bits);
        let sponge_output = rescue_hash::<E>(
            self.params,
            &packed
        );

        assert_eq!(sponge_output.len(), 1);

        // println!("Hashed bits into {}", sponge_output[0]);

        sponge_output[0]
    }

    fn hash_elements<I: IntoIterator<Item = E::Fr>>(&self, elements: I) -> E::Fr {
        let packed: Vec<_> = elements.into_iter().collect();
        let sponge_output = rescue_hash::<E>(
            self.params,
            &packed
        );

        assert_eq!(sponge_output.len(), 1);

        // println!("Hashed elements into {}", sponge_output[0]);

        sponge_output[0]
    }

    fn compress(&self, lhs: &E::Fr, rhs: &E::Fr, _i: usize) -> E::Fr {
        let sponge_output = rescue_hash::<E>(
            self.params,
            &[*lhs, *rhs]
        );

        assert_eq!(sponge_output.len(), 1);

        // println!("Hashed node on level {} into {}", _i, sponge_output[0]);

        sponge_output[0]
    }
}

pub type BabyRescueHasher = RescueHasher<Bn256>;

impl Default for RescueHasher<Bn256> {
    fn default() -> Self {
        Self {
            params: &crate::params::RESCUE_PARAMS,
        }
    }
}

#[test]
fn test_pedersen_hash() {
    let hasher = BabyRescueHasher::default();

    let hash = hasher.hash_bits(vec![false, false, false, true, true, true, true, true]);
    //debug!("hash:  {:?}", &hash);

    hasher.compress(&hash, &hash, 0);
    //debug!("compr: {:?}", &hash2);

    hasher.compress(&hash, &hash, 1);
    //debug!("compr: {:?}", &hash3);

    //assert_eq!(hasher.empty_hash(),
}
