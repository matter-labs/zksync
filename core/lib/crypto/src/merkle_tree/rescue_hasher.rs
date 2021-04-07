// Pedersen hash implementation of the Hasher trait

use crate::franklin_crypto::bellman::pairing::bn256::Bn256;
use crate::franklin_crypto::circuit::multipack;
use crate::franklin_crypto::rescue::{rescue_hash, RescueEngine};

use super::hasher::Hasher;
use core::fmt;

/// Default hasher for the zkSync state hash calculation.
pub struct RescueHasher<E: RescueEngine> {
    params: &'static E::Params,
}

impl<E: RescueEngine> fmt::Debug for RescueHasher<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RescueHasher").finish()
    }
}

// We have to implement `Clone` manually, since deriving it will depend on
// the `Clone` implementation of `E::Params` (and will `.clone()` will not work
// if `E::Params` are not `Clone`), which is redundant: we only hold a reference
// and can just copy it.
impl<E: RescueEngine> Clone for RescueHasher<E> {
    fn clone(&self) -> Self {
        Self {
            params: self.params,
        }
    }
}

impl<E: RescueEngine> Hasher<E::Fr> for RescueHasher<E> {
    fn hash_bits<I: IntoIterator<Item = bool>>(&self, input: I) -> E::Fr {
        let bits: Vec<bool> = input.into_iter().collect();
        let packed = multipack::compute_multipacking::<E>(&bits);
        let sponge_output = rescue_hash::<E>(self.params, &packed);

        assert_eq!(sponge_output.len(), 1);
        sponge_output[0]
    }

    fn hash_elements<I: IntoIterator<Item = E::Fr>>(&self, elements: I) -> E::Fr {
        let packed: Vec<_> = elements.into_iter().collect();
        let sponge_output = rescue_hash::<E>(self.params, &packed);

        assert_eq!(sponge_output.len(), 1);
        sponge_output[0]
    }

    fn compress(&self, lhs: &E::Fr, rhs: &E::Fr, _i: usize) -> E::Fr {
        let sponge_output = rescue_hash::<E>(self.params, &[*lhs, *rhs]);

        assert_eq!(sponge_output.len(), 1);
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
fn test_resue_hash() {
    let hasher = BabyRescueHasher::default();

    let hash = hasher.hash_bits(vec![false, false, false, true, true, true, true, true]);
    hasher.compress(&hash, &hash, 0);
    hasher.compress(&hash, &hash, 1);
}
