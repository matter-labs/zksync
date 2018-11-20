use babyjubjub::{
    JubjubEngine,
    PrimeOrder,
    edwards
};

use ff::{
    PrimeField
};

use blake2_rfc::blake2s::Blake2s;
use constants;

/// Produces a random point in the Jubjub curve.
/// The point is guaranteed to be prime order
/// and not the identity.
pub fn group_hash<E: JubjubEngine>(
    tag: &[u8],
    personalization: &[u8],
    params: &E::Params
) -> Option<edwards::Point<E, PrimeOrder>>
{
    assert_eq!(personalization.len(), 8);

    // Check to see that scalar field is 255 bits
    assert!(E::Fr::NUM_BITS == 254);

    let mut h = Blake2s::with_params(32, &[], &[], personalization);
    h.update(constants::GH_FIRST_BLOCK);
    h.update(tag);
    let h = h.finalize().as_ref().to_vec();
    assert!(h.len() == 32);

    match edwards::Point::<E, _>::read(&h[..], params) {
        Ok(p) => {
            let p = p.mul_by_cofactor(params);

            if p != edwards::Point::zero() {
                Some(p)
            } else {
                None
            }
        },
        Err(_) => None
    }
}
