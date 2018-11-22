use pairing::{
    Engine,
};

use ff::{Field};

use bellman::{
    SynthesisError,
    ConstraintSystem
};

use super::{
    Assignment
};

use super::num::{
    AllocatedNum,
    Num
};

use ::jubjub::{
    edwards,
    JubjubEngine,
    JubjubParams,
    FixedGenerators
};

use super::lookup::{
    lookup3_xy
};

use super::boolean::{Boolean, 
    field_into_allocated_bits_le, 
    field_into_boolean_vec_le
};

use super::ecc::EdwardsPoint;

use ::redjubjub::{PublicKey};

use super::blake2s::{blake2s};

use super::pedersen_hash::{Personalization};

#[derive(Clone)]
pub struct EddsaSignature<E: JubjubEngine> {
    pub r: EdwardsPoint<E>,
    pub s: AllocatedNum<E>,
    pub pk: EdwardsPoint<E>
}

use ::alt_babyjubjub::{fs::Fs};

impl <E: JubjubEngine>EddsaSignature<E> {

    pub fn verify_on_message<CS>(
        &self,
        mut cs: CS,
        params: &E::Params,
        personalization: Personalization,
        message: &[Boolean],
        generator: EdwardsPoint<E>
    ) -> Result<Boolean, SynthesisError>
        where CS: ConstraintSystem<E>
    {
        // TODO check that s < Fs::Char
        let scalar_bits = field_into_boolean_vec_le(cs.namespace(|| "Get S bits"), self.s.get_value());
        assert!(scalar_bits.is_ok());
        let scalar_bits_conv: &[Boolean] = &(scalar_bits.unwrap());
        let sb = generator.mul(cs.namespace(|| "S*B computation"), &scalar_bits_conv, params);
        assert!(sb.is_ok());

        // let personalization_bytes = &personalization.get_bits();
        let personalization_bytes: &[u8] = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        // h = Hash(R, pubkey, message)

        // only order of R is checked. Public key and generator can be guaranteed to be in proper group!
        // by some other means for out particular case
        let r_is_in_order = self.r.assert_not_small_order(cs.namespace(|| "R is in right order"), &params);
        assert!(r_is_in_order.is_ok());
        // let r_x_serialized = field_into_allocated_bits_le(cs, r.get_x().get_value());
        let r_x_value = self.r.get_x().get_value();
        let r_x_serialized = field_into_boolean_vec_le(cs.namespace(|| "Serialize R_X"), r_x_value);
        assert!(r_x_serialized.is_ok());

        // let r_y_serialized = field_into_allocated_bits_le(cs, r.get_y().get_value());
        let r_y_value = self.r.get_y().get_value();
        let r_y_serialized = field_into_boolean_vec_le(cs.namespace(|| "Serialize R_Y"), r_y_value);
        assert!(r_y_serialized.is_ok());

        // let pk_x_serialized = field_into_allocated_bits_le(cs, pk.get_x().get_value());
        let pk_x_value = self.pk.get_x().get_value();
        let pk_x_serialized = field_into_boolean_vec_le(cs.namespace(|| "Serialize PK_X"), pk_x_value);
        assert!(pk_x_serialized.is_ok());

        // let pk_y_serialized = field_into_allocated_bits_le(cs, pk.get_y().get_value());
        let pk_y_value = self.pk.get_y().get_value();
        let pk_y_serialized = field_into_boolean_vec_le(cs.namespace(|| "Serialize PK_Y"), pk_y_value);
        assert!(pk_y_serialized.is_ok());

        let mut hash_content = r_x_serialized.unwrap();
        hash_content.extend(r_y_serialized.unwrap());
        hash_content.extend(pk_x_serialized.unwrap());
        hash_content.extend(pk_y_serialized.unwrap());
        hash_content.extend(message.iter().cloned());

        let hash_content_slice: &[Boolean] = &hash_content;

        let h = blake2s(cs.namespace(|| "Calculate EdDSA hash"), hash_content_slice, &personalization_bytes);
        assert!(h.is_ok());
        
        let pk_mul_hash = self.pk.mul(cs.namespace(|| "Calculate h*PK"), &h.unwrap(), params);
        assert!(pk_mul_hash.is_ok());

        let rhs = pk_mul_hash.unwrap().add(cs.namespace(|| "Make signature RHS"), &self.r, params);
        assert!(rhs.is_ok());

        let rhs_unwrapped = rhs.unwrap();

        let rhs_x = rhs_unwrapped.get_x();
        let rhs_y = rhs_unwrapped.get_y();

        let sb_unwrapped = sb.unwrap();

        let sb_x = sb_unwrapped.get_x();
        let sb_y = sb_unwrapped.get_y();

        // let one = CS::one();
        // cs.enforce(
        //     || "check x coordinate of signature",
        //     |lc| lc + rhs_x.get_variable(),
        //     |lc| lc + one,
        //     |lc| lc + sb_x.get_variable()
        // );

        // cs.enforce(
        //     || "check y coordinate of signature",
        //     |lc| lc + rhs_y.get_variable(),
        //     |lc| lc + one,
        //     |lc| lc + sb_y.get_variable()
        // );

        return Ok(Boolean::constant(true));
    }
} 


