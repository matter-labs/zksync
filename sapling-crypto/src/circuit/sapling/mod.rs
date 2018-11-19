// use pairing::{
//
// };

use ff::{
    PrimeField,
    PrimeFieldRepr,
    Field,
};

use bellman::{
    SynthesisError,
    ConstraintSystem,
    Circuit
};

use jubjub::{
    JubjubEngine,
    FixedGenerators
};

use constants;

use primitives::{
    ValueCommitment,
    ProofGenerationKey,
    PaymentAddress
};

use super::Assignment;
use super::boolean;
use super::ecc;
use super::pedersen_hash;
use super::blake2s;
use super::num;
use super::multipack;

/// This is an instance of the `Spend` circuit.
pub struct Spend<'a, E: JubjubEngine> {
    pub params: &'a E::Params,

    /// Pedersen commitment to the value being spent
    pub value_commitment: Option<ValueCommitment<E>>,

    /// Key required to construct proofs for spending notes
    /// for a particular spending key
    pub proof_generation_key: Option<ProofGenerationKey<E>>,

    /// The payment address associated with the note
    pub payment_address: Option<PaymentAddress<E>>,

    /// The randomness of the note commitment
    pub commitment_randomness: Option<E::Fs>,

    /// Re-randomization of the public key
    pub ar: Option<E::Fs>,

    /// The authentication path of the commitment in the tree
    pub auth_path: Vec<Option<(E::Fr, bool)>>,

    /// The anchor; the root of the tree. If the note being
    /// spent is zero-value, this can be anything.
    pub anchor: Option<E::Fr>
}

/// This is an output circuit instance.
pub struct Output<'a, E: JubjubEngine> {
    pub params: &'a E::Params,

    /// Pedersen commitment to the value being spent
    pub value_commitment: Option<ValueCommitment<E>>,

    /// The payment address of the recipient
    pub payment_address: Option<PaymentAddress<E>>,

    /// The randomness used to hide the note commitment data
    pub commitment_randomness: Option<E::Fs>,

    /// The ephemeral secret key for DH with recipient
    pub esk: Option<E::Fs>
}

/// Exposes a Pedersen commitment to the value as an
/// input to the circuit
fn expose_value_commitment<E, CS>(
    mut cs: CS,
    value_commitment: Option<ValueCommitment<E>>,
    params: &E::Params
) -> Result<Vec<boolean::Boolean>, SynthesisError>
    where E: JubjubEngine,
          CS: ConstraintSystem<E>
{
    // Booleanize the value into little-endian bit order
    let value_bits = boolean::u64_into_boolean_vec_le(
        cs.namespace(|| "value"),
        value_commitment.as_ref().map(|c| c.value)
    )?;

    // Compute the note value in the exponent
    let value = ecc::fixed_base_multiplication(
        cs.namespace(|| "compute the value in the exponent"),
        FixedGenerators::ValueCommitmentValue,
        &value_bits,
        params
    )?;

    // Booleanize the randomness. This does not ensure
    // the bit representation is "in the field" because
    // it doesn't matter for security.
    let rcv = boolean::field_into_boolean_vec_le(
        cs.namespace(|| "rcv"),
        value_commitment.as_ref().map(|c| c.randomness)
    )?;

    // Compute the randomness in the exponent
    let rcv = ecc::fixed_base_multiplication(
        cs.namespace(|| "computation of rcv"),
        FixedGenerators::ValueCommitmentRandomness,
        &rcv,
        params
    )?;

    // Compute the Pedersen commitment to the value
    let cv = value.add(
        cs.namespace(|| "computation of cv"),
        &rcv,
        params
    )?;

    // Expose the commitment as an input to the circuit
    cv.inputize(cs.namespace(|| "commitment point"))?;

    Ok(value_bits)
}

