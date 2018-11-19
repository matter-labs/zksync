use pairing::{Engine};
use bellman::{ConstraintSystem, SynthesisError};
use circuit::sha256::{
    sha256
};
use circuit::boolean::{
    Boolean
};

pub fn note_comm<E, CS>(
    cs: CS,
    a_pk: &[Boolean],
    value: &[Boolean],
    rho: &[Boolean],
    r: &[Boolean]
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>
{
    assert_eq!(a_pk.len(), 256);
    assert_eq!(value.len(), 64);
    assert_eq!(rho.len(), 256);
    assert_eq!(r.len(), 256);

    let mut image = vec![];
    image.push(Boolean::constant(true));
    image.push(Boolean::constant(false));
    image.push(Boolean::constant(true));
    image.push(Boolean::constant(true));
    image.push(Boolean::constant(false));
    image.push(Boolean::constant(false));
    image.push(Boolean::constant(false));
    image.push(Boolean::constant(false));
    image.extend(a_pk.iter().cloned());
    image.extend(value.iter().cloned());
    image.extend(rho.iter().cloned());
    image.extend(r.iter().cloned());

    sha256(
        cs,
        &image
    )
}
