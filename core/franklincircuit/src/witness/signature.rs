use crate::operation::TransactionSignature;
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use franklin_crypto::circuit::baby_eddsa::EddsaSignature;
use franklin_crypto::circuit::boolean::Boolean;
use franklin_crypto::circuit::ecc;
use franklin_crypto::circuit::num::AllocatedNum;
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};

#[derive(Clone)]
pub struct SignatureCircuit<'a, E: JubjubEngine> {
    pub signature: Option<TransactionSignature<E>>,
    pub pub_x: Option<E::Fr>,
    pub pub_y: Option<E::Fr>,
    pub data: Option<E::Fr>,
    pub params: &'a E::Params,
}

impl<'a, E: JubjubEngine> Circuit<E> for SignatureCircuit<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let public_generator = self
            .params
            .generator(FixedGenerators::SpendingKeyGenerator)
            .clone();
        let generator = ecc::EdwardsPoint::witness(
            cs.namespace(|| "allocate public generator"),
            Some(public_generator),
            self.params,
        )?;

        let x = AllocatedNum::alloc(cs.namespace(|| "x"), || self.pub_x.grab())?;
        let y = AllocatedNum::alloc(cs.namespace(|| "y"), || self.pub_y.grab())?;
        let sender_pk = ecc::EdwardsPoint::interpret(
            cs.namespace(|| "signer public key"),
            &x,
            &y,
            self.params,
        )?;

        let signature_r_x = AllocatedNum::alloc(cs.namespace(|| "signature r_x witness"), || {
            Ok(self.signature.get()?.r.into_xy().0)
        })?;

        let signature_r_y = AllocatedNum::alloc(cs.namespace(|| "signature r_y witness"), || {
            Ok(self.signature.get()?.r.into_xy().1)
        })?;

        let signature_r = ecc::EdwardsPoint::interpret(
            cs.namespace(|| "signature r as point"),
            &signature_r_x,
            &signature_r_y,
            self.params,
        )?;

        let signature_s = AllocatedNum::alloc(cs.namespace(|| "signature s witness"), || {
            Ok(self.signature.get()?.s)
        })?;

        let signature = EddsaSignature {
            r: signature_r,
            s: signature_s,
            pk: sender_pk,
        };
        // let data = AllocatedNum::alloc(cs.namespace(||"data"), || Ok(E::Fr::from_str("17").unwrap()))?;
        let data = AllocatedNum::alloc(cs.namespace(|| "data"), || self.data.grab())?;
        let mut data_bits = data.into_bits_le(cs.namespace(|| "data_bits"))?;
        data_bits.resize(256, Boolean::constant(false));

        let mut hash_bits: Vec<Boolean> = vec![];

        let mut pk_x_serialized = signature
            .pk
            .get_x()
            .clone()
            .into_bits_le(cs.namespace(|| "pk_x_bits"))?;
        pk_x_serialized.resize(256, Boolean::constant(false));
        hash_bits.extend(pk_x_serialized);
        let mut r_x_serialized = signature
            .r
            .get_x()
            .clone()
            .into_bits_le(cs.namespace(|| "r_x_bits"))?;
        r_x_serialized.resize(256, Boolean::constant(false));
        hash_bits.extend(r_x_serialized);

        let sig_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "hash_sig"),
            pedersen_hash::Personalization::NoteCommitment,
            &hash_bits,
            self.params,
        )?;
        let mut first_hash_bits = sig_hash
            .get_x()
            .into_bits_le(cs.namespace(|| "first_hash_bits"))?;
        first_hash_bits.resize(256, Boolean::constant(false));

        let mut second_hash_bits = vec![];
        second_hash_bits.extend(first_hash_bits);
        second_hash_bits.extend(data_bits);
        let second_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "second_hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &second_hash_bits,
            self.params,
        )?
        .get_x()
        .clone();

        let h_bits = second_hash.into_bits_le(cs.namespace(|| "h_bits"))?;

        let max_message_len = 32 as usize; //TODO fix when clear
                                           //TOdO: we should always use the same length
        signature.verify_raw_message_signature(
            cs.namespace(|| "verify transaction signature"),
            self.params,
            &h_bits,
            generator,
            max_message_len,
        )?;
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use super::*;

    use crate::utils::*;
    use bellman::Circuit;

    use ff::{BitIterator, PrimeField};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;

    use franklin_crypto::circuit::test::*;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklin_crypto::jubjub::FixedGenerators;

    use pairing::bn256::*;
    use rand::{Rng, SeedableRng, XorShiftRng};

    #[test]
    fn test_signature_circuit_franklin() {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, &params);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
        let sig_msg = Fr::from_str("2").unwrap(); //dummy sig msg cause skipped on deposit proof
        let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
        sig_bits.reverse();
        sig_bits.truncate(80);

        // println!(" capacity {}",<Bn256 as JubjubEngine>::Fs::Capacity);
        let signature = sign(&sig_bits, &sender_sk, p_g, &params, rng);
        let circ = SignatureCircuit {
            signature: signature,
            pub_x: Some(sender_x),
            pub_y: Some(sender_y),
            data: Some(sig_msg),
            params: params,
        };
        let mut cs = TestConstraintSystem::<Bn256>::new();

        circ.synthesize(&mut cs).unwrap();

        println!("unconstrained {}", cs.find_unconstrained());

        println!("num constrained{}", cs.num_constraints());

        let err = cs.which_is_unsatisfied();
        if err.is_some() {
            panic!("ERROR satisfying in {}", err.unwrap());
        }
    }

}