impl<'a, E: JubjubEngine> Circuit<E> for Spend<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError>
    {
        // Prover witnesses ak (ensures that it's on the curve)
        let ak = ecc::EdwardsPoint::witness(
            cs.namespace(|| "ak"),
            self.proof_generation_key.as_ref().map(|k| k.ak.clone()),
            self.params
        )?;

        // There are no sensible attacks on small order points
        // of ak (that we're aware of!) but it's a cheap check,
        // so we do it.
        ak.assert_not_small_order(
            cs.namespace(|| "ak not small order"),
            self.params
        )?;

        // Rerandomize ak and expose it as an input to the circuit
        {
            let ar = boolean::field_into_boolean_vec_le(
                cs.namespace(|| "ar"),
                self.ar
            )?;

            // Compute the randomness in the exponent
            let ar = ecc::fixed_base_multiplication(
                cs.namespace(|| "computation of randomization for the signing key"),
                FixedGenerators::SpendingKeyGenerator,
                &ar,
                self.params
            )?;

            let rk = ak.add(
                cs.namespace(|| "computation of rk"),
                &ar,
                self.params
            )?;

            rk.inputize(cs.namespace(|| "rk"))?;
        }

        // Compute nk = [nsk] ProofGenerationKey
        let nk;
        {
            // Witness nsk as bits
            let nsk = boolean::field_into_boolean_vec_le(
                cs.namespace(|| "nsk"),
                self.proof_generation_key.as_ref().map(|k| k.nsk.clone())
            )?;

            // NB: We don't ensure that the bit representation of nsk
            // is "in the field" (Fs) because it's not used except to
            // demonstrate the prover knows it. If they know a
            // congruency then that's equivalent.

            // Compute nk = [nsk] ProvingPublicKey
            nk = ecc::fixed_base_multiplication(
                cs.namespace(|| "computation of nk"),
                FixedGenerators::ProofGenerationKey,
                &nsk,
                self.params
            )?;
        }

        // This is the "viewing key" preimage for CRH^ivk
        let mut ivk_preimage = vec![];

        // Place ak in the preimage for CRH^ivk
        ivk_preimage.extend(
            ak.repr(cs.namespace(|| "representation of ak"))?
        );

        // This is the nullifier preimage for PRF^nf
        let mut nf_preimage = vec![];

        // Extend ivk and nf preimages with the representation of
        // nk.
        {
            let repr_nk = nk.repr(
                cs.namespace(|| "representation of nk")
            )?;

            ivk_preimage.extend(repr_nk.iter().cloned());
            nf_preimage.extend(repr_nk);
        }

        assert_eq!(ivk_preimage.len(), 512);
        assert_eq!(nf_preimage.len(), 256);

        // Compute the incoming viewing key ivk
        let mut ivk = blake2s::blake2s(
            cs.namespace(|| "computation of ivk"),
            &ivk_preimage,
            constants::CRH_IVK_PERSONALIZATION
        )?;

        // drop_5 to ensure it's in the field
        ivk.truncate(E::Fs::CAPACITY as usize);

        // Witness g_d, checking that it's on the curve.
        let g_d = {
            // This binding is to avoid a weird edge case in Rust's
            // ownership/borrowing rules. self is partially moved
            // above, but the closure for and_then will have to
            // move self (or a reference to self) to reference
            // self.params, so we have to copy self.params here.
            let params = self.params;

            ecc::EdwardsPoint::witness(
                cs.namespace(|| "witness g_d"),
                self.payment_address.as_ref().and_then(|a| a.g_d(params)),
                self.params
            )?
        };

        // Check that g_d is not small order. Technically, this check
        // is already done in the Output circuit, and this proof ensures
        // g_d is bound to a product of that check, but for defense in
        // depth let's check it anyway. It's cheap.
        g_d.assert_not_small_order(
            cs.namespace(|| "g_d not small order"),
            self.params
        )?;

        // Compute pk_d = g_d^ivk
        let pk_d = g_d.mul(
            cs.namespace(|| "compute pk_d"),
            &ivk,
            self.params
        )?;

        // Compute note contents:
        // value (in big endian) followed by g_d and pk_d
        let mut note_contents = vec![];

        // Handle the value; we'll need it later for the
        // dummy input check.
        let mut value_num = num::Num::zero();
        {
            // Get the value in little-endian bit order
            let value_bits = expose_value_commitment(
                cs.namespace(|| "value commitment"),
                self.value_commitment,
                self.params
            )?;

            // Compute the note's value as a linear combination
            // of the bits.
            let mut coeff = E::Fr::one();
            for bit in &value_bits {
                value_num = value_num.add_bool_with_coeff(
                    CS::one(),
                    bit,
                    coeff
                );
                coeff.double();
            }

            // Place the value in the note
            note_contents.extend(value_bits);
        }

        // Place g_d in the note
        note_contents.extend(
            g_d.repr(cs.namespace(|| "representation of g_d"))?
        );

        // Place pk_d in the note
        note_contents.extend(
            pk_d.repr(cs.namespace(|| "representation of pk_d"))?
        );

        assert_eq!(
            note_contents.len(),
            64 + // value
            256 + // g_d
            256 // p_d
        );

        // Compute the hash of the note contents
        let mut cm = pedersen_hash::pedersen_hash(
            cs.namespace(|| "note content hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &note_contents,
            self.params
        )?;

        {
            // Booleanize the randomness for the note commitment
            let rcm = boolean::field_into_boolean_vec_le(
                cs.namespace(|| "rcm"),
                self.commitment_randomness
            )?;

            // Compute the note commitment randomness in the exponent
            let rcm = ecc::fixed_base_multiplication(
                cs.namespace(|| "computation of commitment randomness"),
                FixedGenerators::NoteCommitmentRandomness,
                &rcm,
                self.params
            )?;

            // Randomize the note commitment. Pedersen hashes are not
            // themselves hiding commitments.
            cm = cm.add(
                cs.namespace(|| "randomization of note commitment"),
                &rcm,
                self.params
            )?;
        }

        // This will store (least significant bit first)
        // the position of the note in the tree, for use
        // in nullifier computation.
        let mut position_bits = vec![];

        // This is an injective encoding, as cur is a
        // point in the prime order subgroup.
        let mut cur = cm.get_x().clone();

        // Ascend the merkle tree authentication path
        for (i, e) in self.auth_path.into_iter().enumerate() {
            let cs = &mut cs.namespace(|| format!("merkle tree hash {}", i));

            // Determines if the current subtree is the "right" leaf at this
            // depth of the tree.
            let cur_is_right = boolean::Boolean::from(boolean::AllocatedBit::alloc(
                cs.namespace(|| "position bit"),
                e.map(|e| e.1)
            )?);

            // Push this boolean for nullifier computation later
            position_bits.push(cur_is_right.clone());

            // Witness the authentication path element adjacent
            // at this depth.
            let path_element = num::AllocatedNum::alloc(
                cs.namespace(|| "path element"),
                || {
                    Ok(e.get()?.0)
                }
            )?;

            // Swap the two if the current subtree is on the right
            let (xl, xr) = num::AllocatedNum::conditionally_reverse(
                cs.namespace(|| "conditional reversal of preimage"),
                &cur,
                &path_element,
                &cur_is_right
            )?;

            // We don't need to be strict, because the function is
            // collision-resistant. If the prover witnesses a congruency,
            // they will be unable to find an authentication path in the
            // tree with high probability.
            let mut preimage = vec![];
            preimage.extend(xl.into_bits_le(cs.namespace(|| "xl into bits"))?);
            preimage.extend(xr.into_bits_le(cs.namespace(|| "xr into bits"))?);

            // Compute the new subtree value
            cur = pedersen_hash::pedersen_hash(
                cs.namespace(|| "computation of pedersen hash"),
                pedersen_hash::Personalization::MerkleTree(i),
                &preimage,
                self.params
            )?.get_x().clone(); // Injective encoding
        }

        {
            let real_anchor_value = self.anchor;

            // Allocate the "real" anchor that will be exposed.
            let rt = num::AllocatedNum::alloc(
                cs.namespace(|| "conditional anchor"),
                || {
                    Ok(*real_anchor_value.get()?)
                }
            )?;

            // (cur - rt) * value = 0
            // if value is zero, cur and rt can be different
            // if value is nonzero, they must be equal
            cs.enforce(
                || "conditionally enforce correct root",
                |lc| lc + cur.get_variable() - rt.get_variable(),
                |lc| lc + &value_num.lc(E::Fr::one()),
                |lc| lc
            );

            // Expose the anchor
            rt.inputize(cs.namespace(|| "anchor"))?;
        }

        // Compute the cm + g^position for preventing
        // faerie gold attacks
        let mut rho = cm;
        {
            // Compute the position in the exponent
            let position = ecc::fixed_base_multiplication(
                cs.namespace(|| "g^position"),
                FixedGenerators::NullifierPosition,
                &position_bits,
                self.params
            )?;

            // Add the position to the commitment
            rho = rho.add(
                cs.namespace(|| "faerie gold prevention"),
                &position,
                self.params
            )?;
        }
        
        // Let's compute nf = BLAKE2s(nk || rho)
        nf_preimage.extend(
            rho.repr(cs.namespace(|| "representation of rho"))?
        );

        assert_eq!(nf_preimage.len(), 512);
        
        // Compute nf
        let nf = blake2s::blake2s(
            cs.namespace(|| "nf computation"),
            &nf_preimage,
            constants::PRF_NF_PERSONALIZATION
        )?;

        multipack::pack_into_inputs(cs.namespace(|| "pack nullifier"), &nf)
    }
}

