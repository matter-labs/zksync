use pairing::{Engine};
use bellman::{ConstraintSystem, SynthesisError};
use circuit::boolean::{Boolean};

use super::*;
use super::prfs::*;
use super::commitment::note_comm;

pub struct OutputNote {
    pub cm: Vec<Boolean>
}

impl OutputNote {
    pub fn compute<'a, E, CS>(
        mut cs: CS,
        a_pk: Option<PayingKey>,
        value: &NoteValue,
        r: Option<CommitmentRandomness>,
        phi: &[Boolean],
        h_sig: &[Boolean],
        nonce: bool
    ) -> Result<Self, SynthesisError>
        where E: Engine, CS: ConstraintSystem<E>,
    {
        let rho = prf_rho(
            cs.namespace(|| "rho"),
            phi,
            h_sig,
            nonce
        )?;

        let a_pk = witness_u256(
            cs.namespace(|| "a_pk"),
            a_pk.as_ref().map(|a_pk| &a_pk.0[..])
        )?;

        let r = witness_u256(
            cs.namespace(|| "r"),
            r.as_ref().map(|r| &r.0[..])
        )?;

        let cm = note_comm(
            cs.namespace(|| "cm computation"),
            &a_pk,
            &value.bits_le(),
            &rho,
            &r
        )?;

        Ok(OutputNote {
            cm: cm
        })
    }
}
