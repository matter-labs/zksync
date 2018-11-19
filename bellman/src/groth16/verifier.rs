use pairing::{
    Engine,
    CurveProjective,
    CurveAffine
};

use ff::{PrimeField};

use super::{
    Proof,
    VerifyingKey,
    PreparedVerifyingKey
};

use ::{
    SynthesisError
};

pub fn prepare_verifying_key<E: Engine>(
    vk: &VerifyingKey<E>
) -> PreparedVerifyingKey<E>
{
    let mut gamma = vk.gamma_g2;
    gamma.negate();
    let mut delta = vk.delta_g2;
    delta.negate();

    PreparedVerifyingKey {
        alpha_g1_beta_g2: E::pairing(vk.alpha_g1, vk.beta_g2),
        neg_gamma_g2: gamma.prepare(),
        neg_delta_g2: delta.prepare(),
        ic: vk.ic.clone()
    }
}

pub fn verify_proof<'a, E: Engine>(
    pvk: &'a PreparedVerifyingKey<E>,
    proof: &Proof<E>,
    public_inputs: &[E::Fr]
) -> Result<bool, SynthesisError>
{
    if (public_inputs.len() + 1) != pvk.ic.len() {
        return Err(SynthesisError::MalformedVerifyingKey);
    }

    let mut acc = pvk.ic[0].into_projective();

    for (i, b) in public_inputs.iter().zip(pvk.ic.iter().skip(1)) {
        acc.add_assign(&b.mul(i.into_repr()));
    }

    // The original verification equation is:
    // A * B = alpha * beta + inputs * gamma + C * delta
    // ... however, we rearrange it so that it is:
    // A * B - inputs * gamma - C * delta = alpha * beta
    // or equivalently:
    // A * B + inputs * (-gamma) + C * (-delta) = alpha * beta
    // which allows us to do a single final exponentiation.

    Ok(E::final_exponentiation(
        &E::miller_loop([
            (&proof.a.prepare(), &proof.b.prepare()),
            (&acc.into_affine().prepare(), &pvk.neg_gamma_g2),
            (&proof.c.prepare(), &pvk.neg_delta_g2)
        ].into_iter())
    ).unwrap() == pvk.alpha_g1_beta_g2)
}