impl<'a, E: JubjubEngine> Circuit<E> for Output<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError>
    {
        // Let's start to construct our note, which contains
        // value (big endian)
        let mut note_contents = vec![];

        // Expose the value commitment and place the value
        // in the note.
        note_contents.extend(expose_value_commitment(
            cs.namespace(|| "value commitment"),
            self.value_commitment,
            self.params
        )?);

        // Let's deal with g_d
        {
            let params = self.params;

            // Prover witnesses g_d, ensuring it's on the
            // curve.
            let g_d = ecc::EdwardsPoint::witness(
                cs.namespace(|| "witness g_d"),
                self.payment_address.as_ref().and_then(|a| a.g_d(params)),
                self.params
            )?;

            // g_d is ensured to be large order. The relationship
            // between g_d and pk_d ultimately binds ivk to the
            // note. If this were a small order point, it would
            // not do this correctly, and the prover could
            // double-spend by finding random ivk's that satisfy
            // the relationship.
            //
            // Further, if it were small order, epk would be
            // small order too!
            g_d.assert_not_small_order(
                cs.namespace(|| "g_d not small order"),
                self.params
            )?;

            // Extend our note contents with the representation of
            // g_d.
            note_contents.extend(
                g_d.repr(cs.namespace(|| "representation of g_d"))?
            );

            // Booleanize our ephemeral secret key
            let esk = boolean::field_into_boolean_vec_le(
                cs.namespace(|| "esk"),
                self.esk
            )?;

            // Create the ephemeral public key from g_d.
            let epk = g_d.mul(
                cs.namespace(|| "epk computation"),
                &esk,
                self.params
            )?;

            // Expose epk publicly.
            epk.inputize(cs.namespace(|| "epk"))?;
        }

        // Now let's deal with pk_d. We don't do any checks and
        // essentially allow the prover to witness any 256 bits
        // they would like.
        {
            // Just grab pk_d from the witness
            let pk_d = self.payment_address.as_ref().map(|e| e.pk_d.into_xy());

            // Witness the y-coordinate, encoded as little
            // endian bits (to match the representation)
            let y_contents = boolean::field_into_boolean_vec_le(
                cs.namespace(|| "pk_d bits of y"),
                pk_d.map(|e| e.1)
            )?;

            // Witness the sign bit
            let sign_bit = boolean::Boolean::from(boolean::AllocatedBit::alloc(
                cs.namespace(|| "pk_d bit of x"),
                pk_d.map(|e| e.0.into_repr().is_odd())
            )?);

            // Extend the note with pk_d representation
            note_contents.extend(y_contents);
            note_contents.push(sign_bit);
        }

        assert_eq!(
            note_contents.len(),
            64 + // value
            256 + // g_d
            256 // pk_d
        );

        // Compute the hash of the note contents
        let mut cm = pedersen_hash::pedersen_hash(
            cs.namespace(|| "note content hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &note_contents,
            self.params
        )?;

        {
            // Booleanize the randomness
            let rcm = boolean::field_into_boolean_vec_le(
                cs.namespace(|| "rcm"),
                self.commitment_randomness
            )?;

            // Compute the note commitment randomness in the exponent
            let rcm = ecc::fixed_base_multiplication(
                cs.namespace(|| "computation of commitment randomness"),
                FixedGenerators::NoteCommitmentRandomness,
                &rcm,
                self.params
            )?;

            // Randomize our note commitment
            cm = cm.add(
                cs.namespace(|| "randomization of note commitment"),
                &rcm,
                self.params
            )?;
        }

        // Only the x-coordinate of the output is revealed,
        // since we know it is prime order, and we know that
        // the x-coordinate is an injective encoding for
        // prime-order elements.
        cm.get_x().inputize(cs.namespace(|| "commitment"))?;

        Ok(())
    }
}