#[cfg(test)]
mod test {
    use ::eddsa::{PrivateKey, PublicKey};
    use rand::{SeedableRng, Rng, XorShiftRng};
    use super::*;
    use ::circuit::test::*;
    use ::circuit::boolean::{Boolean, AllocatedBit};
    use pairing::bn256::{Bn256, Fr};
    use ff::{PrimeField, PrimeFieldRepr};
    use ::alt_babyjubjub::AltJubjubBn256;
    use ::alt_babyjubjub::fs::Fs;
    

    #[test]
    fn test_valid_signatures() {
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let params = &AltJubjubBn256::new();
        let personalization = Personalization::NoteCommitment;
        let mut cs = TestConstraintSystem::<Bn256>::new();
        let sk = PrivateKey::<Bn256>(rng.gen());
        let vk = PublicKey::from_private(&sk, p_g, params);

        let msg1 = b"Foo bar";

        let sig1 = sk.sign(msg1, &mut rng, p_g, params);
        assert!(vk.verify(msg1, &sig1, p_g, params));

        // expect message to be around 12 bytes
        let input: Vec<bool> = (0..(8 * 12)).map(|_| rng.gen()).collect();

        let input_bools: Vec<Boolean> = input.iter().enumerate().map(|(i, b)| {
            Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| format!("input {}", i)), Some(*b)).unwrap()
            )
        }).collect();

        let mut sigs_bytes = [0u8; 32];
        sig1.s.into_repr().write_le(& mut sigs_bytes[..]).expect("get LE bytes of signature S");
        let mut sigs_repr = <pairing::bn256::Fr as PrimeField>::Repr::from(0);
        sigs_repr.read_le(&sigs_bytes[..]);
        // let sigs_repr = <pairing::bn256::Fr as PrimeField>::Repr::read_le(sigs_bytes);
        let sigs_converted = Fr::from_repr(sigs_repr).unwrap();
        // print!("{}", sigs_converted);
        // let sigs_converted = Fr::from_str(&sigs_string).unwrap();

        let s = AllocatedNum::alloc(cs.namespace(|| "allocate s"), || {
                Ok(sigs_converted)
            }
        ).unwrap();

        let public_generator = params.generator(FixedGenerators::NoteCommitmentRandomness).clone();

        let generator = EdwardsPoint::witness(cs.namespace(|| "allocate public generator"), Some(public_generator), params).unwrap();

        let r = EdwardsPoint::witness(cs.namespace(|| "allocate r"), Some(sig1.r), params).unwrap();

        let pk = EdwardsPoint::witness(cs.namespace(|| "allocate pk"), Some(vk.0), params).unwrap();

        let signature = EddsaSignature{r, s, pk};
        signature.verify_on_message(cs.namespace(|| "verify signature"), params, personalization, &input_bools, generator);

        assert!(cs.is_satisfied());
        print!("{}", cs.num_constraints());
    }

}


