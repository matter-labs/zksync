use pairing::{Engine};
use bellman::{ConstraintSystem, SynthesisError};
use circuit::sha256::{
    sha256_block_no_padding
};
use circuit::boolean::{
    AllocatedBit,
    Boolean
};

use super::*;
use super::prfs::*;
use super::commitment::note_comm;

pub struct InputNote {
    pub nf: Vec<Boolean>,
    pub mac: Vec<Boolean>,
}

impl InputNote {
    pub fn compute<E, CS>(
        mut cs: CS,
        a_sk: Option<SpendingKey>,
        rho: Option<UniqueRandomness>,
        r: Option<CommitmentRandomness>,
        value: &NoteValue,
        h_sig: &[Boolean],
        nonce: bool,
        auth_path: [Option<([u8; 32], bool)>; TREE_DEPTH],
        rt: &[Boolean]
    ) -> Result<InputNote, SynthesisError>
        where E: Engine, CS: ConstraintSystem<E>
    {
        let a_sk = witness_u252(
            cs.namespace(|| "a_sk"),
            a_sk.as_ref().map(|a_sk| &a_sk.0[..])
        )?;

        let rho = witness_u256(
            cs.namespace(|| "rho"),
            rho.as_ref().map(|rho| &rho.0[..])
        )?;

        let r = witness_u256(
            cs.namespace(|| "r"),
            r.as_ref().map(|r| &r.0[..])
        )?;

        let a_pk = prf_a_pk(
            cs.namespace(|| "a_pk computation"),
            &a_sk
        )?;

        let nf = prf_nf(
            cs.namespace(|| "nf computation"),
            &a_sk,
            &rho
        )?;

        let mac = prf_pk(
            cs.namespace(|| "mac computation"),
            &a_sk,
            h_sig,
            nonce
        )?;

        let cm = note_comm(
            cs.namespace(|| "cm computation"),
            &a_pk,
            &value.bits_le(),
            &rho,
            &r
        )?;

        // Witness into the merkle tree
        let mut cur = cm.clone();

        for (i, layer) in auth_path.into_iter().enumerate() {
            let cs = &mut cs.namespace(|| format!("layer {}", i));

            let cur_is_right = AllocatedBit::alloc(
                cs.namespace(|| "cur is right"),
                layer.as_ref().map(|&(_, p)| p)
            )?;

            let lhs = cur;
            let rhs = witness_u256(
                cs.namespace(|| "sibling"),
                layer.as_ref().map(|&(ref sibling, _)| &sibling[..])
            )?;

            // Conditionally swap if cur is right
            let preimage = conditionally_swap_u256(
                cs.namespace(|| "conditional swap"),
                &lhs[..],
                &rhs[..],
                &cur_is_right
            )?;

            cur = sha256_block_no_padding(
                cs.namespace(|| "hash of this layer"),
                &preimage
            )?;
        }

        // enforce must be true if the value is nonzero
        let enforce = AllocatedBit::alloc(
            cs.namespace(|| "enforce"),
            value.get_value().map(|n| n != 0)
        )?;

        // value * (1 - enforce) = 0
        // If `value` is zero, `enforce` _can_ be zero.
        // If `value` is nonzero, `enforce` _must_ be one.
        cs.enforce(
            || "enforce validity",
            |_| value.lc(),
            |lc| lc + CS::one() - enforce.get_variable(),
            |lc| lc
        );

        assert_eq!(cur.len(), rt.len());

        // Check that the anchor (exposed as a public input)
        // is equal to the merkle tree root that we calculated
        // for this note
        for (i, (cur, rt)) in cur.into_iter().zip(rt.iter()).enumerate() {
            // (cur - rt) * enforce = 0
            // if enforce is zero, cur and rt can be different
            // if enforce is one, they must be equal
            cs.enforce(
                || format!("conditionally enforce correct root for bit {}", i),
                |_| cur.lc(CS::one(), E::Fr::one()) - &rt.lc(CS::one(), E::Fr::one()),
                |lc| lc + enforce.get_variable(),
                |lc| lc
            );
        }

        Ok(InputNote {
            mac: mac,
            nf: nf
        })
    }
}

/// Swaps two 256-bit blobs conditionally, returning the
/// 512-bit concatenation.
pub fn conditionally_swap_u256<E, CS>(
    mut cs: CS,
    lhs: &[Boolean],
    rhs: &[Boolean],
    condition: &AllocatedBit
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>,
{
    assert_eq!(lhs.len(), 256);
    assert_eq!(rhs.len(), 256);

    let mut new_lhs = vec![];
    let mut new_rhs = vec![];

    for (i, (lhs, rhs)) in lhs.iter().zip(rhs.iter()).enumerate() {
        let cs = &mut cs.namespace(|| format!("bit {}", i));

        let x = Boolean::from(AllocatedBit::alloc(
            cs.namespace(|| "x"),
            condition.get_value().and_then(|v| {
                if v {
                    rhs.get_value()
                } else {
                    lhs.get_value()
                }
            })
        )?);

        // x = (1-condition)lhs + (condition)rhs
        // x = lhs - lhs(condition) + rhs(condition)
        // x - lhs = condition (rhs - lhs)
        // if condition is zero, we don't swap, so
        //   x - lhs = 0
        //   x = lhs
        // if condition is one, we do swap, so
        //   x - lhs = rhs - lhs
        //   x = rhs
        cs.enforce(
            || "conditional swap for x",
            |lc| lc + &rhs.lc(CS::one(), E::Fr::one())
                    - &lhs.lc(CS::one(), E::Fr::one()),
            |lc| lc + condition.get_variable(),
            |lc| lc + &x.lc(CS::one(), E::Fr::one())
                    - &lhs.lc(CS::one(), E::Fr::one())
        );

        let y = Boolean::from(AllocatedBit::alloc(
            cs.namespace(|| "y"),
            condition.get_value().and_then(|v| {
                if v {
                    lhs.get_value()
                } else {
                    rhs.get_value()
                }
            })
        )?);

        // y = (1-condition)rhs + (condition)lhs
        // y - rhs = condition (lhs - rhs)
        cs.enforce(
            || "conditional swap for y",
            |lc| lc + &lhs.lc(CS::one(), E::Fr::one())
                    - &rhs.lc(CS::one(), E::Fr::one()),
            |lc| lc + condition.get_variable(),
            |lc| lc + &y.lc(CS::one(), E::Fr::one())
                    - &rhs.lc(CS::one(), E::Fr::one())
        );

        new_lhs.push(x);
        new_rhs.push(y);
    }

    let mut f = new_lhs;
    f.extend(new_rhs);

    assert_eq!(f.len(), 512);

    Ok(f)
}