#[test]
fn test_input_circuit_with_bls12_381() {
    use ff::{Field, BitIterator};
    use pairing::bls12_381::*;
    use rand::{SeedableRng, Rng, XorShiftRng};
    use ::circuit::test::*;
    use jubjub::{JubjubBls12, fs, edwards};

    let params = &JubjubBls12::new();
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let tree_depth = 32;

    for _ in 0..10 {
        let value_commitment = ValueCommitment {
            value: rng.gen(),
            randomness: rng.gen()
        };

        let nsk: fs::Fs = rng.gen();
        let ak = edwards::Point::rand(rng, params).mul_by_cofactor(params);

        let proof_generation_key = ::primitives::ProofGenerationKey {
            ak: ak.clone(),
            nsk: nsk.clone()
        };

        let viewing_key = proof_generation_key.into_viewing_key(params);

        let payment_address;

        loop {
            let diversifier = ::primitives::Diversifier(rng.gen());

            if let Some(p) = viewing_key.into_payment_address(
                diversifier,
                params
            )
            {
                payment_address = p;
                break;
            }
        }

        let g_d = payment_address.diversifier.g_d(params).unwrap();
        let commitment_randomness: fs::Fs = rng.gen();
        let auth_path = vec![Some((rng.gen(), rng.gen())); tree_depth];
        let ar: fs::Fs = rng.gen();

        {
            let rk = viewing_key.rk(ar, params).into_xy();
            let expected_value_cm = value_commitment.cm(params).into_xy();
            let note = ::primitives::Note {
                value: value_commitment.value,
                g_d: g_d.clone(),
                pk_d: payment_address.pk_d.clone(),
                r: commitment_randomness.clone()
            };

            let mut position = 0u64;
            let cm: Fr = note.cm(params);
            let mut cur = cm.clone();

            for (i, val) in auth_path.clone().into_iter().enumerate()
            {
                let (uncle, b) = val.unwrap();

                let mut lhs = cur;
                let mut rhs = uncle;

                if b {
                    ::std::mem::swap(&mut lhs, &mut rhs);
                }

                let mut lhs: Vec<bool> = BitIterator::new(lhs.into_repr()).collect();
                let mut rhs: Vec<bool> = BitIterator::new(rhs.into_repr()).collect();

                lhs.reverse();
                rhs.reverse();

                cur = ::pedersen_hash::pedersen_hash::<Bls12, _>(
                    ::pedersen_hash::Personalization::MerkleTree(i),
                    lhs.into_iter()
                       .take(Fr::NUM_BITS as usize)
                       .chain(rhs.into_iter().take(Fr::NUM_BITS as usize)),
                    params
                ).into_xy().0;

                if b {
                    position |= 1 << i;
                }
            }

            let expected_nf = note.nf(&viewing_key, position, params);
            let expected_nf = multipack::bytes_to_bits_le(&expected_nf);
            let expected_nf = multipack::compute_multipacking::<Bls12>(&expected_nf);
            assert_eq!(expected_nf.len(), 2);

            let mut cs = TestConstraintSystem::<Bls12>::new();

            let instance = Spend {
                params: params,
                value_commitment: Some(value_commitment.clone()),
                proof_generation_key: Some(proof_generation_key.clone()),
                payment_address: Some(payment_address.clone()),
                commitment_randomness: Some(commitment_randomness),
                ar: Some(ar),
                auth_path: auth_path.clone(),
                anchor: Some(cur)
            };

            instance.synthesize(&mut cs).unwrap();

            assert!(cs.is_satisfied());
            assert_eq!(cs.num_constraints(), 98777);
            assert_eq!(cs.hash(), "d37c738e83df5d9b0bb6495ac96abf21bcb2697477e2c15c2c7916ff7a3b6a89");

            assert_eq!(cs.get("randomization of note commitment/x3/num"), cm);

            assert_eq!(cs.num_inputs(), 8);
            assert_eq!(cs.get_input(0, "ONE"), Fr::one());
            assert_eq!(cs.get_input(1, "rk/x/input variable"), rk.0);
            assert_eq!(cs.get_input(2, "rk/y/input variable"), rk.1);
            assert_eq!(cs.get_input(3, "value commitment/commitment point/x/input variable"), expected_value_cm.0);
            assert_eq!(cs.get_input(4, "value commitment/commitment point/y/input variable"), expected_value_cm.1);
            assert_eq!(cs.get_input(5, "anchor/input variable"), cur);
            assert_eq!(cs.get_input(6, "pack nullifier/input 0"), expected_nf[0]);
            assert_eq!(cs.get_input(7, "pack nullifier/input 1"), expected_nf[1]);
        }
    }
}

