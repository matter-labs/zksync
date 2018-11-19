use pairing::{Engine};
use bellman::{ConstraintSystem, SynthesisError};
use circuit::sha256::{
    sha256_block_no_padding
};
use circuit::boolean::{
    Boolean
};

fn prf<E, CS>(
    cs: CS,
    a: bool,
    b: bool,
    c: bool,
    d: bool,
    x: &[Boolean],
    y: &[Boolean]
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>
{
    assert_eq!(x.len(), 252);
    assert_eq!(y.len(), 256);

    let mut image = vec![];
    image.push(Boolean::constant(a));
    image.push(Boolean::constant(b));
    image.push(Boolean::constant(c));
    image.push(Boolean::constant(d));
    image.extend(x.iter().cloned());
    image.extend(y.iter().cloned());

    assert_eq!(image.len(), 512);

    sha256_block_no_padding(
        cs,
        &image
    )
}

pub fn prf_a_pk<E, CS>(
    cs: CS,
    a_sk: &[Boolean]
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>
{
    prf(cs, true, true, false, false, a_sk, &(0..256).map(|_| Boolean::constant(false)).collect::<Vec<_>>())
}

pub fn prf_nf<E, CS>(
    cs: CS,
    a_sk: &[Boolean],
    rho: &[Boolean]
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>
{
    prf(cs, true, true, true, false, a_sk, rho)
}

pub fn prf_pk<E, CS>(
    cs: CS,
    a_sk: &[Boolean],
    h_sig: &[Boolean],
    nonce: bool
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>
{
    prf(cs, false, nonce, false, false, a_sk, h_sig)
}

pub fn prf_rho<E, CS>(
    cs: CS,
    phi: &[Boolean],
    h_sig: &[Boolean],
    nonce: bool
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>
{
    prf(cs, false, nonce, true, false, phi, h_sig)
}