#[test]
fn test_output_circuit_with_bls12_381() {
    use ff::{Field};
    use pairing::bls12_381::*;
    use rand::{SeedableRng, Rng, XorShiftRng};
    use ::circuit::test::*;
    use jubjub::{JubjubBls12, fs, edwards};

    let params = &JubjubBls12::new();
    let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..100 {
        let value_commitment = ValueCommitment {
            value: rng.gen(),
            randomness: rng.gen()
        };

        let nsk: fs::Fs = rng.gen();
        let ak = edwards::Point::rand(rng, params).mul_by_cofactor(params);

        let proof_generation_key = ::primitives::ProofGenerationKey {
            ak: ak.clone(),
            nsk: nsk.clone()
        };

        let viewing_key = proof_generation_key.into_viewing_key(params);

        let payment_address;

        loop {
            let diversifier = ::primitives::Diversifier(rng.gen());

            if let Some(p) = viewing_key.into_payment_address(
                diversifier,
                params
            )
            {
                payment_address = p;
                break;
            }
        }

        let commitment_randomness: fs::Fs = rng.gen();
        let esk: fs::Fs = rng.gen();

        {
            let mut cs = TestConstraintSystem::<Bls12>::new();

            let instance = Output {
                params: params,
                value_commitment: Some(value_commitment.clone()),
                payment_address: Some(payment_address.clone()),
                commitment_randomness: Some(commitment_randomness),
                esk: Some(esk.clone())
            };

            instance.synthesize(&mut cs).unwrap();

            assert!(cs.is_satisfied());
            assert_eq!(cs.num_constraints(), 7827);
            assert_eq!(cs.hash(), "c26d5cdfe6ccd65c03390902c02e11393ea6bb96aae32a7f2ecb12eb9103faee");

            let expected_cm = payment_address.create_note(
                value_commitment.value,
                commitment_randomness,
                params
            ).expect("should be valid").cm(params);

            let expected_value_cm = value_commitment.cm(params).into_xy();

            let expected_epk = payment_address.g_d(params).expect("should be valid").mul(esk, params);
            let expected_epk_xy = expected_epk.into_xy();

            assert_eq!(cs.num_inputs(), 6);
            assert_eq!(cs.get_input(0, "ONE"), Fr::one());
            assert_eq!(cs.get_input(1, "value commitment/commitment point/x/input variable"), expected_value_cm.0);
            assert_eq!(cs.get_input(2, "value commitment/commitment point/y/input variable"), expected_value_cm.1);
            assert_eq!(cs.get_input(3, "epk/x/input variable"), expected_epk_xy.0);
            assert_eq!(cs.get_input(4, "epk/y/input variable"), expected_epk_xy.1);
            assert_eq!(cs.get_input(5, "commitment/input variable"), expected_cm);
        }
    }
}
